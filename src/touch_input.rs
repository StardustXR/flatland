use derive_setters::Setters;
use glam::{Mat4, Vec2, Vec3, vec3};
use rustc_hash::FxHashMap;
use stardust_xr_asteroids::{
	Context, CreateInnerInfo, CustomElement, FnWrapper, Transformable, ValidState,
};
use stardust_xr_fusion::{
	Error,
	client::FrameInfo,
	drawable::{Line, LinePoint, Lines, LinesExt as _},
	fields::{Field, FieldExt as _, Shape},
	spatial::{PartialTransform, Spatial, Transform},
	suis::{InputDataType, InputMethod},
	types::{Vec2F, Vec3F, rgba_linear},
};
use stardust_xr_molecules::{
	DebugSettings, VisualDebug,
	input_action::{InputQueue, InputSnapshot, MultiAction},
	lines::{self, LineExt},
};
use std::{
	collections::HashMap,
	time::{Duration, Instant},
};

#[derive_where::derive_where(Debug, PartialEq)]
#[derive(Setters)]
#[setters(into, strip_option)]
#[allow(clippy::type_complexity)]
pub struct TouchPlane<State: ValidState> {
	pub transform: Transform,
	pub physical_size: Vec2F,
	pub thickness: f32,
	pub click_freeze_time: Duration,
	pub debug_line_settings: Option<DebugSettings>,

	#[setters(skip)]
	pub on_touch_down: FnWrapper<dyn Fn(&mut State, u32, Vec3F) + Send + Sync>,
	#[setters(skip)]
	pub on_touch_move: FnWrapper<dyn Fn(&mut State, u32, Vec3F) + Send + Sync>,
	#[setters(skip)]
	pub on_touch_up: FnWrapper<dyn Fn(&mut State, u32) + Send + Sync>,
}

impl<State: ValidState> Default for TouchPlane<State> {
	fn default() -> Self {
		Self {
			transform: Transform::IDENTITY,
			physical_size: [1.0; 2].into(),
			thickness: 0.0,
			click_freeze_time: Duration::from_millis(300),
			debug_line_settings: None,

			on_touch_down: FnWrapper(Box::new(|_, _, _| {})),
			on_touch_move: FnWrapper(Box::new(|_, _, _| {})),
			on_touch_up: FnWrapper(Box::new(|_, _| {})),
		}
	}
}

impl<State: ValidState> TouchPlane<State> {
	pub fn on_touch_down(
		mut self,
		f: impl Fn(&mut State, u32, Vec3F) + Send + Sync + 'static,
	) -> Self {
		self.on_touch_down = FnWrapper(Box::new(f));
		self
	}

	pub fn on_touch_move(
		mut self,
		f: impl Fn(&mut State, u32, Vec3F) + Send + Sync + 'static,
	) -> Self {
		self.on_touch_move = FnWrapper(Box::new(f));
		self
	}

	pub fn on_touch_up(mut self, f: impl Fn(&mut State, u32) + Send + Sync + 'static) -> Self {
		self.on_touch_up = FnWrapper(Box::new(f));
		self
	}
}

impl<State: ValidState> CustomElement<State> for TouchPlane<State> {
	type Inner = TouchSurfaceInputInner;
	type Error = Error;

	async fn create_inner(
		&self,
		ctx: &Context,
		info: CreateInnerInfo,
	) -> Result<Self::Inner, Self::Error> {
		let (field, _) = Field::new(
			&ctx.stardust_client,
			&info.child_space,
			Shape::Box {
				size: [self.physical_size.x, self.physical_size.y, self.thickness].into(),
			},
		)
		.await?;

		let input = InputQueue::new(
			&ctx.stardust_client,
			info.child_space.clone(),
			field.clone(),
			info.child_space.spatial_ref().await?,
		)
		.await?;
		let lines = Lines::new(&ctx.stardust_client, &info.child_space, Vec::new()).await?;

		Ok(TouchSurfaceInputInner {
			input,
			field,
			touch: MultiAction::default(),
			start_tap_times: FxHashMap::default(),
			physical_size: self.physical_size.into(),
			thickness: self.thickness,
			lines,
			debug_line_settings: self.debug_line_settings,
			spatial: info.child_space,
			points: HashMap::new(),
			last_touch: 0,
		})
	}

	fn diff(&self, old_self: &Self, _ctx: &Context, inner: &mut Self::Inner) {
		self.apply_transform(old_self, &inner.spatial);
		if self.debug_line_settings != old_self.debug_line_settings {
			inner.set_debug(self.debug_line_settings);
		}
		if self.physical_size != old_self.physical_size {
			inner.resize(self.physical_size.into());
		}
	}

	fn frame(
		&self,
		_context: &Context,
		info: &FrameInfo,
		state: &mut State,
		inner: &mut Self::Inner,
	) {
		inner.handle_events(state, self, info);
	}
}

impl<State: ValidState> Transformable for TouchPlane<State> {
	fn transform(&self) -> &Transform {
		&self.transform
	}
	fn transform_mut(&mut self) -> &mut Transform {
		&mut self.transform
	}
}

pub struct TouchSurfaceInputInner {
	input: InputQueue,
	field: Field,
	spatial: Spatial,
	touch: MultiAction,
	start_tap_times: FxHashMap<u32, Instant>,
	points: HashMap<InputMethod, u32>,
	last_touch: u32,
	physical_size: Vec2,
	thickness: f32,
	lines: Lines,
	debug_line_settings: Option<DebugSettings>,
}

