use crate::toplevel::ToplevelInner;
use ashpd::desktop::settings::Settings;
use asteroids::{custom::ElementTrait, elements::Spatial, Reify, View};
use close_button::ExposureButton;
use resize_handles::ResizeHandlesElement;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::Client,
	fields::Field,
	items::{
		panel::{
			ChildInfo, PanelItem, PanelItemAcceptor, PanelItemAspect, PanelItemUi,
			PanelItemUiAspect, PanelItemUiEvent, SurfaceId, ToplevelInfo,
		},
		ItemUiAspect, ItemUiEvent,
	},
	node::NodeType,
	project_local_resources,
	root::{RootAspect, RootEvent},
	spatial::Transform,
	values::{color::rgba_linear, Color},
};
use surface_v2::{ChildSurfaceData, SurfaceElement};
use toplevel::{CHILD_THICKNESS, TOPLEVEL_THICKNESS};
use tracing::info;
use tracing_subscriber::EnvFilter;

pub mod close_button;
pub mod grab_ball;
pub mod panel_shell_transfer;
pub mod resize_handles;
pub mod surface;
pub mod surface_input;
pub mod surface_v2;
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
	children: Vec<ChildInfo>,
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
		let handles = ResizeHandlesElement {
			initial_position: spatial_ref,
			accent_color: rgba_linear!(1.0, 1.0, 1.0, 1.0),
			initial_size: [
				self.info.size.x as f32 / self.density,
				self.info.size.y as f32 / self.density,
			]
			.into(),
			min_size: self
				.info
				.min_size
				.map(|v| [v.x / self.density, v.y / self.density].into()),
			max_size: self
				.info
				.max_size
				.map(|v| [v.x / self.density, v.y / self.density].into()),
			on_size_changed: Some(|state: &mut ToplevelState, size_meters| {
				state.info.size = [
					(size_meters.x * state.density) as u32,
					(size_meters.y * state.density) as u32,
				]
				.into();
			}),
		};
		let mut children = vec![ExposureButton::<Self> {
			transform: Transform::from_translation([
				(self.info.size.x as f32 / (self.density * 2.0)),
				-(self.info.size.y as f32 / (self.density * 2.0)),
				0.0,
			]),
			thickness: 0.01,
			on_click: Box::new(|state| {
				state.panel_item.close_toplevel().unwrap();
			}),
		}
		.build()];
		if self.info.parent.is_none() {
			children.push(
				SurfaceElement {
					initial_resolution: self.info.size,
					receives_input: true,
					item: self.panel_item.clone(),
					id: SurfaceId::Toplevel(()),
					density: self.density,
					thickness: TOPLEVEL_THICKNESS,
					child_thickness: CHILD_THICKNESS,
					children: self
						.children
						.iter()
						.map(|v| ChildSurfaceData {
							id: SurfaceId::Child(v.id),
							geometry: v.geometry.clone(),
						})
						.collect(),
					highlight_color: self.accent_color.c,
					parent_thickness: None,
				}
				.build(),
			)
		}
		handles.with_children(children)
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
					PanelItemUiEvent::CreateItem {
						item,
						mut initial_data,
					} => {
						let id = item.node().id();
						println!(
							"awa nyew panel item with id:{id}, size:{:?}",
							initial_data.toplevel.size
						);
						// The server returns size [0,0] for new toplevles,
						// toplevels already open when flatland starts work
						// as expected
						if let Some(v) = initial_data.toplevel.min_size {
							initial_data.toplevel.size = [
								initial_data.toplevel.size.x.max(v.x as u32),
								initial_data.toplevel.size.y.max(v.y as u32),
							]
							.into();
						}
						item.set_toplevel_size(initial_data.toplevel.size);
						state.toplevels.insert(
							id,
							ToplevelState {
								panel_item: item,
								info: initial_data.toplevel,
								density: 3000.0,
								children: initial_data.children,
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
