use crate::panel_ui::PanelItemUI;
use anyhow::Result;
use libstardustxr::fusion::{
	async_trait,
	client::{Client, LifeCycleHandler, LogicStepInfo},
	item::{ItemUI, ItemUIHandler, PanelItem, PanelItemInitData},
};
use parking_lot::Mutex;
use rustc_hash::FxHashMap;
use std::{
	future::Future,
	sync::{Arc, Weak},
};

pub struct Flatland {
	pub client: Arc<Client>,
	ui: Arc<ItemUI<PanelItem>>,
	pub panels: Mutex<FxHashMap<String, Arc<PanelItemUI>>>,
	pub focused: Mutex<Weak<PanelItemUI>>,
}
impl Flatland {
	pub async fn new(client: Arc<Client>) -> Result<Arc<Self>> {
		let ui = ItemUI::register(&client).await?;
		let panels = Mutex::default();

		let flatland = Arc::new(Flatland {
			client: client.clone(),
			ui,
			panels,
			focused: Mutex::new(Weak::new()),
		});
		client.set_life_cycle_handler(&flatland);
		flatland.ui.set_handler(&flatland);

		Ok(flatland)
	}

	pub fn on_focused<F, O>(&self, closure: F)
	where
		F: FnOnce(Arc<PanelItemUI>) -> O + Send + 'static,
		O: Future + Send,
	{
		let maybe_focused = self.focused.lock().clone();
		tokio::task::spawn(async move {
			if let Some(focused) = maybe_focused.upgrade() {
				closure(focused).await;
			}
		});
	}
}
#[async_trait]
impl LifeCycleHandler for Flatland {
	async fn logic_step(&self, info: LogicStepInfo) {
		let panels: Vec<Arc<PanelItemUI>> = self
			.panels
			.lock()
			.iter()
			.map(|(_, panel)| panel.clone())
			.collect();
		for panel in panels {
			panel.step(&info).await;
		}
	}
}

#[async_trait]
impl ItemUIHandler<PanelItem> for Flatland {
	async fn create(&self, item_id: &str, item: &Arc<PanelItem>, init_data: PanelItemInitData) {
		println!("Panel item {item_id} created with {:#?}", init_data);
		let panel_ui = PanelItemUI::new(item, init_data).await;

		*self.focused.lock() = Arc::downgrade(&panel_ui);
		self.panels.lock().insert(item_id.to_string(), panel_ui);
	}
	async fn destroy(&self, item_id: &str) {
		println!("Panel item {item_id} destroyed");

		self.panels.lock().remove(item_id).unwrap();
	}
}
