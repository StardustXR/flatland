use std::{ops::Range, sync::Arc};

use asteroids::{custom::ElementTrait, ValidState};
use derive_setters::Setters;
use glam::{vec2, vec3, Mat4, Vec2, Vec3};
use input_event_codes::BTN_LEFT;
use lazy_static::lazy_static;
use map_range::MapRange as _;
use stardust_xr_fusion::{
	drawable::{Line, LinePoint, Lines, LinesAspect as _, Model},
	fields::{Field, FieldAspect as _, Shape},
	input::{Finger, Hand, InputData, InputDataType, InputHandler},
	items::panel::{Geometry, PanelItem, PanelItemAspect, SurfaceId},
	node::{NodeError, OwnedAspect as _},
	spatial::{Spatial, SpatialAspect, SpatialRef, Transform},
	values::{color::rgba_linear, ResourceID, Vector2, Vector3},
};
use stardust_xr_molecules::{
	input_action::{InputQueue, InputQueueable as _, MultiAction, SimpleAction, SingleAction},
	keyboard::{create_keyboard_panel_handler, KeyboardPanelHandler},
	lines::{self, LineExt as _},
	mouse::MouseEvent,
	DebugSettings, UIElement as _, VisualDebug,
};
use tracing::{info, warn};

use crate::{surface_input::HoverPlaneElement, ToplevelState};

lazy_static! {
	pub static ref PANEL_RESOURCE: ResourceID = ResourceID::new_namespaced("flatland", "panel");
}

#[derive(Setters, Debug, Clone, PartialEq)]
#[setters(into, strip_option)]
pub struct SurfaceElement {
	pub initial_resolution: Vector2<u32>,
	pub receives_input: bool,
	pub item: PanelItem,
	pub id: SurfaceId,
	/// Pixels per meter
	pub density: f32,
	pub thickness: f32,
	pub child_thickness: f32,
	pub children: Vec<ChildSurfaceData>,
}

#[derive(Setters, Debug, Clone, PartialEq)]
pub struct ChildSurfaceData {
	id: SurfaceId,
	geometry: Geometry,
}

pub struct Surface {
	root: Spatial,
	item: PanelItem,
	id: SurfaceId,
	parent_thickness: Option<f32>,
	thickness: f32,
	model: Model,
}

impl SurfaceElement {
	pub fn new_child_element(&self, child_data: &ChildSurfaceData) -> SurfaceElement {
		SurfaceElement {
			initial_resolution: child_data.geometry.size,
			receives_input: self.receives_input,
			item: self.item.clone(),
			id: child_data.id.clone(),
			density: self.density,
			thickness: self.child_thickness,
			child_thickness: self.child_thickness,
			children: vec![],
		}
	}
}

impl ElementTrait<ToplevelState> for SurfaceElement {
	type Inner = Surface;

	type Error = NodeError;

	fn create_inner(
		&self,
		parent_space: &stardust_xr_fusion::spatial::SpatialRef,
	) -> Result<Self::Inner, Self::Error> {
		let physical_size: Vec2 = vec2(
			self.initial_resolution.x as f32,
			self.initial_resolution.y as f32,
		) / self.density;
		let root = Spatial::create(parent_space, Transform::none(), false)?;
		let panel_size = vec3(physical_size.x, physical_size.y, self.thickness);
		let model = Model::create(
			&root,
			Transform::from_translation_scale(panel_size * vec3(0.0, 0.0, -0.5), panel_size),
			&PANEL_RESOURCE,
		)?;
		self.item
			.apply_surface_material(self.id.clone(), &model.part("Panel")?)?;
		let surface = Surface {
			root,
			item: self.item.clone(),
			id: self.id.clone(),
			parent_thickness: None,
			thickness: self.thickness,
			model,
		};
		Ok(surface)
	}

	fn update(&self, old_decl: &Self, state: &mut ToplevelState, inner: &mut Self::Inner) {
		if self.initial_resolution != old_decl.initial_resolution {
			let physical_size: Vec2 = vec2(
				self.initial_resolution.x as f32,
				self.initial_resolution.y as f32,
			) / self.density;
			info!("{}", self.initial_resolution.x);
			let panel_size = vec3(physical_size.x, physical_size.y, self.thickness);
			if let Err(err) = inner
				.model
				.set_local_transform(Transform::from_translation_scale(
					panel_size * vec3(0.0, 0.0, -0.5),
					panel_size,
				)) {
				warn!("error while applying new scale to surface model: {err}");
			}
			inner.item.set_toplevel_size(self.initial_resolution);
			// if let Some(input) = inner.input.as_mut() {
			// 	input.resize(physical_size, self.initial_resolution);
			// }
		}
		// if let Some(input) = inner.input.as_mut() {
		// 	input.handle_events(&inner.item, &inner.id);
		// }
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> stardust_xr_fusion::spatial::SpatialRef {
		inner.root.clone().as_spatial_ref()
	}

	fn frame(&self, _info: &stardust_xr_fusion::root::FrameInfo, _inner: &mut Self::Inner) {}

	fn build(self) -> asteroids::Element<ToplevelState> {
		let mut iter = self
			.children
			.iter()
			.map(|v| self.new_child_element(v))
			.map(|v| v.build())
			.collect::<Vec<_>>();
		let surface_id = self.id.clone();
		let surface_id_2 = self.id.clone();
		iter.push(
			HoverPlaneElement {
				density: self.density,
				thickness: self.thickness,
				resolution: self.initial_resolution,
				distance_range: 0.05..0.2,
				line_start_thickness: 0.005,
				line_start_color_hover: rgba_linear!(1.0, 1.0, 1.0, 1.0),
				line_start_color_interact: rgba_linear!(1.0, 0.0, 1.0, 1.0),
				line_end_thickness: 0.005,
				line_end_color_hover: rgba_linear!(1.0, 1.0, 1.0, 1.0),
				line_end_color_interact: rgba_linear!(1.0, 0.0, 1.0, 1.0),
				on_hover: Some(move |state: &mut ToplevelState, point| {
					info!("{:?}", point);
					_ = state.panel_item.pointer_motion(surface_id.clone(), point);
				}),
				on_interact: Some(move |state: &mut ToplevelState, point, _distance| {
					let id = surface_id_2.clone();
					_ = state.panel_item.pointer_motion(id.clone(), point);
					_ = state.panel_item.pointer_button(id.clone(), BTN_LEFT!(), true);
					_ = state.panel_item.pointer_button(id.clone(), BTN_LEFT!(), false);
				}),
				_state: Default::default(),
			}
			.build(),
		);
		self.with_children(iter)
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
