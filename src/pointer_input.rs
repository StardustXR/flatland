use derive_setters::Setters;
use glam::{vec2, vec3, Mat4, Vec2, Vec3};
use serde::Deserialize;
use stardust_xr_asteroids::{
	Context, CreateInnerInfo, CustomElement, FnWrapper, Transformable, ValidState,
};
use stardust_xr_fusion::{
	drawable::{Line, LinePoint, Lines, LinesAspect},
	fields::{Field, FieldAspect, Shape},
	input::{Finger, Hand, InputData, InputDataType, InputHandler},
	node::{NodeError, NodeType},
	root::FrameInfo,
	spatial::{SpatialRef, Transform},
	values::Vector2,
	values::{color::rgba_linear, Vector3},
};
use stardust_xr_molecules::{
	input_action::{InputQueue, InputQueueable, SimpleAction, SingleAction},
	lines::{self, LineExt},
	DebugSettings, VisualDebug,
};
use std::{sync::Arc, time::Duration};

#[derive(Debug, Default, Clone, Deserialize)]
pub struct MouseEvent {
	pub scroll_continuous: Option<Vector2<f32>>,
	pub scroll_discrete: Option<Vector2<f32>>,
}

#[derive_where::derive_where(Debug, PartialEq)]
#[derive(Setters)]
#[setters(into, strip_option)]
#[allow(clippy::type_complexity)]
pub struct PointerPlane<State: ValidState> {
	pub transform: Transform,
	pub physical_size: Vector2<f32>,
	pub thickness: f32,
	pub click_freeze_time: Duration,
	pub debug_line_settings: Option<DebugSettings>,

	#[setters(skip)]
	pub on_mouse_button: FnWrapper<dyn Fn(&mut State, u32, bool) + Send + Sync>,
	#[setters(skip)]
	pub on_pointer_motion: FnWrapper<dyn Fn(&mut State, Vector3<f32>) + Send + Sync>,
	#[setters(skip)]
	pub on_scroll: FnWrapper<dyn Fn(&mut State, MouseEvent) + Send + Sync>,
}

impl<State: ValidState> Default for PointerPlane<State> {
	fn default() -> Self {
		Self {
			transform: Transform::identity(),
			physical_size: [1.0; 2].into(),
			thickness: 0.0,
			click_freeze_time: Duration::from_millis(300),
			debug_line_settings: None,

			on_mouse_button: FnWrapper(Box::new(|_, _, _| {})),
			on_pointer_motion: FnWrapper(Box::new(|_, _| {})),
			on_scroll: FnWrapper(Box::new(|_, _| {})),
		}
	}
}

impl<State: ValidState> PointerPlane<State> {
	pub fn on_mouse_button(
		mut self,
		f: impl Fn(&mut State, u32, bool) + Send + Sync + 'static,
	) -> Self {
		self.on_mouse_button = FnWrapper(Box::new(f));
		self
	}

	pub fn on_pointer_motion(
		mut self,
		f: impl Fn(&mut State, Vector3<f32>) + Send + Sync + 'static,
	) -> Self {
		self.on_pointer_motion = FnWrapper(Box::new(f));
		self
	}

	pub fn on_scroll(mut self, f: impl Fn(&mut State, MouseEvent) + Send + Sync + 'static) -> Self {
		self.on_scroll = FnWrapper(Box::new(f));
		self
	}
}

impl<State: ValidState> CustomElement<State> for PointerPlane<State> {
	type Inner = PointerSurfaceInputInner;
	type Resource = ();
	type Error = NodeError;

	fn create_inner(
		&self,
		_context: &Context,
		info: CreateInnerInfo,
		_resource: &mut Self::Resource,
	) -> Result<Self::Inner, Self::Error> {
		let field = Field::create(
			info.parent_space,
			self.transform,
			Shape::Box([self.physical_size.x, self.physical_size.y, self.thickness].into()),
		)?;

		let input = InputHandler::create(&field, Transform::none(), &field)?.queue()?;
		let hover = SimpleAction::default();
		let lines = Lines::create(&field, Transform::identity(), &[])?;

		Ok(PointerSurfaceInputInner {
			input,
			field,
			hover,
			pointer_hover: None,
			left_click: SingleAction::default(),
			middle_click: SingleAction::default(),
			right_click: SingleAction::default(),
			start_click_time: 0.0,
			physical_size: self.physical_size.into(),
			thickness: self.thickness,
			lines,
			debug_line_settings: self.debug_line_settings,
		})
	}

