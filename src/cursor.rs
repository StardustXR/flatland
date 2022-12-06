use crate::panel_ui::PPM;
use lazy_static::lazy_static;
use mint::Vector2;
use stardust_xr_molecules::fusion::{
	drawable::Model,
	items::panel::{PanelItem, PanelItemCursor},
	resource::NamespacedResource,
	spatial::Spatial,
};

lazy_static! {
	static ref CURSOR_RESOURCE: NamespacedResource = NamespacedResource::new("flatland", "cursor");
}

pub struct Cursor {
	root: Spatial,
	model: Model,
}
impl Cursor {
	pub fn new(parent: &Spatial) -> Cursor {
		let root = Spatial::builder()
			.spatial_parent(parent)
			.zoneable(false)
			.build()
			.unwrap();
		let model = Model::builder()
			.spatial_parent(&root)
			.resource(&*CURSOR_RESOURCE)
			// .scale(glam::vec3(0.0, 0.0, 0.0))
			.build()
			.unwrap();

		Cursor { root, model }
	}

	pub fn update_info(&self, info: &Option<PanelItemCursor>, item: &PanelItem) {
		if let Some(info) = info {
			self.model
				.set_transform(
					None,
					Some(
						(glam::vec3(-info.hotspot.x as f32, info.hotspot.y as f32, 0.0) / PPM)
							.into(),
					),
					None,
					Some((glam::vec3(info.size.x as f32, info.size.y as f32, 1.0) / PPM).into()),
				)
				.unwrap();
			item.apply_cursor_material(info, &self.model, 0).unwrap();
		} else {
			self.model
				.set_scale(None, glam::vec3(0.0, 0.0, 1.0))
				.unwrap();
		}
	}

	pub fn update_position(&self, size: Vector2<f32>, position: Vector2<f32>) {
		self.root
			.set_position(
				None,
				mint::Vector3::from([
					(-size.x * 0.5 + position.x) / PPM,
					(-size.y * 0.5 + position.y) / -PPM,
					0.006,
				]),
			)
			.unwrap();
	}
}
