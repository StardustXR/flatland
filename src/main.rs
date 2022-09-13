use anyhow::{anyhow, Result};
use flatland::Flatland;
use input_window::InputWindow;
use libstardustxr::fusion::client::Client;
use manifest_dir_macros::directory_relative_path;
use std::thread;
use tokio::{runtime::Handle, sync::oneshot};
use winit::{event_loop::EventLoopBuilder, platform::unix::EventLoopBuilderExtUnix};

mod cursor;
mod flatland;
mod input_window;
mod panel_ui;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	let (client, stardust_event_loop) = Client::connect_with_async_loop().await?;
	client
		.set_base_prefixes(&[directory_relative_path!("res")])
		.await?;

	let tokio_handle = Handle::current();
	let flatland = Flatland::new(client).await?;
	let (winit_stop_tx, mut winit_stop_rx) = oneshot::channel::<()>();
	let winit_thread = thread::Builder::new().name("winit".to_owned()).spawn({
		let flatland = flatland.clone();
		move || -> Result<()> {
			let _tokio_guard = tokio_handle.enter();
			let event_loop = EventLoopBuilder::new()
				.with_any_thread(true)
				.with_x11()
				.build();
			let mut input_window = InputWindow::new(&event_loop, flatland)?;

			event_loop.run(move |event, _, control_flow| {
				match winit_stop_rx.try_recv() {
					Ok(_) => {
						control_flow.set_exit();
						return;
					}
					Err(ref e) if *e == oneshot::error::TryRecvError::Closed => {
						return;
					}
					_ => (),
				}

				input_window.handle_event(event);
			});
		}
	})?;

	let result = stardust_event_loop
		.await
		.map_err(|_| anyhow!("Server disconnected"));

	winit_stop_tx
		.send(())
		.expect("Failed to send stop signal to winit thread");
	winit_thread.join().expect("Couldn't rejoin winit thread")?;

	result
}
