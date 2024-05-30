use crate::toplevel::Toplevel;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::{ClientState, FrameInfo, RootHandler},
	fields::Field,
	items::{
		panel::{
			PanelItem, PanelItemAcceptor, PanelItemAspect, PanelItemInitData, PanelItemUiHandler,
		},
		ItemUiHandler,
	},
	node::NodeType,
	spatial::Spatial,
	HandlerWrapper,
};

pub struct Flatland {
	root: Spatial,
	panel_items: FxHashMap<String, HandlerWrapper<PanelItem, Toplevel>>,
	acceptors: FxHashMap<String, (PanelItemAcceptor, Field)>,
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

impl PanelItemUiHandler for Flatland {
	fn create_item(&mut self, uid: String, item: PanelItem, init_data: PanelItemInitData) {
		self.add_item(uid, item, init_data);
	}
	fn create_acceptor(&mut self, acceptor_uid: String, acceptor: PanelItemAcceptor, field: Field) {
		self.acceptors
			.insert(acceptor_uid.to_string(), (acceptor, field));
	}
}
impl ItemUiHandler for Flatland {
	fn capture_item(&mut self, item_uid: String, _acceptor_uid: String) {
		let Some(toplevel) = self.panel_items.get(&item_uid) else {
			return;
		};
		toplevel.lock_wrapped().set_enabled(false);
	}
	fn release_item(&mut self, item_uid: String, _acceptor_uid: String) {
		let Some(toplevel) = self.panel_items.get(&item_uid) else {
			return;
		};
		toplevel.lock_wrapped().set_enabled(true);
	}
	fn destroy_item(&mut self, uid: String) {
		self.remove_item(&uid);
	}
	fn destroy_acceptor(&mut self, uid: String) {
		self.acceptors.remove(&uid);
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
