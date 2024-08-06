use glam::{vec2, vec3, Vec2};
use lazy_static::lazy_static;
use rustc_hash::FxHashSet;
use stardust_xr_fusion::{
	core::values::{ResourceID, Vector2},
	drawable::Model,
	fields::Field,
	input::{InputData, InputDataType},
	items::panel::{Geometry, PanelItem, PanelItemAspect, SurfaceId},
	node::{NodeError, NodeType},
	spatial::{Spatial, SpatialAspect, Transform},
};
use stardust_xr_molecules::{
	hover_plane::{HoverPlane, HoverPlaneSettings},
	keyboard::{create_keyboard_panel_handler, KeyboardPanelHandler},
	mouse::MouseEvent,
	touch_plane::TouchPlane,
};
use std::sync::Arc;

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
		let input = receives_input
			.then(|| SurfaceInput::new(&root, &item, &id, physical_size, thickness, px_size))
			.transpose()?;
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
			parent.item.alias(),
			SurfaceId::Child(id),
			geometry.size,
			thickness,
			receives_input,
		)?;
		surface.parent_thickness = parent.thickness;
		Ok(surface)
	}

	pub fn update(&mut self) {
		if let Some(input) = &mut self.input {
			input.update(&self.item, &self.id);
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
			let _ = input.resize(physical_size, px_size);
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
	hover_plane: HoverPlane,
	touch_plane: TouchPlane,
	touches: FxHashSet<Arc<InputData>>,
	_keyboard: KeyboardPanelHandler,
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
		let plane_transform =
			Transform::from_translation(vec3(physical_size.x, -physical_size.y, 0.0) / 2.0);
		let hover_plane = HoverPlane::create(
			root,
			plane_transform.clone(),
			physical_size,
			thickness,
			0.0..px_size.x as f32,
			0.0..px_size.y as f32,
			HoverPlaneSettings {
				distance_range: 0.05..1.0,
				..Default::default()
			},
		)?;
		let touch_plane = TouchPlane::create(
			root,
			plane_transform.clone(),
			physical_size,
			thickness,
			0.0..px_size.x as f32,
			0.0..px_size.y as f32,
		)?;

		let keyboard = create_keyboard_panel_handler(
			item,
			Transform::none(),
			touch_plane.field(),
			item,
			id.clone(),
		)?;

		Ok(SurfaceInput {
			hover_plane,
			touch_plane,
			touches: FxHashSet::default(),
			_keyboard: keyboard,
		})
	}

	pub fn update(&mut self, item: &PanelItem, id: &SurfaceId) {
		self.hover_plane.update();
		self.touch_plane.update();

		self.update_pointer(item, id);
		self.update_touches(item, id);
	}

	pub fn update_pointer(&mut self, item: &PanelItem, id: &SurfaceId) {
		// set pointer position with the closest thing that's hovering
		if let Some(closest_hover) = self
			.hover_plane
			.hovering()
			.current()
			.iter()
			.chain(self.hover_plane.interact_status().actor())
			.reduce(|a, b| if a.distance > b.distance { b } else { a })
		{
			let (interact_point, _depth) = self.hover_plane.interact_point(closest_hover);
			let _ = item.pointer_motion(id.clone(), interact_point);
		}

		// left mouse button
		if self.hover_plane.interact_status().actor_started() {
			let _ = item.pointer_button(id.clone(), input_event_codes::BTN_LEFT!(), true);
		} else if self.hover_plane.interact_status().actor_stopped() {
			let _ = item.pointer_button(id.clone(), input_event_codes::BTN_LEFT!(), false);
		}

		for input in self
			.hover_plane
			.hovering()
			.current()
			.iter()
			.chain(self.hover_plane.interact_status().actor())
		{
			let mouse_event = input
				.datamap
				.deserialize::<MouseEvent>()
				.unwrap_or_default();

			let _ = item.pointer_scroll(
				id.clone(),
				mouse_event.scroll_continuous.unwrap_or([0.0; 2].into()),
				mouse_event.scroll_discrete.unwrap_or([0.0; 2].into()),
			);
		}
	}
	pub fn update_touches(&mut self, item: &PanelItem, id: &SurfaceId) {
		// proper touches
		for input_data in self
			.touch_plane
			.action()
			.interact()
			.added()
			.iter()
			.filter(filter_touch)
		{
			self.touches.insert(input_data.clone());
			let position = self.touch_plane.interact_point(input_data).0;
			let _ = item.touch_down(id.clone(), input_data.id as u32, position);
		}
		for input_data in self
			.touch_plane
			.action()
			.interact()
			.current()
			.iter()
			.filter(filter_touch)
		{
			if !self.touches.contains(input_data) {
				continue;
			}
			let position = self.touch_plane.interact_point(input_data).0;
			let _ = item.touch_move(input_data.id as u32, position);
		}
		for input_data in self
			.touch_plane
			.action()
			.interact()
			.removed()
			.iter()
			.filter(filter_touch)
		{
			self.touches.remove(input_data);
			let _ = item.touch_up(input_data.id as u32);
		}
	}

	pub fn resize(&mut self, physical_size: Vec2, px_size: Vector2<u32>) {
		let plane_transform =
			Transform::from_translation(vec3(physical_size.x, -physical_size.y, 0.0) / 2.0);

		let _ = self
			.hover_plane
			.root()
			.set_local_transform(plane_transform.clone());
		let _ = self.touch_plane.root().set_local_transform(plane_transform);

		let _ = self.hover_plane.set_size(physical_size);
		let _ = self.touch_plane.set_size(physical_size);
		self.hover_plane.x_range = 0.0..px_size.x as f32;
		self.hover_plane.y_range = 0.0..px_size.y as f32;
		self.touch_plane.x_range = 0.0..px_size.x as f32;
		self.touch_plane.y_range = 0.0..px_size.y as f32;
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.hover_plane.set_enabled(enabled);
		let _ = self.touch_plane.set_enabled(enabled);
	}

	pub fn field(&self) -> &Field {
		self.touch_plane.field()
	}
}

fn filter_touch(t: &&Arc<InputData>) -> bool {
	!matches!(t.input, InputDataType::Pointer(_))
}
