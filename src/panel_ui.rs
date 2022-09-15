use crate::cursor::Cursor;
use glam::Quat;
use lazy_static::lazy_static;
use libstardustxr::fusion::{
	client::LogicStepInfo,
	drawable::Model,
	items::panel::{PanelItem, PanelItemCursor, PanelItemHandler, PanelItemInitData},
	node::NodeType,
	resource::Resource,
	WeakNodeRef,
};
use mint::Vector2;

lazy_static! {
	static ref PANEL_RESOURCE: Resource = Resource::new("flatland", "panel.glb");
}

pub const PPM: f32 = 1000.0;
pub struct PanelItemUI {
	pub item: WeakNodeRef<PanelItem>,
	pub model: Model,
	cursor: Cursor,
	cursor_pos: Vector2<f64>,
	size: Vector2<f64>,
}
impl PanelItemUI {
	pub fn new(
		init_data: PanelItemInitData,
		weak_item: WeakNodeRef<PanelItem>,
		item: &PanelItem,
	) -> Self {
		println!("Panel item created with {:#?}", init_data);
		item.set_spatial_parent(item.client().unwrap().get_root())
			.unwrap();
		item.set_transform(
			Some(item.client().unwrap().get_hmd()),
			Some(glam::vec3(0.0, 0.0, -0.5).into()),
			Some(Quat::IDENTITY.into()),
			Some(glam::vec3(1.0, 1.0, 1.0).into()),
		)
		.unwrap();
		let model = Model::resource_builder()
			.spatial_parent(&item)
			// .spatial_parent(item.node.client.upgrade().unwrap().get_root())
			.resource(&PANEL_RESOURCE)
			.scale(glam::vec3(
				init_data.size.x as f32 / PPM,
				init_data.size.y as f32 / PPM,
				0.01,
			))
			.build()
			.unwrap();

		item.apply_surface_material(&model, 0).unwrap();

		let cursor = Cursor::new(&item.spatial);
		cursor.update_info(&init_data.cursor, &item);
		cursor.update_position(
			Vector2::from([init_data.size.x as f64, init_data.size.y as f64]),
			Vector2::from([0.0, 0.0]),
		);

		PanelItemUI {
			item: weak_item,
			model,
			cursor,
			cursor_pos: Vector2::from([0.0, 0.0]),
			size: Vector2::from([init_data.size.x as f64, init_data.size.y as f64]),
		}
	}

	pub fn step(&mut self, _info: &LogicStepInfo) {}

	pub fn cursor_delta(&mut self, delta: mint::Vector2<f64>) {
		self.cursor_pos.x = (self.cursor_pos.x + delta.x).clamp(0.0, self.size.x - 1.0);
		self.cursor_pos.y = (self.cursor_pos.y + delta.y).clamp(0.0, self.size.y - 1.0);
		self.item.with_node(|panel_item| {
			panel_item
				.pointer_motion(Vector2::from_slice(&[
					self.cursor_pos.x as f32,
					self.cursor_pos.y as f32,
				]))
				.unwrap();
		});
		self.cursor.update_position(self.size, self.cursor_pos);
	}
}
impl PanelItemHandler for PanelItemUI {
	fn resize(&mut self, size: Vector2<u32>) {
		println!("Got resize of {}, {}", size.x, size.y);
		self.size = Vector2::from_slice(&[size.x as f64, size.y as f64]);
		self.model
			.set_scale(
				None,
				glam::vec3(size.x as f32 / PPM, size.y as f32 / PPM, 0.01),
			)
			.unwrap();
	}

	fn set_cursor(&mut self, info: Option<PanelItemCursor>) {
		println!("Set cursor with info {:?}", info);

		self.item.with_node(|panel_item| {
			self.cursor.update_info(&info, panel_item);
		});
	}
}
impl Drop for PanelItemUI {
	fn drop(&mut self) {
		println!("Panel item destroyed");
	}
}
