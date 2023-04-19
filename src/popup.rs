use stardust_xr_fusion::{
	core::values::Transform,
	drawable::Model,
	items::panel::{PanelItem, PopupInfo, SurfaceID},
	node::NodeError,
	spatial::Spatial,
};

use crate::panel_ui::PANEL_RESOURCE;

pub struct Popup {
	parent: Spatial,
	root: Spatial,
	item: PanelItem,
	id: SurfaceID,
	model: Model,
}
impl Popup {
	pub fn new(
		parent: Spatial,
		item: PanelItem,
		uid: &str,
		info: PopupInfo,
	) -> Result<Self, NodeError> {
		info.positioner_data.anchor
		let root = Spatial::create(&parent, Transform::default(), false)?;
		let model = Model::create(&root, Transform::default(), &PANEL_RESOURCE)?;

		Ok(Popup {
			parent,
			root,
			item,
			id: SurfaceID::Popup(uid.to_string()),
			model,
		})
	}

	pub fn reposition(&self, info: PopupInfo) {}
}
