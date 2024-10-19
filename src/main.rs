use crate::toplevel::ToplevelInner;
use ashpd::desktop::settings::Settings;
use asteroids::{custom::ElementTrait, elements::Spatial, Reify, View};
use resize_handles::ResizeHandlesElement;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::Client,
	fields::Field,
	items::{
		panel::{
			PanelItem, PanelItemAcceptor, PanelItemUi, PanelItemUiAspect, PanelItemUiEvent,
			ToplevelInfo,
		},
		ItemUiAspect, ItemUiEvent,
	},
	node::NodeType,
	project_local_resources,
	root::{RootAspect, RootEvent},
	values::{color::rgba_linear, Color},
};
use tracing_subscriber::EnvFilter;

pub mod close_button;
pub mod grab_ball;
pub mod panel_shell_transfer;
pub mod resize_handles;
pub mod surface;
pub mod toplevel;

async fn accent_color() -> color_eyre::eyre::Result<Color> {
	let accent_color = Settings::new().await?.accent_color().await?;
	Ok(rgba_linear!(
		accent_color.red() as f32,
		accent_color.green() as f32,
		accent_color.blue() as f32,
		1.0
	))
}

// pub struct Flatland {
// 	accent_color: Color,
// 	hmd: SpatialRef,
// 	panel_items: FxHashMap<u64, HandlerWrapper<PanelItem, Toplevel>>,
// 	acceptors: FxHashMap<u64, (PanelItemAcceptor, Field)>,
// }
// impl Flatland {
// 	pub async fn new(client: &Client) -> Self {
// 		let accent_color = accent_color()
// 			.await
// 			.unwrap_or(rgba_linear!(0.0, 0.75, 1.0, 1.0));
// 		let hmd = hmd(client).await.unwrap();

// 		Flatland {
// 			accent_color,
// 			hmd,
// 			panel_items: FxHashMap::default(),
// 			acceptors: FxHashMap::default(),
// 		}
// 	}

// 	fn add_item(&mut self, item: PanelItem, init_data: PanelItemInitData) {
// 		let Ok(toplevel) =
// 			Toplevel::create(self.accent_color, self.hmd.alias(), item.alias(), init_data)
// 		else {
// 			return;
// 		};
// 		let id = item.node().get_id().unwrap();
// 		let handler = item.wrap(toplevel).unwrap();
// 		self.panel_items.insert(id, handler);
// 	}
// 	fn remove_item(&mut self, id: u64) {
// 		self.panel_items.remove(&id);
// 	}
// }

// impl PanelItemUiHandler for Flatland {
// 	fn create_item(&mut self, item: PanelItem, init_data: PanelItemInitData) {
// 		self.add_item(item, init_data);
// 	}
// 	fn create_acceptor(&mut self, acceptor: PanelItemAcceptor, field: Field) {
// 		self.acceptors
// 			.insert(acceptor.node().get_id().unwrap(), (acceptor, field));
// 	}
// }
// impl ItemUiHandler for Flatland {
// 	fn capture_item(&mut self, item_id: u64, _acceptor_id: u64) {
// 		let Some(toplevel) = self.panel_items.get(&item_id) else {
// 			return;
// 		};
// 		toplevel.lock_wrapped().set_enabled(false);
// 	}
// 	fn release_item(&mut self, item_id: u64, _acceptor_id: u64) {
// 		let Some(toplevel) = self.panel_items.get(&item_id) else {
// 			return;
// 		};
// 		toplevel.lock_wrapped().set_enabled(true);
// 	}
// 	fn destroy_item(&mut self, id: u64) {
// 		self.remove_item(id);
// 	}
// 	fn destroy_acceptor(&mut self, id: u64) {
// 		self.acceptors.remove(&id);
// 	}
// }
// impl ItemAcceptorHandler<PanelItem> for Flatland {
// 	fn captured(&mut self, id: u64, item: PanelItem, init_data: PanelItemInitData) {
// 		self.add_item(uid, item, init_data);
// 	}
// 	fn released(&mut self, id: u64) {
// 		self.remove_item(uid);
// 	}
// }
// impl RootHandler for Flatland {
// 	fn frame(&mut self, info: FrameInfo) {
// 		for item in self.panel_items.values() {
// 			item.lock_wrapped().update(&info, &self.acceptors);
// 		}
// 	}

// 	fn save_state(&mut self) -> color_eyre::eyre::Result<ClientState> {
// 		Ok(ClientState::default())
// 	}
// }

