use crate::panel_ui::PanelItemUI;
use anyhow::Result;
use libstardustxr::fusion::{
	client::{Client, LifeCycleHandler, LogicStepInfo},
	item::{ItemUI, ItemUIType, PanelItem},
	WeakWrapped,
};
use parking_lot::Mutex;
use std::sync::{Arc, Weak};

pub struct Flatland {
	pub client: Arc<Client>,
	ui: ItemUI<PanelItem, PanelItemUI>,
	pub focused: Mutex<WeakWrapped<PanelItemUI>>,
}
impl Flatland {
	pub async fn new(client: Arc<Client>) -> Result<Arc<Self>> {
		let flatland = Arc::new_cyclic(|weak_flatland: &Weak<Flatland>| {
			let weak_flatland = weak_flatland.clone();
			let ui = ItemUI::<PanelItem, PanelItemUI>::register(
				&client,
				move |init_data, weak_wrapped, weak_item, item| {
					*weak_flatland.upgrade().unwrap().focused.lock() = weak_wrapped;
					PanelItemUI::new(init_data, weak_item, item)
				},
			)
			.unwrap();
			Flatland {
				client: client.clone(),
				ui,
				focused: Mutex::new(WeakWrapped::new()),
			}
		});

		Ok(flatland)
	}

	pub fn with_focused<F, O>(&self, f: F) -> Option<O>
	where
		F: FnOnce(&PanelItem) -> O,
	{
		self.focused
			.lock()
			.upgrade()
			.and_then(|ui| ui.lock().item.clone().with_node(|node| f(node)))
	}
}
impl LifeCycleHandler for Flatland {
	fn logic_step(&mut self, info: LogicStepInfo) {
		for (_id, wrapper) in &*self.ui.items() {
			wrapper.lock_inner().step(&info);
		}
	}
}
