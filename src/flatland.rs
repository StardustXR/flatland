use crate::panel_ui::PanelItemUI;
use anyhow::Result;
use stardust_xr_fusion::{
	client::{Client, LifeCycleHandler, LogicStepInfo},
	items::{panel::PanelItem, ItemUI, ItemUIType},
	WeakWrapped,
};
use std::sync::Arc;

pub struct Flatland {
	pub client: Arc<Client>,
	ui: ItemUI<PanelItem, PanelItemUI>,
	pub focused: WeakWrapped<PanelItemUI>,
}
impl Flatland {
	pub async fn new(client: Arc<Client>) -> Result<Self> {
		let ui = ItemUI::<PanelItem, PanelItemUI>::register(
			&client,
			move |init_data, _, weak_item, item| PanelItemUI::new(init_data, weak_item, item),
		)
		.unwrap();
		Ok(Flatland {
			client: client.clone(),
			ui,
			focused: WeakWrapped::new(),
		})
	}

	pub fn with_focused<F, O>(&mut self, f: F) -> Option<O>
	where
		F: FnOnce(&PanelItem) -> O,
	{
		self.focused
			.upgrade()
			.and_then(|ui| ui.lock().item.clone().with_node(|node| f(node)))
	}
}
impl LifeCycleHandler for Flatland {
	fn logic_step(&mut self, _info: LogicStepInfo) {
		let items = self.ui.items();
		let focus = items
			.iter()
			.map(|(_, wrapper)| (wrapper.clone(), wrapper.lock_inner().step()))
			.reduce(|a, b| if a.1 > b.1 { b } else { a });
		if let Some((focus, _)) = focus {
			self.focused = focus.weak_wrapped();
		}
	}
}
