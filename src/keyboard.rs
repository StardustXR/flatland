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
	keyboard::{KeyboardEvent, KEYBOARD_MASK},
};

pub struct Keyboard {
	pub panel_item: Option<PanelItem>,
	keys: Vec<u32>,
}
impl Keyboard {
	pub fn new<Fi: Field>(
		spatial_parent: &Spatial,
		transform: Transform,
		field: &Fi,
		panel_item: Option<PanelItem>,
	) -> Result<HandlerWrapper<PulseReceiver, Keyboard>, NodeError> {
		PulseReceiver::create(spatial_parent, transform, field, &KEYBOARD_MASK)?.wrap(Keyboard {
			panel_item,
			keys: Vec::new(),
		})
	}
}
impl PulseReceiverHandler for Keyboard {
	fn data(&mut self, _uid: &str, data: &[u8], _data_reader: flexbuffers::MapReader<&[u8]>) {
		if let Some(keyboard_event) = KeyboardEvent::from_pulse_data(data) {
			if let Some(panel_item) = &self.panel_item {
				let _ = keyboard_event.send_to_panel(panel_item);
			}
			for key_down in keyboard_event.keys_down.clone().unwrap_or_default() {
				if !self.keys.contains(&key_down) {
					self.keys.push(key_down);
				}
			}
			let keys_up = keyboard_event.keys_down.clone().unwrap_or_default();
			self.keys.retain_mut(|key| !keys_up.contains(key));

			if self.keys.is_empty() {
				if let Some(panel_item) = &self.panel_item {
					let _ = panel_item.keyboard_deactivate();
				}
			}
		}
	}
}
