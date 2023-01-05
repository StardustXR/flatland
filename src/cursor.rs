use crate::panel_ui::PPM;
use lazy_static::lazy_static;
use mint::{Vector2, Vector3};
use stardust_xr_molecules::fusion::{
	core::values::Transform,
	drawable::Model,
	items::panel::{CursorInfo, PanelItem},
	resource::NamespacedResource,
	spatial::Spatial,
};

lazy_static! {
	static ref CURSOR_RESOURCE: NamespacedResource = NamespacedResource::new("flatland", "cursor");
}

pub struct Cursor {
	root: Spatial,
	model: Model,
	pub pos: Vector2<f32>,
}
impl Cursor {
	pub fn new(parent: &Spatial, info: &Option<CursorInfo>, item: &PanelItem) -> Cursor {
		let root = Spatial::create(parent, Transform::default(), false).unwrap();
		let model = Model::create(
			&root,
			Transform::from_scale(Vector3::from([0.1; 3])),
			&*CURSOR_RESOURCE,
		)
		.unwrap();
		dbg!(info);
		if let Some(info) = info {
			model
				.set_transform(
					None,
					Transform::from_position_scale(
						[
							-info.hotspot.x as f32 / PPM,
							info.hotspot.y as f32 / PPM,
							0.0,
						],
						[info.size.x as f32 / PPM, info.size.y as f32 / PPM, 1.0],
					),
				)
				.unwrap();
			item.apply_cursor_material(&model, 0).unwrap();
		}

		Cursor {
			root,
			model,
			pos: Vector2::from([0.0, 0.0]),
		}
	}

	pub fn update_info(&self, cursor_info: &Option<CursorInfo>, item: &PanelItem) {
		// dbg!(&cursor_info);
		if let Some(cursor_info) = cursor_info {
			self.model
				.set_transform(
					None,
					Transform::from_position_scale(
						[
							-cursor_info.hotspot.x as f32 / PPM,
							cursor_info.hotspot.y as f32 / PPM,
							0.0,
						],
						[
							cursor_info.size.x as f32 / PPM,
							cursor_info.size.y as f32 / PPM,
							1.0,
						],
					),
				)
				.unwrap();
			item.apply_cursor_material(&self.model, 0).unwrap();
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
					0.01,
				]),
			)
			.unwrap();
	}
}
