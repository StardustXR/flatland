use crate::cursor::Cursor;
use glam::Quat;
use lazy_static::lazy_static;
use libstardustxr::fusion::{
	async_trait,
	client::LogicStepInfo,
	drawable::Model,
	item::{PanelItem, PanelItemCursor, PanelItemHandler, PanelItemInitData},
	resource::Resource,
};
use mint::Vector2;
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;

lazy_static! {
	static ref PANEL_RESOURCE: Resource = Resource::new("flatland", "panel.glb");
}

pub const PPM: f32 = 1000.0;
pub struct PanelItemUI {
	pub item: Weak<PanelItem>,
	pub model: Model,
	cursor: Cursor,
	cursor_pos: Mutex<Vector2<f64>>,
	size: Mutex<Vector2<f64>>,
}
impl PanelItemUI {
	pub async fn new(item: &Arc<PanelItem>, init_data: PanelItemInitData) -> Arc<Self> {
		item.set_spatial_parent(item.node.client.upgrade().unwrap().get_root())
			.await
			.unwrap();
		item.set_transform(
			Some(item.node.client.upgrade().unwrap().get_hmd()),
			Some(glam::vec3(0.0, 0.0, -0.5).into()),
			Some(Quat::IDENTITY.into()),
			Some(glam::vec3(1.0, 1.0, 1.0).into()),
		)
		.await
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
			.await
			.unwrap();

		item.apply_surface_material(&model, 0).await.unwrap();

		let cursor = Cursor::new(&item.spatial).await;
		cursor.update_info(&init_data.cursor, &item).await;
		cursor
			.update_position(
				Vector2::from([init_data.size.x as f64, init_data.size.y as f64]),
				Vector2::from([0.0, 0.0]),
			)
			.await;

		let panel_ui = Arc::new(PanelItemUI {
			item: Arc::downgrade(item),
			model,
			cursor,
			cursor_pos: Mutex::new(Vector2::from([0.0, 0.0])),
			size: Mutex::new(Vector2::from([
				init_data.size.x as f64,
				init_data.size.y as f64,
			])),
		});
		item.set_handler(&panel_ui);

		panel_ui
	}

	pub async fn step(&self, _info: &LogicStepInfo) {}

	pub async fn cursor_delta(&self, delta: mint::Vector2<f64>) {
		let size = *self.size.lock().await;
		let mut cursor_pos = self.cursor_pos.lock().await;
		cursor_pos.x = (cursor_pos.x + delta.x).clamp(0.0, size.x - 1.0);
		cursor_pos.y = (cursor_pos.y + delta.y).clamp(0.0, size.y - 1.0);
		if let Some(panel_item) = self.item.upgrade() {
			panel_item
				.pointer_motion(Vector2::from_slice(&[
					cursor_pos.x as f32,
					cursor_pos.y as f32,
				]))
				.await
				.unwrap();
		}
		self.cursor.update_position(size, *cursor_pos).await;
	}
}
#[async_trait]
impl PanelItemHandler for PanelItemUI {
	async fn resize(&self, size: Vector2<u32>) {
		println!("Got resize of {}, {}", size.x, size.y);
		*self.size.lock().await = Vector2::from_slice(&[size.x as f64, size.y as f64]);
		self.model
			.set_scale(
				None,
				glam::vec3(size.x as f32 / PPM, size.y as f32 / PPM, 0.01),
			)
			.await
			.unwrap();
	}

	async fn set_cursor(&self, info: Option<PanelItemCursor>) {
		println!("Set cursor with info {:?}", info);
		self.cursor
			.update_info(&info, &self.item.upgrade().unwrap())
			.await;
	}
}