	fn diff(&self, old: &Self, inner: &mut Self::Inner, _resource: &mut Self::Resource) {
		self.apply_transform(old, &inner.field);
		if self.debug_line_settings != old.debug_line_settings {
			inner.set_debug(self.debug_line_settings);
		}
		if self.physical_size != old.physical_size {
			inner.resize(self.physical_size.into());
		}
	}

	fn frame(
		&self,
		_context: &Context,
		frame_info: &FrameInfo,
		state: &mut State,
		inner: &mut Self::Inner,
	) {
		inner.handle_events(state, self, frame_info);
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.field.clone().as_spatial().as_spatial_ref()
	}
}

impl<State: ValidState> Transformable for PointerPlane<State> {
	fn transform(&self) -> &Transform {
		&self.transform
	}
	fn transform_mut(&mut self) -> &mut Transform {
		&mut self.transform
	}
}

pub struct PointerSurfaceInputInner {
	input: InputQueue,
	field: Field,
	hover: SimpleAction,
	pointer_hover: Option<Arc<InputData>>,
	left_click: SingleAction,
	middle_click: SingleAction,
	right_click: SingleAction,
	start_click_time: f32,
	physical_size: Vec2,
	thickness: f32,
	lines: Lines,
	debug_line_settings: Option<DebugSettings>,
}

impl PointerSurfaceInputInner {
	pub fn handle_events<State: ValidState>(
		&mut self,
		state: &mut State,
		decl: &PointerPlane<State>,
		frame_info: &FrameInfo,
	) {
		if !self.input.handle_events() {
			return;
		}
		self.update_pointer(state, decl, frame_info);
		self.update_signifiers();
	}

	pub fn resize(&mut self, physical_size: Vec2) {
		self.physical_size = physical_size;
		let _ = self.field.set_shape(Shape::Box(
			[physical_size.x, physical_size.y, self.thickness].into(),
		));
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.input.handler().set_enabled(enabled);
	}

	fn hovering(size: Vector2<f32>, point: Vector3<f32>, front: bool) -> bool {
		point.x.abs() * 2.0 < size.x
			&& point.y.abs() * 2.0 < size.y
			&& point.z.is_sign_positive() == front
	}

	fn hover_point(input: &InputData, stable: bool) -> Vec3 {
		match &input.input {
			InputDataType::Pointer(p) => {
				let normal = vec3(0.0, 0.0, 1.0);
				let denom = normal.dot(p.direction().into());
				let t = -Vec3::from(p.origin).dot(normal) / denom;
				Vec3::from(p.origin) + Vec3::from(p.direction()) * t
			}
			InputDataType::Hand(h) => {
				if stable {
					h.stable_pinch_position().into()
				} else {
					h.predicted_pinch_position().into()
				}
			}
			InputDataType::Tip(t) => t.origin.into(),
		}
	}

	fn to_local_coords(&self, point: Vec3) -> Vector3<f32> {
		[
			point.x + self.physical_size.x / 2.0,
			-point.y + self.physical_size.y / 2.0,
			point.z,
		]
		.into()
	}

	#[allow(clippy::too_many_arguments)]
	fn handle_button<State: ValidState>(
		state: &mut State,
		input: &InputQueue,
		decl: &PointerPlane<State>,
		elapsed_time: f32,
		start_click_time: &mut f32,
		action: &mut SingleAction,
		finger: fn(&Hand) -> &Finger,
		datamap_key: &str,
		button: u32,
		closest_hover: Option<Arc<InputData>>,
	) {
		action.update(
			false,
			input,
			|input| Some(input.id) == closest_hover.clone().map(|c| c.id),
			|input| match &input.input {
				InputDataType::Hand(h) => {
					let thumb_tip = Vec3::from(h.thumb.tip.position);
					let finger_tip = Vec3::from((finger)(h).tip.position);
					thumb_tip.distance(finger_tip) < 0.02
				}
				_ => input
					.datamap
					.with_data(|d| d.idx(datamap_key).as_f32() > 0.5),
			},
		);
		if action.actor_started() {
			*start_click_time = elapsed_time;
			(decl.on_mouse_button.0)(state, button, true);
		}
		if action.actor_stopped() {
			(decl.on_mouse_button.0)(state, button, false);
		}
	}