impl TouchSurfaceInputInner {
	pub fn handle_events<State: ValidState>(
		&mut self,
		state: &mut State,
		decl: &TouchPlane<State>,
		_info: &FrameInfo,
	) {
		if !self.input.handle_events() {
			return;
		}
		self.update_touches(state, decl);
		self.update_signifiers();
	}

	pub fn resize(&mut self, physical_size: Vec2) {
		self.physical_size = physical_size;
		let _ = self.field.set_shape(Shape::Box {
			size: [physical_size.x, physical_size.y, self.thickness].into(),
		});
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self
			.spatial
			.set_local_transform(PartialTransform::from_scale([enabled as u8 as f32; 3]));
	}

	fn hovering(size: Vec2F, point: Vec3F, front: bool) -> bool {
		point.x.abs() * 2.0 < size.x
			&& point.y.abs() * 2.0 < size.y
			&& point.z.is_sign_positive() == front
	}

	fn hover_point(input: &InputSnapshot) -> Vec3 {
		match &input.input() {
			InputDataType::Hand { data: h } => Vec3::from(h.index.tip.pose.position),
			InputDataType::Tip { data: t } => t.pose.position.into(),
			_ => Vec3::ZERO,
		}
	}

	fn to_local_coords(&self, point: Vec3) -> Vec3F {
		[
			point.x + self.physical_size.x / 2.0,
			-point.y + self.physical_size.y / 2.0,
			point.z,
		]
		.into()
	}

	pub fn update_touches<State: ValidState>(
		&mut self,
		state: &mut State,
		decl: &TouchPlane<State>,
	) {
		let physical_size = self.physical_size.into();
		self.touch.update(
			&self.input,
			|input| match &input.input() {
				InputDataType::Pointer { data: _ } => false,
				InputDataType::Hand { data: h } => {
					Self::hovering(physical_size, h.index.tip.pose.position, true)
				}
				InputDataType::Tip { data: t } => {
					Self::hovering(physical_size, t.pose.position, true)
				}
			},
			|input| match &input.input() {
				InputDataType::Hand { data: h } => {
					Self::hovering(physical_size, h.index.tip.pose.position, false)
				}
				InputDataType::Tip { data: t } => {
					Self::hovering(physical_size, t.pose.position, false)
				}
				_ => false,
			},
		);

		for input_data in self.touch.interact().added().iter() {
			let position = self.to_local_coords(Self::hover_point(input_data));
			self.last_touch += 1;
			let id = self.last_touch;
			self.points.insert(input_data.method.clone(), id);
			// TODO: use proper timestamps for this?
			self.start_tap_times.insert(id, Instant::now());
			(decl.on_touch_down.0)(state, id, position);
		}
		for input_data in self.touch.interact().current().iter() {
			let position = self.to_local_coords(Self::hover_point(input_data));
			// should always exist
			let id = *self.points.get(&input_data.method).unwrap();
			if let Some(start_time) = self.start_tap_times.get(&(id))
				&& start_time.elapsed().as_secs_f32() > decl.click_freeze_time.as_secs_f32()
			{
				(decl.on_touch_move.0)(state, id, position);
			}
		}
		for input_data in self.touch.interact().removed().iter() {
			// should always exist
			let id = self.points.remove(&input_data.method).unwrap();
			self.start_tap_times.remove(&id);
			(decl.on_touch_up.0)(state, id);
		}
	}

	fn update_signifiers(&mut self) {
		let mut lines = vec![];
		lines.extend(self.debug_lines());

		// Add touch point visualization
		for input in self.touch.interact().current().iter() {
			lines.push(self.line_from_input(input));
		}

		self.lines.set_lines(lines).unwrap();
	}

	fn debug_lines(&self) -> Vec<Line> {
		let Some(settings) = &self.debug_line_settings else {
			return vec![];
		};
		let line_front = lines::rounded_rectangle(
			self.physical_size.x,
			self.physical_size.y,
			settings.line_thickness * 0.5,
			4,
		)
		.thickness(settings.line_thickness)
		.color(settings.line_color);
		let line_back = line_front
			.clone()
			.color(rgba_linear!(
				settings.line_color.c.r,
				settings.line_color.c.g,
				settings.line_color.c.b,
				settings.line_color.a * 0.5
			))
			.transform(Mat4::from_translation(vec3(0.0, 0.0, -self.thickness)));
		vec![line_front, line_back]
	}

	fn line_from_input(&self, input: &InputSnapshot) -> Line {
		self.line_from_point(Self::hover_point(input))
	}

	fn line_from_point(&self, point: Vec3) -> Line {
		let settings = stardust_xr_molecules::hover_plane::HoverPlaneSettings::default();
		Line {
			points: vec![
				LinePoint {
					point: [
						point
							.x
							.clamp(self.physical_size.x * -0.5, self.physical_size.x * 0.5),
						point
							.y
							.clamp(self.physical_size.y * -0.5, self.physical_size.y * 0.5),
						0.0,
					]
					.into(),
					thickness: settings.line_start_thickness,
					color: settings.line_start_color_interact,
				},
				LinePoint {
					point: point.into(),
					thickness: settings.line_end_thickness,
					color: settings.line_end_color_interact,
				},
			],
			cyclic: false,
		}
	}
}
impl VisualDebug for TouchSurfaceInputInner {
	fn set_debug(&mut self, settings: Option<DebugSettings>) {
		self.debug_line_settings = settings;
	}
}
