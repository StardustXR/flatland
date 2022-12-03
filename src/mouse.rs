use std::sync::Arc;

use mint::Vector3;
use parking_lot::Mutex;
use stardust_xr_molecules::{
	fusion::{
		data::{PulseReceiver, PulseReceiverHandler},
		fields::Field,
		items::panel::PanelItem,
		node::{ClientOwned, NodeError},
		spatial::Spatial,
		HandlerWrapper,
	},
	mouse::{MouseEvent, MOUSE_MASK},
};

use crate::panel_ui::PanelItemUI;

pub struct Mouse {
	pub panel_item: Option<PanelItem>,
	pub panel_item_ui: Option<Arc<Mutex<PanelItemUI>>>,
}
impl Mouse {
	pub fn new<Fi: Field + ClientOwned>(
		spatial_parent: &Spatial,
		field: &Fi,
		position: Option<Vector3<f32>>,
		panel_item: Option<PanelItem>,
		panel_item_ui: Option<Arc<Mutex<PanelItemUI>>>,
	) -> Result<HandlerWrapper<PulseReceiver, Mouse>, NodeError> {
		PulseReceiver::create(spatial_parent, position, None, field, MOUSE_MASK.clone())?.wrap(
			Mouse {
				panel_item,
				panel_item_ui,
			},
		)
	}
}
impl PulseReceiverHandler for Mouse {
	fn data(&mut self, _uid: &str, data: &[u8], _data_reader: flexbuffers::MapReader<&[u8]>) {
		if let Some(mouse_event) = MouseEvent::from_pulse_data(data) {
			if let Some(panel_item) = &self.panel_item {
				let _ = mouse_event.send_to_panel(panel_item);
			}
			if let Some(delta) = mouse_event.delta {
				if let Some(panel_item_ui) = &self.panel_item_ui {
					panel_item_ui.lock().pointer_delta(delta);
				}
			}
		}
	}
}
