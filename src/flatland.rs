use std::sync::Arc;

use crate::panel_ui::PanelItemUI;
use rustc_hash::FxHashMap;
use stardust_xr_molecules::fusion::{
	client::FrameInfo,
	items::{
		panel::{PanelItem, PanelItemInitData},
		ItemAcceptor, ItemAcceptorHandler, ItemUIHandler,
	},
	node::NodeType,
	HandlerWrapper,
};

pub struct Flatland {
	panel_items: FxHashMap<String, HandlerWrapper<PanelItem, PanelItemUI>>,
}
impl Flatland {
	pub fn new() -> Self {
		Flatland {
			panel_items: FxHashMap::default(),
		}
	}

	pub fn frame(&mut self, info: FrameInfo) {
		for item in self.panel_items.values() {
			item.lock_wrapped().frame(&info);
		}
		// let items = self.panel_items.items();
		// let focus = items
		// 	.iter()
		// 	.map(|(_, wrapper)| (wrapper, wrapper.lock_inner().step()))
		// 	.reduce(|a, b| if a.1 > b.1 { b } else { a });
		// if let Some((focus, _)) = focus {
		// 	self.focused = focus.weak_wrapped();
		// }
	}

	fn add_item(&mut self, uid: &str, item: PanelItem, init_data: PanelItemInitData) {
		let ui = PanelItemUI::new(init_data, item.alias());
		let handler = item.wrap(ui).unwrap();
		handler.lock_wrapped().mouse.lock_wrapped().panel_item_ui =
			Arc::downgrade(handler.wrapped());
		self.panel_items.insert(uid.to_string(), handler);
	}
	fn remove_item(&mut self, uid: &str) {
		self.panel_items.remove(uid);
	}
}
impl ItemUIHandler<PanelItem> for Flatland {
	fn item_created(&mut self, uid: &str, item: PanelItem, init_data: PanelItemInitData) {
		self.add_item(uid, item, init_data);
	}
	fn item_captured(&mut self, _uid: &str, _acceptor_uid: &str, _item: PanelItem) {}
	fn item_released(&mut self, _uid: &str, _acceptor_uid: &str, _item: PanelItem) {}
	fn item_destroyed(&mut self, uid: &str) {
		self.remove_item(uid);
	}
	fn acceptor_created(&mut self, _uid: &str, _acceptor: ItemAcceptor<PanelItem>) {}
	fn acceptor_destroyed(&mut self, _uid: &str) {}
}
impl ItemAcceptorHandler<PanelItem> for Flatland {
	fn captured(&mut self, uid: &str, item: PanelItem, init_data: PanelItemInitData) {
		self.add_item(uid, item, init_data);
	}
	fn released(&mut self, uid: &str) {
		self.remove_item(uid);
	}
}