#[derive(Debug, Default)]
pub struct State {
	toplevels: FxHashMap<u64, ToplevelState>,
}
impl Reify for State {
	fn reify(&self) -> asteroids::Element<Self> {
		Spatial::default().with_children(
			self.toplevels
				.iter()
				.map(|(uid, t)| (*uid, t.reify()))
				.map(|(uid, t)| t.map(move |s: &mut Self| s.toplevels.get_mut(&uid).unwrap())),
		)
	}
}

#[derive(Debug)]
pub struct ToplevelState {
	accent_color: Color,
	panel_item: PanelItem,
	info: ToplevelInfo,
	density: f32, //pixels per meter
}
impl Reify for ToplevelState {
	fn reify(&self) -> asteroids::Element<Self> {
		let spatial_ref = self
			.panel_item
			.clone()
			.as_item()
			.as_spatial()
			.as_spatial_ref();
		ResizeHandlesElement {
			initial_position: spatial_ref,
			accent_color: rgba_linear!(1.0, 1.0, 1.0, 1.0),
			initial_size: [
				self.info.size.x as f32 / self.density,
				self.info.size.y as f32 / self.density,
			]
			.into(),
			min_size_px: self.info.min_size,
			max_size_px: self.info.max_size,
			on_size_changed: Some(|state: &mut ToplevelState, size_meters| {
				state.info.size = [
					(size_meters.x * state.density) as u32,
					(size_meters.y * state.density) as u32,
				]
				.into();
			}),
		}
		.build()
	}
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
	tracing_subscriber::fmt()
		.compact()
		.with_env_filter(EnvFilter::from_default_env())
		.init();
	let mut client = Client::connect().await.unwrap();
	client
		.setup_resources(&[&project_local_resources!("res")])
		.unwrap();

	let accent_color = accent_color()
		.await
		.unwrap_or(rgba_linear!(0.0, 0.75, 1.0, 1.0));

	let mut panel_items: FxHashMap<u64, ToplevelInner> = Default::default();
	let mut acceptors: FxHashMap<u64, (PanelItemAcceptor, Field)> = Default::default();

	let item_ui = PanelItemUi::register(&client.handle()).unwrap();

	let mut state = State::default();
	let mut asteroids_view = View::new(&state, client.handle().get_root());

	client
		.event_loop(|client, _| {
			while let Some(item_ui_event) = item_ui.recv_panel_item_ui_event() {
				match item_ui_event {
					PanelItemUiEvent::CreateItem { item, initial_data } => {
						let id = item.node().id();
						println!("awa nyew panel item with id:{id}");
						// let toplevel = Toplevel::create(accent_color, item, initial_data);
						// if let Ok(toplevel) = toplevel {
						// panel_items.insert(id, toplevel);
						// }
						state.toplevels.insert(
							id,
							ToplevelState {
								panel_item: item,
								info: initial_data.toplevel,
								density: 3000.0,
								accent_color,
							},
						);
					}
					PanelItemUiEvent::CreateAcceptor {
						acceptor,
						acceptor_field,
					} => {
						let id = acceptor.node().id();
						acceptors.insert(id, (acceptor, acceptor_field));
					}
				}
			}
			while let Some(item_ui_event) = item_ui.recv_item_ui_event() {
				match item_ui_event {
					ItemUiEvent::CaptureItem {
						item_id,
						acceptor_id: _,
					} => {
						let Some(toplevel) = panel_items.get_mut(&item_id) else {
							return;
						};
						toplevel.set_enabled(false);
					}
					ItemUiEvent::ReleaseItem {
						item_id,
						acceptor_id: _,
					} => {
						let Some(toplevel) = panel_items.get_mut(&item_id) else {
							return;
						};
						toplevel.set_enabled(true);
					}
					ItemUiEvent::DestroyItem { id } => {
						// panel_items.remove(&id);
						state.toplevels.remove(&id);
					}
					ItemUiEvent::DestroyAcceptor { id } => {
						acceptors.remove(&id);
					}
				}
			}

			for panel_item in panel_items.values_mut() {
				panel_item.handle_events(&acceptors);
			}
			while let Some(RootEvent::Frame { info }) = client.get_root().recv_root_event() {
				asteroids_view.frame(&info);
				asteroids_view.update(&mut state);
				// for panel_item in panel_items.values_mut() {
				// 	panel_item.frame(&info)
				// }
			}
		})
		.await
		.unwrap();
}
