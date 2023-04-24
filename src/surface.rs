use glam::{vec2, vec3, Vec2};
use lazy_static::lazy_static;
use mint::Vector2;
use stardust_xr_fusion::{
	core::values::Transform,
	drawable::{Model, ResourceID},
	fields::UnknownField,
	items::panel::{PanelItem, PositionerData, SurfaceID},
	node::{NodeError, NodeType},
	spatial::Spatial,
};
use stardust_xr_molecules::touch_plane::TouchPlane;

lazy_static! {
	pub static ref PANEL_RESOURCE: ResourceID = ResourceID::new_namespaced("flatland", "panel");
}

// Pixels per meter, screen density
pub const PPM: f32 = 1000.0;
pub const THICKNESS: f32 = 0.01;
pub struct Surface {
	root: Spatial,
	item: PanelItem,
	id: SurfaceID,
	model: Model,
	touch_plane: TouchPlane,
	physical_size: Vec2,
}
impl Surface {
	pub fn create(
		parent: &Spatial,
		transform: Transform,
		item: PanelItem,
		id: SurfaceID,
		px_size: Vector2<u32>,
	) -> Result<Self, NodeError> {
		let physical_size: Vec2 = vec2(px_size.x as f32, px_size.y as f32) / PPM;
		let root = Spatial::create(parent, transform, false)?;
		let panel_size = vec3(physical_size.x, physical_size.y, THICKNESS);
		let model = Model::create(
			&root,
			Transform::from_position_scale(panel_size * vec3(0.5, -0.5, -0.5), panel_size),
			&PANEL_RESOURCE,
		)?;
		item.apply_surface_material(&id, &model, 0)?;
		let touch_plane = TouchPlane::create(
			&root,
			Transform::from_position(vec3(physical_size.x, -physical_size.y, 0.0) / 2.0),
			physical_size,
			THICKNESS,
			0.0..px_size.x as f32,
			0.0..px_size.y as f32,
		)?;
		// touch_plane.set_debug(Some(DebugSettings::default()));

		Ok(Surface {
			root,
			item,
			id,
			model,
			touch_plane,
			physical_size,
		})
	}
	pub fn new_child(
		parent: &Surface,
		id: SurfaceID,
		positioner_data: &PositionerData,
	) -> Result<Self, NodeError> {
		let offset = positioner_data.get_pos();
		let position = [offset.x as f32 / PPM, offset.y as f32 / PPM, THICKNESS];
		Self::create(
			&parent.root,
			Transform::from_position(position),
			parent.item.alias(),
			id,
			positioner_data.size,
		)
	}

	pub fn update(&mut self) {
		self.touch_plane.update();

		if let Some(closest_hover) = self
			.touch_plane
			.hovering_inputs()
			.into_iter()
			.chain(self.touch_plane.interacting_inputs())
			.reduce(|a, b| if a.distance > b.distance { b } else { a })
		{
			let interact_point = self.touch_plane.interact_point(closest_hover);
			self.item.pointer_motion(&self.id, interact_point).unwrap();
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
	pub fn resize(&mut self, px_size: Vector2<u32>) -> Result<(), NodeError> {
		let physical_size: Vec2 = vec2(px_size.x as f32, px_size.y as f32) / PPM;
		let panel_size = vec3(physical_size.x, physical_size.y, THICKNESS);
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
		// self.touch_plane.set_debug(Some(DebugSettings::default()));

		Ok(())
	}
	pub fn set_offset(&self, px_offset: Vector2<i32>) -> Result<(), NodeError> {
		self.root.set_position(
			None,
			[
				px_offset.x as f32 / PPM,
				px_offset.y as f32 / PPM,
				THICKNESS,
			],
		)
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
}
