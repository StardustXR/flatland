use glam::{vec2, vec3, Mat4, Vec2, Vec3};
use lazy_static::lazy_static;
use map_range::MapRange;
use stardust_xr_fusion::{
	core::values::{ResourceID, Vector2},
	drawable::{Line, LinePoint, Lines, LinesAspect, Model},
	fields::{Field, FieldAspect, Shape},
	input::{Finger, Hand, InputData, InputDataType, InputHandler},
	items::panel::{Geometry, PanelItem, PanelItemAspect, SurfaceId},
	node::{NodeError, NodeType},
	spatial::{Spatial, SpatialAspect, Transform},
	values::{color::rgba_linear, Vector3},
};
use stardust_xr_molecules::{
	input_action::{InputQueue, InputQueueable, MultiAction, SimpleAction, SingleAction},
	keyboard::{create_keyboard_panel_handler, KeyboardPanelHandler},
	lines::{self, LineExt},
	mouse::MouseEvent,
	DebugSettings, UIElement, VisualDebug,
};
use std::{ops::Range, sync::Arc};

lazy_static! {
	pub static ref PANEL_RESOURCE: ResourceID = ResourceID::new_namespaced("flatland", "panel");
}

// Pixels per meter, screen density
pub const PPM: f32 = 3000.0;
pub struct Surface {
	root: Spatial,
	item: PanelItem,
	id: SurfaceId,
	parent_thickness: f32,
	thickness: f32,
	model: Model,
	pub input: Option<SurfaceInput>,
	physical_size: Vec2,
}
impl Surface {
	pub fn create(
		parent: &impl SpatialAspect,
		transform: Transform,
		item: PanelItem,
		id: SurfaceId,
		px_size: Vector2<u32>,
		thickness: f32,
		receives_input: bool,
	) -> Result<Self, NodeError> {
		let physical_size: Vec2 = vec2(px_size.x as f32, px_size.y as f32) / PPM;
		let root = Spatial::create(parent, transform, false)?;
		let panel_size = vec3(physical_size.x, physical_size.y, thickness);
		let model = Model::create(
			&root,
			Transform::from_translation_scale(panel_size * vec3(0.5, -0.5, -0.5), panel_size),
			&PANEL_RESOURCE,
		)?;
		item.apply_surface_material(id.clone(), &model.part("Panel")?)?;
		let mut input = receives_input
			.then(|| SurfaceInput::new(&root, &item, &id, physical_size, thickness, px_size))
			.transpose()?;
		if let Some(input) = &mut input {
			input.set_debug(Some(DebugSettings::default()));
		}
		Ok(Surface {
			root,
			item,
			id,
			parent_thickness: 0.0,
			thickness,
			model,
			input,
			physical_size,
		})
	}
	pub fn new_child(
		parent: &Surface,
		id: u64,
		geometry: &Geometry,
		thickness: f32,
		receives_input: bool,
	) -> Result<Self, NodeError> {
		let position = [
			geometry.origin.x as f32 / PPM,
			-geometry.origin.y as f32 / PPM,
			thickness,
		];
		let mut surface = Self::create(
			&parent.root,
			Transform::from_translation(position),
			parent.item.clone(),
			SurfaceId::Child(id),
			geometry.size,
			thickness,
			receives_input,
		)?;
		surface.parent_thickness = parent.thickness;
		Ok(surface)
	}

	pub fn handle_events(&mut self) {
		if let Some(input) = &mut self.input {
			input.handle_events(&self.item, &self.id);
		}
	}

	pub fn set_offset(&self, px_offset: Vector2<i32>) -> Result<(), NodeError> {
		self.root.set_local_transform(Transform::from_translation([
			px_offset.x as f32 / PPM,
			px_offset.y as f32 / PPM,
			self.parent_thickness,
		]))
	}
	pub fn resize(&mut self, px_size: Vector2<u32>) -> Result<(), NodeError> {
		let physical_size: Vec2 = vec2(px_size.x as f32, px_size.y as f32) / PPM;
		let panel_size = vec3(physical_size.x, physical_size.y, self.thickness);
		self.model
			.set_local_transform(Transform::from_translation_scale(
				panel_size * vec3(0.5, -0.5, -0.5),
				panel_size,
			))?;
		if let Some(input) = &mut self.input {
			input.resize(physical_size, px_size);
		}
		self.physical_size = physical_size;
		Ok(())
	}

	pub fn root(&self) -> &Spatial {
		&self.root
	}
	pub fn physical_size(&self) -> Vec2 {
		self.physical_size
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.model.set_enabled(enabled);
		if let Some(input) = &mut self.input {
			input.set_enabled(enabled);
		}
	}
}

pub struct SurfaceInput {
	input: InputQueue,
	field: Field,
	hover: SimpleAction,
	pointer_hover: Option<Arc<InputData>>,
	left_click: SingleAction,
	middle_click: SingleAction,
	right_click: SingleAction,
	touch: MultiAction,

	physical_size: Vec2,
	thickness: f32,
	pub x_range: Range<f32>,
	pub y_range: Range<f32>,

