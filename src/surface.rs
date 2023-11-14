use glam::{vec2, vec3, Vec2};
use lazy_static::lazy_static;
use mint::Vector2;
use rustc_hash::FxHashSet;
use stardust_xr_fusion::{
	core::values::Transform,
	drawable::{Model, ResourceID},
	fields::UnknownField,
	input::{InputData, InputDataType},
	items::panel::{Geometry, PanelItem, SurfaceID},
	node::{NodeError, NodeType},
	spatial::Spatial,
};
use stardust_xr_molecules::{
	keyboard::{create_keyboard_panel_handler, KeyboardPanelHandler},
	touch_plane::TouchPlane,
};
use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
	sync::Arc,
};

lazy_static! {
	pub static ref PANEL_RESOURCE: ResourceID = ResourceID::new_namespaced("flatland", "panel");
}

fn hash_input_data(input_data: &InputData) -> u32 {
	let mut hasher = DefaultHasher::new();
	input_data.uid.hash(&mut hasher);
	hasher.finish() as u32
}

// Pixels per meter, screen density
pub const PPM: f32 = 3000.0;
pub struct Surface {
	root: Spatial,
	item: PanelItem,
	id: SurfaceID,
	parent_thickness: f32,
	thickness: f32,
	model: Model,
	pub touch_plane: TouchPlane,
	touches: FxHashSet<Arc<InputData>>,
	keyboard: KeyboardPanelHandler,
	physical_size: Vec2,
}
impl Surface {
	pub fn create(
		parent: &Spatial,
		transform: Transform,
		item: PanelItem,
		id: SurfaceID,
		px_size: Vector2<u32>,
		thickness: f32,
	) -> Result<Self, NodeError> {
		let physical_size: Vec2 = vec2(px_size.x as f32, px_size.y as f32) / PPM;
		let root = Spatial::create(parent, transform, false)?;
		let panel_size = vec3(physical_size.x, physical_size.y, thickness);
		let model = Model::create(
			&root,
			Transform::from_position_scale(panel_size * vec3(0.5, -0.5, -0.5), panel_size),
			&PANEL_RESOURCE,
		)?;
		item.apply_surface_material(&id, &model.model_part("Panel")?)?;
		let touch_plane = TouchPlane::create(
			&root,
			Transform::from_position(vec3(physical_size.x, -physical_size.y, 0.0) / 2.0),
			physical_size,
			thickness,
			0.0..px_size.x as f32,
			0.0..px_size.y as f32,
		)?;
		// touch_plane.set_debug(Some(DebugSettings::default()));

		let keyboard = create_keyboard_panel_handler(
			&item,
			Transform::none(),
			&touch_plane.field(),
			&item,
			id.clone(),
		)?;

		Ok(Surface {
			root,
			item,
			id,
			parent_thickness: 0.0,
			thickness,
			model,
			touch_plane,
			touches: FxHashSet::default(),
			keyboard,
			physical_size,
		})
	}
	pub fn new_child(
		parent: &Surface,
		uid: String,
		geometry: &Geometry,
		thickness: f32,
	) -> Result<Self, NodeError> {
		let position = [
			geometry.origin.x as f32 / PPM,
			geometry.origin.y as f32 / PPM,
			thickness,
		];
		let mut surface = Self::create(
			&parent.root,
			Transform::from_position(position),
			parent.item.alias(),
			SurfaceID::Child(uid),
			geometry.size,
			thickness,
		)?;
		surface.parent_thickness = parent.thickness;
		Ok(surface)
	}

