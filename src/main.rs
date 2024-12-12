use ashpd::desktop::settings::Settings;
use asteroids::{
	custom::{ElementTrait, FnWrapper, Transformable},
	elements::{KeyboardHandler, Model, ModelPart, Spatial, Text},
	Reify, View,
};
use close_button::ExposureButton;
use glam::Quat;
use initial_panel_placement::InitialPanelPlacement;
use initial_positioner::InitialPositioner;
use panel_ui::PanelUI;
use panel_wrapper::PanelWrapper;
use pointer_input::PointerPlane;
use resize_handles::ResizeHandles;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::Client,
	drawable::{TextBounds, TextFit, XAlign, YAlign},
	fields::Shape,
	items::panel::{PanelItem, PanelItemAspect, SurfaceId, ToplevelInfo},
	node::NodeType,
	objects::connect_client,
	project_local_resources,
	root::{RootAspect, RootEvent},
	spatial::Transform,
	values::{color::rgba_linear, Color, Vector2},
};
use std::f32::consts::FRAC_PI_2;
use touch_input::TouchPlane;
use tracing_subscriber::EnvFilter;

pub mod close_button;
pub mod grab_ball;
pub mod initial_panel_placement;
pub mod initial_positioner;
pub mod panel_shell_transfer;
pub mod panel_ui;
pub mod panel_wrapper;
pub mod pointer_input;
pub mod resize_handles;
pub mod touch_input;

async fn accent_color() -> color_eyre::eyre::Result<Color> {
	let accent_color = Settings::new().await?.accent_color().await?;
	Ok(rgba_linear!(
		accent_color.red() as f32,
		accent_color.green() as f32,
		accent_color.blue() as f32,
		1.0
	))
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

	let dbus_connection = connect_client().await.unwrap();
	let mut state = State {
		accent_color,
		toplevels: Default::default(),
		// acceptors: Default::default(),
	};
	let mut asteroids_view = View::new(&state, dbus_connection, client.handle().get_root());

	client
		.sync_event_loop(|client, _| {
			while let Some(RootEvent::Frame { info }) = client.get_root().recv_root_event() {
				asteroids_view.frame(&info);
				asteroids_view.update(&mut state);
			}
		})
		.await
		.unwrap();
}

#[derive(Debug)]
pub struct State {
	accent_color: Color,
	toplevels: FxHashMap<u64, ToplevelState>,
	// acceptors: FxHashMap<u64, (PanelItemAcceptor, Field)>,
}
impl Reify for State {
	fn reify(&self) -> asteroids::Element<Self> {
		let panel_ui = PanelUI::<State> {
			on_create_item: FnWrapper(Box::new(|state, item, data| {
				state.toplevels.insert(
					item.id(),
					ToplevelState {
						enabled: true,
						accent_color: state.accent_color,
						panel_item: item,
						info: data.toplevel,
						density: 3000.0,
					},
				);
			})),
			on_create_acceptor: FnWrapper(Box::new(|_, _, _| {})),
			on_capture_item: FnWrapper(Box::new(|state, panel_id, _| {
				let Some(toplevel) = state.toplevels.get_mut(&panel_id) else {
					return;
				};
				toplevel.enabled = false;
			})),
			on_release_item: FnWrapper(Box::new(|state, panel_id, _| {
				let Some(toplevel) = state.toplevels.get_mut(&panel_id) else {
					return;
				};
				toplevel.enabled = true;
			})),
			on_destroy_item: FnWrapper(Box::new(|state, id| {
				println!("killed panel {id}");
				state.toplevels.remove(&id);
			})),
			on_destroy_acceptor: FnWrapper(Box::new(|_, _| {})),
		}
		.build();

		let toplevels = self.toplevels.iter().filter_map(|(uid, t)| {
			let uid = *uid;
			// self.toplevels.get_mut(&uid)?;
			t.enabled.then(|| {
				t.reify_substate(move |s: &mut Self| s.toplevels.get_mut(&uid))
					.identify(&t.panel_item.id())
			})
		});
		let toplevel_group = Spatial::default().with_children(toplevels);
		Spatial::default().with_children([panel_ui, toplevel_group])
	}
}

