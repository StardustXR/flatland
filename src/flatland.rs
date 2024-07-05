use std::sync::Arc;

use crate::toplevel::Toplevel;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::Client,
	fields::Field,
	items::{
		panel::{
			PanelItem, PanelItemAcceptor, PanelItemAspect, PanelItemInitData, PanelItemUiHandler,
		},
		ItemUiHandler,
	},
	node::NodeType,
	objects::hmd,
	root::{ClientState, FrameInfo, RootHandler},
	spatial::SpatialRef,
	HandlerWrapper,
};

pub struct Flatland {
	hmd: SpatialRef,
	panel_items: FxHashMap<u64, HandlerWrapper<PanelItem, Toplevel>>,
	acceptors: FxHashMap<u64, (PanelItemAcceptor, Field)>,
}
impl Flatland {
	pub async fn new(client: &Arc<Client>) -> Self {
		let hmd = hmd(client).await.unwrap();
		Flatland {
			hmd,
			panel_items: FxHashMap::default(),
			acceptors: FxHashMap::default(),
		}
	}

	fn add_item(&mut self, item: PanelItem, init_data: PanelItemInitData) {
		let Ok(toplevel) = Toplevel::create(self.hmd.alias(), item.alias(), init_data) else {
			return;
		};
		let id = item.node().get_id().unwrap();
		let handler = item.wrap(toplevel).unwrap();
		self.panel_items.insert(id, handler);
	}
	fn remove_item(&mut self, id: u64) {
		self.panel_items.remove(&id);
	}
}

impl PanelItemUiHandler for Flatland {
	fn create_item(&mut self, item: PanelItem, init_data: PanelItemInitData) {
		self.add_item(item, init_data);
	}
	fn create_acceptor(&mut self, acceptor: PanelItemAcceptor, field: Field) {
		self.acceptors
			.insert(acceptor.node().get_id().unwrap(), (acceptor, field));
	}
}
impl ItemUiHandler for Flatland {
	fn capture_item(&mut self, item_id: u64, _acceptor_id: u64) {
		let Some(toplevel) = self.panel_items.get(&item_id) else {
			return;
		};
		toplevel.lock_wrapped().set_enabled(false);
	}
	fn release_item(&mut self, item_id: u64, _acceptor_id: u64) {
		let Some(toplevel) = self.panel_items.get(&item_id) else {
			return;
		};
		toplevel.lock_wrapped().set_enabled(true);
	}
	fn destroy_item(&mut self, id: u64) {
		self.remove_item(id);
	}
	fn destroy_acceptor(&mut self, id: u64) {
		self.acceptors.remove(&id);
	}
}
// impl ItemAcceptorHandler<PanelItem> for Flatland {
// 	fn captured(&mut self, id: u64, item: PanelItem, init_data: PanelItemInitData) {
// 		self.add_item(uid, item, init_data);
// 	}
// 	fn released(&mut self, id: u64) {
// 		self.remove_item(uid);
// 	}
// }
impl RootHandler for Flatland {
	fn frame(&mut self, info: FrameInfo) {
		for item in self.panel_items.values() {
			item.lock_wrapped().update(&info, &self.acceptors);
		}
	}

	fn save_state(&mut self) -> color_eyre::eyre::Result<ClientState> {
		Ok(ClientState::default())
	}
}