	pub fn update(&mut self) {
		self.touch_plane.update();

		// proper touches
		for input_data in self
			.touch_plane
			.started_inputs()
			.into_iter()
			.filter(|t| match t.input {
				InputDataType::Hand(_) => true,
				_ => false,
			}) {
			self.touches.insert(input_data.clone());
			let uid = hash_input_data(input_data);
			let position = self.touch_plane.interact_point(&input_data).0;
			let _ = self.item.touch_down(&self.id, uid, position);
		}
		for input_data in self
			.touch_plane
			.interacting_inputs()
			.into_iter()
			.filter(|t| match t.input {
				InputDataType::Hand(_) => true,
				_ => false,
			}) {
			if !self.touches.contains(input_data) {
				return;
			}
			let uid = hash_input_data(input_data);
			let position = self.touch_plane.interact_point(&input_data).0;
			let _ = self.item.touch_move(uid, position);
		}
		for input_data in self
			.touch_plane
			.stopped_inputs()
			.into_iter()
			.filter(|t| match t.input {
				InputDataType::Hand(_) => true,
				_ => false,
			}) {
			self.touches.remove(input_data);
			let uid = hash_input_data(input_data);
			let _ = self.item.touch_up(uid);
		}

		// "touches" but actually use the pointer instead
		if let Some(closest_hover) = self
			.touch_plane
			.hovering_inputs()
			.into_iter()
			.chain(self.touch_plane.interacting_inputs().clone())
			.filter(|i| match i.input {
				InputDataType::Hand(_) => false,
				_ => true,
			})
			.reduce(|a, b| if a.distance > b.distance { b } else { a })
		{
			let (interact_point, _depth) = self.touch_plane.interact_point(&closest_hover);
			self.item.pointer_motion(&self.id, interact_point).unwrap();
		}

		for input in self
			.touch_plane
			.hovering_inputs()
			.into_iter()
			.chain(self.touch_plane.interacting_inputs().clone())
		{
			let scroll_continous = input.datamap.with_data(|r| {
				let scroll_continous = r.index("scroll_continous").ok()?.as_vector();
				Some(
					[
						scroll_continous.index(0).ok()?.as_f32(),
						scroll_continous.index(1).ok()?.as_f32(),
					]
					.into(),
				)
			});
			let scroll_discrete = input.datamap.with_data(|r| {
				let scroll_discrete = r.index("scroll_discrete").ok()?.as_vector();
				Some(
					[
						scroll_discrete.index(0).ok()?.as_f32(),
						scroll_discrete.index(1).ok()?.as_f32(),
					]
					.into(),
				)
			});
			self.item
				.pointer_scroll(&self.id, scroll_continous, scroll_discrete)
				.unwrap();
		}

		if self.touch_plane.touch_started() {
			self.item
				.pointer_button(&self.id, input_event_codes::BTN_LEFT!(), true)
				.unwrap();
		} else if self.touch_plane.touch_stopped() {
			self.item
				.pointer_button(&self.id, input_event_codes::BTN_LEFT!(), false)
				.unwrap();
		}
	}
	pub fn set_offset(&self, px_offset: Vector2<i32>) -> Result<(), NodeError> {
		self.root.set_position(
			None,
			[
				px_offset.x as f32 / PPM,
				px_offset.y as f32 / PPM,
				self.parent_thickness,
			],
		)
	}
	pub fn resize(&mut self, px_size: Vector2<u32>) -> Result<(), NodeError> {
		let physical_size: Vec2 = vec2(px_size.x as f32, px_size.y as f32) / PPM;
		let panel_size = vec3(physical_size.x, physical_size.y, self.thickness);
		self.model.set_transform(
			None,
			Transform::from_position_scale(panel_size * vec3(0.5, -0.5, -0.5), panel_size),
		)?;
		self.touch_plane
			.root()
			.set_position(None, vec3(physical_size.x, -physical_size.y, 0.0) / 2.0)?;
		self.touch_plane.set_size(physical_size)?;
		self.touch_plane.x_range = 0.0..px_size.x as f32;
		self.touch_plane.y_range = 0.0..px_size.y as f32;
		self.physical_size = physical_size;
		self.keyboard
			.set_position(None, [-0.01, physical_size.y * -0.5, 0.0])
			.unwrap();
		// self.touch_plane.set_debug(Some(DebugSettings::default()));

		Ok(())
	}

	pub fn root(&self) -> &Spatial {
		&self.root
	}
	pub fn field(&self) -> UnknownField {
		self.touch_plane.field()
	}
	pub fn physical_size(&self) -> Vec2 {
		self.physical_size
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.touch_plane.set_enabled(enabled);
		let _ = self.model.set_enabled(enabled);
	}
}