	keyboard: KeyboardPanelHandler,
	lines: Lines,
	debug_line_settings: Option<DebugSettings>,
}

impl SurfaceInput {
	pub fn new(
		root: &impl SpatialAspect,
		item: &PanelItem,
		id: &SurfaceId,
		physical_size: Vec2,
		thickness: f32,
		px_size: Vector2<u32>,
	) -> Result<Self, NodeError> {
		let field = Field::create(
			root,
			Transform::from_translation(vec3(physical_size.x, -physical_size.y, 0.0) / 2.0),
			Shape::Box([physical_size.x, physical_size.y, thickness].into()),
		)?;
		let input = InputHandler::create(&field, Transform::none(), &field)?.queue()?;
		let hover = SimpleAction::default();
		let lines = Lines::create(&field, Transform::identity(), &[])?;

		let keyboard = create_keyboard_panel_handler(
			item,
			Transform::none(),
			&field,
			item.clone(),
			id.clone(),
		)?;

		Ok(SurfaceInput {
			input,
			field,
			hover,
			pointer_hover: None,
			left_click: SingleAction::default(),
			middle_click: SingleAction::default(),
			right_click: SingleAction::default(),
			touch: MultiAction::default(),

			physical_size,
			thickness,
			x_range: 0.0..px_size.x as f32,
			y_range: 0.0..px_size.y as f32,

			keyboard,
			lines,
			debug_line_settings: None,
		})
	}

	pub fn handle_events(&mut self, item: &PanelItem, id: &SurfaceId) {
		self.keyboard.handle_events();
		self.update_pointer(item, id);
		self.update_touches(item, id);
		self.update_signifiers();
	}

	pub fn resize(&mut self, physical_size: Vec2, px_size: Vector2<u32>) {
		self.physical_size = physical_size;

		let _ = self.field.set_local_transform(Transform::from_translation(
			vec3(physical_size.x, -physical_size.y, 0.0) / 2.0,
		));
		let _ = self.field.set_shape(Shape::Box(
			[physical_size.x, physical_size.y, self.thickness].into(),
		));

		self.x_range = 0.0..px_size.x as f32;
		self.y_range = 0.0..px_size.y as f32;
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.input.handler().set_enabled(enabled);
	}
}

// Pointer inputs
impl SurfaceInput {
	fn hovering(size: Vector2<f32>, point: Vector3<f32>, front: bool) -> bool {
		point.x.abs() * 2.0 < size.x
			&& point.y.abs() * 2.0 < size.y
			&& point.z.is_sign_positive() == front
	}
	fn hover_point(input: &InputData) -> Vec3 {
		match &input.input {
			InputDataType::Pointer(p) => {
				let normal = vec3(0.0, 0.0, 1.0);
				let denom = normal.dot(p.direction().into());
				let t = -Vec3::from(p.origin).dot(normal) / denom;
				Vec3::from(p.origin) + Vec3::from(p.direction()) * t
			}
			InputDataType::Hand(h) => {
				(Vec3::from(h.index.tip.position) + Vec3::from(h.thumb.tip.position)) * 0.5
			}
			InputDataType::Tip(t) => t.origin.into(),
		}
	}
	fn hover_info(&self, input: &InputData) -> (Vector2<f32>, f32) {
		let interact_point = Self::hover_point(input);

		let half_size_x = self.physical_size.x / 2.0;
		let half_size_y = self.physical_size.y / 2.0;
		let x = interact_point
			.x
			.map_range(-half_size_x..half_size_x, self.x_range.clone());
		let y = interact_point
			.y
			.map_range(half_size_y..-half_size_y, self.y_range.clone());

		([x, y].into(), interact_point.z)
	}
	#[inline]
	#[allow(clippy::too_many_arguments)]
	fn handle_mouse_button(
		input: &InputQueue,
		item: &PanelItem,
		id: &SurfaceId,
		closest_hover: Option<Arc<InputData>>,
		action: &mut SingleAction,
		finger: fn(&Hand) -> &Finger,
		datamap_key: &str,
		button_code: u32,
	) {
		action.update(
			false,
			input,
			|input| Some(input.id) == closest_hover.clone().map(|c| c.id),
			|input| {
				match &input.input {
					InputDataType::Hand(h) => {
						let thumb_tip = Vec3::from(h.thumb.tip.position);
						let finger_tip = Vec3::from((finger)(h).tip.position);
						thumb_tip.distance(finger_tip) < 0.02 // Adjust threshold as needed
					}
					_ => input
						.datamap
						.with_data(|d| d.idx(datamap_key).as_f32() > 0.5),
				}
			},
		);
		if action.actor_started() {
			// println!("Mouse button {button_code} down");
			let _ = item.pointer_button(id.clone(), button_code, true);
		}
		if action.actor_stopped() {
			// println!("Mouse button {button_code} up");
			let _ = item.pointer_button(id.clone(), button_code, false);
		}
	}
	fn update_pointer(&mut self, item: &PanelItem, surface_id: &SurfaceId) {
		self.hover.update(&self.input, &|input| match &input.input {
			InputDataType::Pointer(_) => input.distance <= 0.0,
			_ => {
				let hover_point = Self::hover_point(input);
				(0.05..0.2).contains(&hover_point.z.abs())
					&& Self::hovering(self.physical_size.into(), hover_point.into(), true)
			}
		});

		// set pointer position with the closest thing that's hovering
		self.pointer_hover = self
			.hover
			.currently_acting()
			.iter()
			.chain(self.input.input().keys().filter(|c| c.captured))
			.min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap())
			.cloned();

