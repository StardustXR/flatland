use color_eyre::eyre::{bail, Result};
use flatland::Flatland;
use manifest_dir_macros::directory_relative_path;
use stardust_xr_fusion::{
	client::Client,
	items::{
		panel::{PanelItemUi, PanelItemUiAspect},
		ItemUi, ItemUiAspect,
	},
	node::NodeType,
	root::RootAspect,
};
use tracing_subscriber::EnvFilter;

pub mod close_button;
pub mod flatland;
pub mod grab_ball;
pub mod panel_shell_transfer;
pub mod resize_handles;
pub mod surface;
pub mod toplevel;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	tracing_subscriber::fmt()
		.compact()
		.with_env_filter(EnvFilter::from_env("LOG_LEVEL"))
		.init();
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")])?;

	let flatland = client
		.get_root()
		.alias()
		.wrap(Flatland::new(&client).await)?;
	let item_ui_wrapped = PanelItemUi::register(&client)?.wrap_raw(flatland.wrapped().clone())?;
	<ItemUi as ItemUiAspect>::add_handlers(&item_ui_wrapped)?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => Ok(()),
		_ = event_loop => bail!("Server crashed"),
	}
}
