use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::FrameInfo,
	fields::UnknownField,
	items::{
		panel::{PanelItem, PanelItemInitData},
		ItemAcceptor, ItemAcceptorHandler, ItemUIHandler,
	},
	node::NodeType,
	HandlerWrapper,
};

use crate::toplevel::Toplevel;

pub struct Flatland {
	panel_items: FxHashMap<String, HandlerWrapper<PanelItem, Toplevel>>,
}
impl Flatland {
	pub fn new() -> Self {
		Flatland {
			panel_items: FxHashMap::default(),
		}
	}

	pub fn frame(
		&mut self,
		info: FrameInfo,
		acceptors: &FxHashMap<String, (ItemAcceptor<PanelItem>, UnknownField)>,
	) {
		for item in self.panel_items.values() {
			item.lock_wrapped().update(&info, acceptors);
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
		let Ok(toplevel) = Toplevel::create(item.alias(), init_data) else {return};
		let handler = item.wrap(toplevel).unwrap();
		// handler.lock_wrapped().mouse.lock_wrapped().panel_item_ui =
		// 	Arc::downgrade(handler.wrapped());
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
	fn item_destroyed(&mut self, uid: &str) {
		self.remove_item(uid);
	}

	fn item_captured(&mut self, uid: &str, _acceptor_uid: &str, _item: PanelItem) {
		let Some(toplevel) = self.panel_items.get(uid) else {return};
		toplevel.lock_wrapped().set_enabled(false);
	}
	fn item_released(&mut self, uid: &str, _acceptor_uid: &str, _item: PanelItem) {
		let Some(toplevel) = self.panel_items.get(uid) else {return};
		toplevel.lock_wrapped().set_enabled(true);
	}
}
impl ItemAcceptorHandler<PanelItem> for Flatland {
	fn captured(&mut self, uid: &str, item: PanelItem, init_data: PanelItemInitData) {
		self.add_item(uid, item, init_data);
	}
	fn released(&mut self, uid: &str) {
		self.remove_item(uid);
	}
}
