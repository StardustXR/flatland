use stardust_xr_fusion::{
	core::{schemas::flex::flexbuffers, values::Transform},
	data::{PulseReceiver, PulseReceiverHandler},
	fields::Field,
	items::panel::PanelItem,
	node::NodeError,
	spatial::Spatial,
	HandlerWrapper,
};
use stardust_xr_molecules::keyboard::{KeyboardEvent, KEYBOARD_MASK};
use tracing::debug;

pub struct Keyboard {
	pub panel_item: Option<PanelItem>,
}
impl Keyboard {
	pub fn new<Fi: Field>(
		spatial_parent: &Spatial,
		transform: Transform,
		field: &Fi,
		panel_item: Option<PanelItem>,
	) -> Result<HandlerWrapper<PulseReceiver, Keyboard>, NodeError> {
		PulseReceiver::create(spatial_parent, transform, field, &KEYBOARD_MASK)?
			.wrap(Keyboard { panel_item })
	}
}
impl PulseReceiverHandler for Keyboard {
	fn data(&mut self, _uid: &str, data: &[u8], _data_reader: flexbuffers::MapReader<&[u8]>) {
		let Some(keyboard_event) = KeyboardEvent::from_pulse_data(data) else {return};
		debug!(?keyboard_event, "Keyboard event");
		if let Some(panel_item) = &self.panel_item {
			let _ = keyboard_event.send_to_panel(panel_item);
		}
	}
}
