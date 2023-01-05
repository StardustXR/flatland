use parking_lot::Mutex;
use stardust_xr_molecules::{
	fusion::{
		core::values::Transform,
		data::{PulseReceiver, PulseReceiverHandler},
		fields::Field,
		items::panel::PanelItem,
		node::NodeError,
		spatial::Spatial,
		HandlerWrapper,
	},
	mouse::{MouseEvent, MOUSE_MASK},
};
use std::sync::Weak;

use crate::panel_ui::PanelItemUI;

pub struct Mouse {
	pub panel_item: Option<PanelItem>,
	pub panel_item_ui: Weak<Mutex<PanelItemUI>>,
}
impl Mouse {
	pub fn new<Fi: Field>(
		spatial_parent: &Spatial,
		transform: Transform,
		field: &Fi,
		panel_item: Option<PanelItem>,
		panel_item_ui: Weak<Mutex<PanelItemUI>>,
	) -> Result<HandlerWrapper<PulseReceiver, Mouse>, NodeError> {
		PulseReceiver::create(spatial_parent, transform, field, &MOUSE_MASK)?.wrap(Mouse {
			panel_item,
			panel_item_ui,
		})
	}
}
impl PulseReceiverHandler for Mouse {
	fn data(&mut self, _uid: &str, data: &[u8], _data_reader: flexbuffers::MapReader<&[u8]>) {
		if let Some(mouse_event) = MouseEvent::from_pulse_data(data) {
			if let Some(panel_item) = &self.panel_item {
				let _ = mouse_event.send_to_panel(panel_item);
			}
			if let Some(delta) = mouse_event.delta {
				if let Some(panel_item_ui) = self.panel_item_ui.upgrade() {
					panel_item_ui.lock().pointer_delta(delta);
				}
			}
		}
	}
}
