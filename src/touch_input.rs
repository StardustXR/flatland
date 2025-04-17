use std::path::Path;

use asteroids::{Context, ElementTrait, FnWrapper, Transformable, ValidState};
use derive_setters::Setters;
use glam::{vec3, Mat4, Vec2, Vec3};
use stardust_xr_fusion::{
	core::values::Vector2,
	drawable::{Line, LinePoint, Lines, LinesAspect},
	fields::{Field, FieldAspect, Shape},
	input::{InputData, InputDataType, InputHandler},
	node::{NodeError, NodeType},
	spatial::{SpatialRef, Transform},
	values::{color::rgba_linear, Vector3},
};
use stardust_xr_molecules::{
	input_action::{InputQueue, InputQueueable, MultiAction},
	lines::{self, LineExt},
	DebugSettings, VisualDebug,
};

#[derive_where::derive_where(Debug, PartialEq)]
#[derive(Setters)]
#[setters(into, strip_option)]
#[allow(clippy::type_complexity)]
pub struct TouchPlane<State: ValidState> {
	pub transform: Transform,
	pub physical_size: Vector2<f32>,
	pub thickness: f32,
	pub debug_line_settings: Option<DebugSettings>,

	#[setters(skip)]
	pub on_touch_down: FnWrapper<dyn Fn(&mut State, u32, Vector3<f32>) + Send + Sync>,
	#[setters(skip)]
	pub on_touch_move: FnWrapper<dyn Fn(&mut State, u32, Vector3<f32>) + Send + Sync>,
	#[setters(skip)]
	pub on_touch_up: FnWrapper<dyn Fn(&mut State, u32) + Send + Sync>,
}

impl<State: ValidState> Default for TouchPlane<State> {
	fn default() -> Self {
		Self {
			transform: Transform::identity(),
			physical_size: [1.0; 2].into(),
			thickness: 0.0,
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
		f: impl Fn(&mut State, u32, Vector3<f32>) + Send + Sync + 'static,
	) -> Self {
		self.on_touch_down = FnWrapper(Box::new(f));
		self
	}

	pub fn on_touch_move(
		mut self,
		f: impl Fn(&mut State, u32, Vector3<f32>) + Send + Sync + 'static,
	) -> Self {
		self.on_touch_move = FnWrapper(Box::new(f));
		self
	}

	pub fn on_touch_up(mut self, f: impl Fn(&mut State, u32) + Send + Sync + 'static) -> Self {
		self.on_touch_up = FnWrapper(Box::new(f));
		self
	}
}

impl<State: ValidState> ElementTrait<State> for TouchPlane<State> {
	type Inner = TouchSurfaceInputInner;
	type Resource = ();
	type Error = NodeError;

	fn create_inner(
		&self,
		spatial_parent: &SpatialRef,
		_context: &Context,
		_path: &Path,
		_resource: &mut Self::Resource,
	) -> Result<Self::Inner, Self::Error> {
		let field = Field::create(
			spatial_parent,
			self.transform,
			Shape::Box([self.physical_size.x, self.physical_size.y, self.thickness].into()),
		)?;

		let input = InputHandler::create(&field, Transform::none(), &field)?.queue()?;
		let lines = Lines::create(&field, Transform::identity(), &[])?;

		Ok(TouchSurfaceInputInner {
			input,
			field,
			touch: MultiAction::default(),
			physical_size: self.physical_size.into(),
			thickness: self.thickness,
			lines,
			debug_line_settings: self.debug_line_settings,
		})
	}

	fn update(
		&self,
		old: &Self,
		state: &mut State,
		inner: &mut Self::Inner,
		_resource: &mut Self::Resource,
	) {
		self.apply_transform(old, &inner.field);
		if self.debug_line_settings != old.debug_line_settings {
			inner.set_debug(self.debug_line_settings);
		}
		if self.physical_size != old.physical_size {
			inner.resize(self.physical_size.into());
		}

		inner.handle_events(state, self);
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.field.clone().as_spatial().as_spatial_ref()
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
	touch: MultiAction,
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
	) {
		if !self.input.handle_events() {
			return;
		}
		self.update_touches(state, decl);
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

	fn hover_point(input: &InputData) -> Vec3 {
		match &input.input {
			InputDataType::Hand(h) => Vec3::from(h.index.tip.position),
			InputDataType::Tip(t) => t.origin.into(),
			_ => Vec3::ZERO,
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

	pub fn update_touches<State: ValidState>(
		&mut self,
		state: &mut State,
		decl: &TouchPlane<State>,
	) {
		let physical_size = self.physical_size.into();
		self.touch.update(
			&self.input,
			|input| match &input.input {
				InputDataType::Pointer(_) => false,
				InputDataType::Hand(h) => Self::hovering(physical_size, h.index.tip.position, true),
				InputDataType::Tip(t) => Self::hovering(physical_size, t.origin, true),
			},
			|input| match &input.input {
				InputDataType::Hand(h) => {
					Self::hovering(physical_size, h.index.tip.position, false)
				}
				InputDataType::Tip(t) => Self::hovering(physical_size, t.origin, false),
				_ => false,
			},
		);

		for input_data in self.touch.interact().added().iter() {
			let position = self.to_local_coords(Self::hover_point(input_data));
			(decl.on_touch_down.0)(state, input_data.id as u32, position);
		}
		for input_data in self.touch.interact().current().iter() {
			let position = self.to_local_coords(Self::hover_point(input_data));
			(decl.on_touch_move.0)(state, input_data.id as u32, position);
		}
		for input_data in self.touch.interact().removed().iter() {
			(decl.on_touch_up.0)(state, input_data.id as u32);
		}
	}

	fn update_signifiers(&mut self) {
		let mut lines = vec![];
		lines.extend(self.debug_lines());

		// Add touch point visualization
		for input in self.touch.interact().current().iter() {
			lines.push(self.line_from_input(input));
		}

		self.lines.set_lines(&lines).unwrap();
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

	fn line_from_input(&self, input: &InputData) -> Line {
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
