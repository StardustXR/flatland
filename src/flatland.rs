use crate::toplevel::Toplevel;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::{ClientState, FrameInfo, RootHandler},
	fields::UnknownField,
	items::{
		panel::{PanelItem, PanelItemInitData},
		ItemAcceptor, ItemUIHandler,
	},
	node::NodeType,
	spatial::Spatial,
	HandlerWrapper,
};

pub struct Flatland {
	root: Spatial,
	panel_items: FxHashMap<String, HandlerWrapper<PanelItem, Toplevel>>,
	acceptors: FxHashMap<String, (ItemAcceptor<PanelItem>, UnknownField)>,
}
impl Flatland {
	pub fn new(root: &Spatial) -> Self {
		Flatland {
			root: root.alias(),
			panel_items: FxHashMap::default(),
			acceptors: FxHashMap::default(),
		}
	}

	fn add_item(&mut self, uid: String, item: PanelItem, init_data: PanelItemInitData) {
		let Ok(toplevel) = Toplevel::create(item.alias(), init_data) else {
			return;
		};
		let handler = item.wrap(toplevel).unwrap();
		self.panel_items.insert(uid, handler);
	}
	fn remove_item(&mut self, uid: &str) {
		self.panel_items.remove(uid);
	}
}
impl ItemUIHandler<PanelItem> for Flatland {
	fn item_created(&mut self, uid: String, item: PanelItem, init_data: PanelItemInitData) {
		self.add_item(uid, item, init_data);
	}
	fn item_destroyed(&mut self, uid: String) {
		self.remove_item(&uid);
	}

	fn item_captured(&mut self, uid: String, _acceptor_uid: String) {
		let Some(toplevel) = self.panel_items.get(&uid) else {
			return;
		};
		toplevel.lock_wrapped().set_enabled(false);
	}
	fn item_released(&mut self, uid: String, _acceptor_uid: String) {
		let Some(toplevel) = self.panel_items.get(&uid) else {
			return;
		};
		toplevel.lock_wrapped().set_enabled(true);
	}

	fn acceptor_created(
		&mut self,
		acceptor_uid: String,
		acceptor: ItemAcceptor<PanelItem>,
		field: UnknownField,
	) {
		self.acceptors
			.insert(acceptor_uid.to_string(), (acceptor, field));
	}

	fn acceptor_destroyed(&mut self, acceptor_uid: String) {
		self.acceptors.remove(&acceptor_uid);
	}
}
// impl ItemAcceptorHandler<PanelItem> for Flatland {
// 	fn captured(&mut self, uid: String, item: PanelItem, init_data: PanelItemInitData) {
// 		self.add_item(uid, item, init_data);
// 	}
// 	fn released(&mut self, uid: String) {
// 		self.remove_item(uid);
// 	}
// }
impl RootHandler for Flatland {
	fn frame(&mut self, info: FrameInfo) {
		for item in self.panel_items.values() {
			item.lock_wrapped().update(&info, &self.acceptors);
		}
	}

	fn save_state(&mut self) -> ClientState {
		ClientState::from_root(&self.root)
	}
}