		// Handle mouse button actions
		SurfaceInput::handle_mouse_button(
			&self.input,
			item,
			surface_id,
			self.pointer_hover.clone(),
			&mut self.left_click,
			|hand| &hand.index,
			"select",
			input_event_codes::BTN_LEFT!(),
		);
		SurfaceInput::handle_mouse_button(
			&self.input,
			item,
			surface_id,
			self.pointer_hover.clone(),
			&mut self.middle_click,
			|hand| &hand.middle,
			"middle",
			input_event_codes::BTN_MIDDLE!(),
		);
		SurfaceInput::handle_mouse_button(
			&self.input,
			item,
			surface_id,
			self.pointer_hover.clone(),
			&mut self.right_click,
			|hand| &hand.ring,
			"context",
			input_event_codes::BTN_RIGHT!(),
		);

		let Some(closest_hover) = self.pointer_hover.clone() else {
			return;
		};
		let (interact_point, _depth) = self.hover_info(&closest_hover);
		let _ = item.pointer_motion(surface_id.clone(), interact_point);

		// Scroll handling
		let mouse_event = closest_hover
			.datamap
			.deserialize::<MouseEvent>()
			.unwrap_or_default();

		let _ = item.pointer_scroll(
			surface_id.clone(),
			mouse_event.scroll_continuous.unwrap_or([0.0; 2].into()),
			mouse_event.scroll_discrete.unwrap_or([0.0; 2].into()),
		);
	}
}

// Touch points
impl SurfaceInput {
	fn touch_point(&self, input: &InputData) -> (Vector2<f32>, f32) {
		let interact_point = match &input.input {
			InputDataType::Pointer(p) => {
				let normal = vec3(0.0, 0.0, 1.0);
				let denom = normal.dot(p.direction().into());
				let t = -Vec3::from(p.origin).dot(normal) / denom;
				(Vec3::from(p.origin) + Vec3::from(p.direction()) * t).into()
			}
			InputDataType::Hand(h) => h.index.tip.position,
			InputDataType::Tip(t) => t.origin,
		};
		let half_size_x = self.physical_size.x / 2.0;
		let half_size_y = self.physical_size.y / 2.0;

		let x = interact_point
			.x
			.clamp(-half_size_x, half_size_x)
			.map_range(-half_size_x..half_size_x, self.x_range.clone());
		let y = interact_point
			.y
			.clamp(-half_size_y, half_size_y)
			.map_range(half_size_y..-half_size_y, self.y_range.clone());

		([x, y].into(), interact_point.z)
	}
	pub fn update_touches(&mut self, item: &PanelItem, id: &SurfaceId) {
		let physical_size = self.physical_size.into();
		self.touch.update(
			&self.input,
			|input| match &input.input {
				InputDataType::Pointer(_) => false,
				InputDataType::Hand(h) => Self::hovering(physical_size, h.index.tip.position, true),
				InputDataType::Tip(t) => Self::hovering(physical_size, t.origin, true),
			},
			|input| match &input.input {
				InputDataType::Pointer(_) => {
					input.datamap.with_data(|d| d.idx("select").as_f32() > 0.5)
				}
				InputDataType::Hand(h) => {
					Self::hovering(physical_size, h.index.tip.position, false)
				}
				InputDataType::Tip(t) => Self::hovering(physical_size, t.origin, false),
			},
		);

		// proper touches
		for input_data in self.touch.interact().added().iter() {
			let _ = item.touch_down(
				id.clone(),
				input_data.id as u32,
				self.touch_point(input_data).0,
			);
		}
		for input_data in self.touch.interact().current().iter() {
			let _ = item.touch_move(input_data.id as u32, self.touch_point(input_data).0);
		}
		for input_data in self.touch.interact().removed().iter() {
			let _ = item.touch_up(input_data.id as u32);
		}
	}
}

impl SurfaceInput {
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
			.filter(|_| self.touch.interact().current().is_empty())
			.filter_map(|p| self.line_from_input(p, p.captured))
			.collect::<Vec<_>>()
	}
	fn line_from_input(&self, input: &InputData, interacting: bool) -> Option<Line> {
		if let InputDataType::Pointer(_) = &input.input {
			None
		} else {
			Some(self.line_from_point(SurfaceInput::hover_point(input), interacting))
		}
	}
	fn line_from_point(&self, point: Vec3, interacting: bool) -> Line {
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
					color: if interacting {
						settings.line_start_color_interact
					} else {
						settings.line_start_color_hover
					},
				},
				LinePoint {
					point: point.into(),
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

impl VisualDebug for SurfaceInput {
	fn set_debug(&mut self, settings: Option<DebugSettings>) {
		self.debug_line_settings = settings;
	}
}
