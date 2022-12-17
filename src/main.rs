use anyhow::Result;
use flatland::Flatland;
use manifest_dir_macros::directory_relative_path;
use stardust_xr_molecules::fusion::{
	client::{Client, LifeCycleHandler, LogicStepInfo},
	items::{ItemUI, PanelItem},
	HandlerWrapper,
};
use std::sync::Arc;

pub mod cursor;
pub mod flatland;
pub mod panel_ui;
// pub mod resize_handle;
pub mod keyboard;
pub mod mouse;
pub mod util;

struct Root {
	flatland: HandlerWrapper<ItemUI<PanelItem>, Flatland>,
}
impl Root {
	fn new(client: Arc<Client>) -> Result<Self> {
		let flatland = ItemUI::register(&client)?.wrap(Flatland::new())?;
		Ok(Root { flatland })
	}
}
impl LifeCycleHandler for Root {
	fn logic_step(&mut self, info: LogicStepInfo) {
		self.flatland.lock_wrapped().logic_step(info);
	}
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _wrapped_root = client.wrap_root(Root::new(client.clone())?);

	tokio::select! {
		_ = tokio::signal::ctrl_c() => Ok(()),
		_ = event_loop => Err(anyhow::anyhow!("Server crashed")),
	}
}
