use color_eyre::eyre::{bail, Result};
use flatland::Flatland;
use manifest_dir_macros::directory_relative_path;
use stardust_xr_fusion::{
	client::{Client, FrameInfo, RootHandler},
	items::{panel::PanelItem, ItemUI},
	HandlerWrapper,
};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

pub mod cursor;
pub mod flatland;
pub mod mouse;
pub mod panel_ui;
pub mod surface;
pub mod toplevel;
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
impl RootHandler for Root {
	fn frame(&mut self, info: FrameInfo) {
		self.flatland.lock_wrapped().frame(info);
	}
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	tracing_subscriber::fmt()
		.compact()
		.with_env_filter(EnvFilter::from_env("LOG_LEVEL"))
		.init();
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _wrapped_root = client.wrap_root(Root::new(client.clone())?);

	tokio::select! {
		_ = tokio::signal::ctrl_c() => Ok(()),
		_ = event_loop => bail!("Server crashed"),
	}
}