	fn update_pointer<State: ValidState>(
		&mut self,
		state: &mut State,
		decl: &PointerPlane<State>,
		frame_info: &FrameInfo,
	) {
		self.hover.update(&self.input, &|input| match &input.input {
			InputDataType::Pointer(_) => input.distance <= 0.0,
			_ => {
				let hover_point = Self::hover_point(input, true);
				(0.05..0.2).contains(&hover_point.z.abs())
					&& Self::hovering(self.physical_size.into(), hover_point.into(), true)
			}
		});

		self.pointer_hover = self
			.hover
			.currently_acting()
			.iter()
			.chain(self.input.input().keys().filter(|c| c.captured))
			.min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap())
			.cloned();

		Self::handle_button(
			state,
			&self.input,
			decl,
			frame_info.elapsed,
			&mut self.start_click_time,
			&mut self.left_click,
			|hand| &hand.index,
			"select",
			input_event_codes::BTN_LEFT!(),
			self.pointer_hover.clone(),
		);

		Self::handle_button(
			state,
			&self.input,
			decl,
			frame_info.elapsed,
			&mut self.start_click_time,
			&mut self.middle_click,
			|hand| &hand.middle,
			"middle",
			input_event_codes::BTN_MIDDLE!(),
			self.pointer_hover.clone(),
		);

		Self::handle_button(
			state,
			&self.input,
			decl,
			frame_info.elapsed,
			&mut self.start_click_time,
			&mut self.right_click,
			|hand| &hand.ring,
			"context",
			input_event_codes::BTN_RIGHT!(),
			self.pointer_hover.clone(),
		);

		let Some(closest_hover) = self.pointer_hover.clone() else {
			return;
		};

		let position = self.to_local_coords(Self::hover_point(&closest_hover, true));
		if frame_info.elapsed - self.start_click_time > decl.click_freeze_time.as_secs_f32() {
			(decl.on_pointer_motion.0)(state, position);
		}

		let mouse_event = closest_hover
			.datamap
			.deserialize::<MouseEvent>()
			.unwrap_or_default();
		(decl.on_scroll.0)(state, mouse_event);

		#[derive(Deserialize, Default)]
		struct ScrollInput {
			scroll: Option<Vector2<f32>>,
		}

		let scroll = closest_hover
			.datamap
			.deserialize::<ScrollInput>()
			.unwrap_or_default()
			.scroll;
		(decl.on_scroll.0)(
			state,
			MouseEvent {
				// TODO: fix the server, we're not sending some events some apps need to register
				// continuous scroll, we should send that instead of discrete
				scroll_continuous: None,
				scroll_discrete: scroll
					.map(|scroll| (vec2(scroll.x, scroll.y) * frame_info.delta).into()),
			},
		);
	}

	fn update_signifiers(&mut self) {
		let mut lines = self.hover_lines();
		lines.extend(self.debug_lines());

		self.lines.set_lines(&lines).unwrap();
	}

	fn debug_lines(&mut self) -> Vec<Line> {
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

	fn hover_lines(&mut self) -> Vec<Line> {
		self.pointer_hover
			.iter()
			.filter_map(|p| self.line_from_input(p, p.captured))
			.collect::<Vec<_>>()
	}

	fn line_from_input(&self, input: &InputData, interacting: bool) -> Option<Line> {
		if let InputDataType::Pointer(_) = &input.input {
			None
		} else {
			Some(self.line_from_point(
				PointerSurfaceInputInner::hover_point(input, true),
				PointerSurfaceInputInner::hover_point(input, false),
				interacting,
			))
		}
	}

	fn line_from_point(&self, stable_point: Vec3, unstable_point: Vec3, interacting: bool) -> Line {
		let settings = stardust_xr_molecules::hover_plane::HoverPlaneSettings::default();
		Line {
			points: vec![
				LinePoint {
					point: [
						stable_point
							.x
							.clamp(self.physical_size.x * -0.5, self.physical_size.x * 0.5),
						stable_point
							.y
							.clamp(self.physical_size.y * -0.5, self.physical_size.y * 0.5),
						0.0,
					]
					.into(),
					thickness: settings.line_start_thickness,
					color: if interacting {
						settings.line_start_color_interact
					} else {
						settings.line_start_color_hover
					},
				},
				LinePoint {
					point: unstable_point.into(),
					thickness: settings.line_end_thickness,
					color: if interacting {
						settings.line_end_color_interact
					} else {
						settings.line_end_color_hover
					},
				},
			],
			cyclic: false,
		}
	}
}

impl VisualDebug for PointerSurfaceInputInner {
	fn set_debug(&mut self, settings: Option<DebugSettings>) {
		self.debug_line_settings = settings;
	}
}