#[derive(Debug)]
pub struct ToplevelState {
	enabled: bool,
	accent_color: Color,
	panel_item: PanelItem,
	info: ToplevelInfo,
	density: f32, //pixels per meter
}
impl ToplevelState {
	#[inline]
	pub fn size_meters(&self) -> Vector2<f32> {
		[
			self.info.size.x as f32 / self.density,
			self.info.size.y as f32 / self.density,
		]
		.into()
	}
}
impl Reify for ToplevelState {
	fn reify(&self) -> asteroids::Element<Self> {
		let panel_thickness = 0.01;

		// base model
		let model = Model::namespaced("flatland", "panel")
			.part(
				ModelPart::new("Panel")
					.apply_panel_item(self.panel_item.clone(), SurfaceId::Toplevel(())),
			)
			.scl([
				self.info.size.x as f32 / self.density,
				self.info.size.y as f32 / self.density,
				panel_thickness,
			])
			.build();

		// keyboard handler
		let keyboard_handler = KeyboardHandler::<Self>::new(
			Shape::Box(
				[
					self.info.size.x as f32 / self.density,
					self.info.size.y as f32 / self.density,
					panel_thickness,
				]
				.into(),
			),
			|state, key_data| {
				let _ = state.panel_item.keyboard_key(
					SurfaceId::Toplevel(()),
					key_data.keymap_id,
					key_data.key,
					key_data.pressed,
				);
			},
		)
		.build();

		// input handler
		let pointer_plane = PointerPlane::<Self>::default()
			.pos([0.0, 0.0, panel_thickness / 2.0])
			.physical_size([
				self.info.size.x as f32 / self.density,
				self.info.size.y as f32 / self.density,
			])
			.thickness(panel_thickness)
			.on_mouse_button(|state, button, pressed| {
				let _ = state
					.panel_item
					.pointer_button(SurfaceId::Toplevel(()), button, pressed);
			})
			.on_pointer_motion(|state, pos| {
				let _ = state.panel_item.pointer_motion(
					SurfaceId::Toplevel(()),
					[pos.x * state.density, pos.y * state.density],
				);
			})
			.on_scroll(|state, scroll| {
				let _ = match (scroll.scroll_continuous, scroll.scroll_discrete) {
					(None, None) => state
						.panel_item
						.pointer_stop_scroll(SurfaceId::Toplevel(())),
					(None, Some(steps)) => {
						state
							.panel_item
							.pointer_scroll(SurfaceId::Toplevel(()), [0.0; 2], steps)
					}
					(Some(continuous), None) => state.panel_item.pointer_scroll(
						SurfaceId::Toplevel(()),
						continuous,
						[0.0; 2],
					),
					(Some(continuous), Some(steps)) => {
						state
							.panel_item
							.pointer_scroll(SurfaceId::Toplevel(()), continuous, steps)
					}
				};
			})
			.build();
		let touch_plane = TouchPlane::<Self>::default()
			.pos([0.0, 0.0, panel_thickness / 2.0])
			.physical_size([
				self.info.size.x as f32 / self.density,
				self.info.size.y as f32 / self.density,
			])
			.thickness(panel_thickness)
			.on_touch_down(|state, id, position| {
				let _ = state.panel_item.touch_down(
					SurfaceId::Toplevel(()),
					id,
					[position.x * state.density, position.y * state.density],
				);
			})
			.on_touch_move(|state, id, position| {
				let _ = state
					.panel_item
					.touch_move(id, [position.x * state.density, position.y * state.density]);
			})
			.on_touch_up(|state, id| {
				let _ = state.panel_item.touch_up(id);
			})
			.build();

		// close button
		let close_button = ExposureButton::<Self> {
			transform: Transform::from_translation([
				self.size_meters().x / 2.0,
				self.size_meters().y / -2.0,
				panel_thickness / 2.0,
			]),
			thickness: panel_thickness,
			on_click: FnWrapper(Box::new(|state: &mut Self| {
				let _ = state.panel_item.close_toplevel();
			})),
		}
		.build();

		// title text
		let app_name = self
			.info
			.app_id
			.as_ref()
			.map(|id| id.split('.').last().unwrap_or_default());
		let title_text = match (&self.info.app_id, app_name) {
			(Some(title), Some(app_name)) => {
				if title == app_name {
					title.to_string()
				} else {
					format!("{title} - {app_name}")
				}
			}
			(Some(title), None) => title.to_string(),
			(None, Some(app_name)) => app_name.to_string(),
			(None, None) => String::new(),
		};
		let text = Text::default()
			.text(title_text)
			.character_height(panel_thickness * 0.75)
			.text_align_x(XAlign::Left)
			.text_align_y(YAlign::Center)
			.bounds(TextBounds {
				bounds: [self.size_meters().y, panel_thickness].into(),
				fit: TextFit::Squeeze,
				anchor_align_x: XAlign::Left,
				anchor_align_y: YAlign::Bottom,
			})
			.pos([
				self.size_meters().x / 2.0 + 0.0005,
				self.size_meters().y / 2.0,
				panel_thickness / 2.0,
			])
			.rot(Quat::from_rotation_z(FRAC_PI_2) * Quat::from_rotation_x(-FRAC_PI_2))
			.build();

		let resize_handles = ResizeHandles::<ToplevelState> {
			accent_color: self.accent_color,
			zoneable: true,
			current_size: self.size_meters(),
			min_size: self
				.info
				.min_size
				.map(|s| [s.x / self.density, s.y / self.density].into()),
			max_size: self
				.info
				.max_size
				.map(|s| [s.x / self.density, s.y / self.density].into()),
			on_size_changed: FnWrapper(Box::new(|state, size_meters| {
				let size = [
					(size_meters.x * state.density) as u32,
					(size_meters.y * state.density) as u32,
				];
				let _ = state.panel_item.set_toplevel_size(size);
				state.info.size = size.into();
			})),
		}
		.with_children([
			close_button,
			text,
			model,
			keyboard_handler,
			pointer_plane,
			touch_plane,
		]);

		let panel_wrapper = PanelWrapper::<Self>::new(self.panel_item.clone())
			.on_toplevel_size_changed(|state, size| {
				state.info.size = size;
			})
			.on_toplevel_app_id_changed(|state, app_id| {
				state.info.app_id.replace(app_id);
			})
			.on_toplevel_title_changed(|state, title| {
				state.info.title.replace(title);
			})
			.build();

		let panel_spatial_ref = self
			.panel_item
			.clone()
			.as_item()
			.as_spatial()
			.as_spatial_ref();
		let panel_aligner = InitialPanelPlacement.with_children([panel_wrapper, resize_handles]);
		InitialPositioner(panel_spatial_ref).with_children([panel_aligner])
	}
}
