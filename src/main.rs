use asteroids::{
	client::{run, ClientState},
	elements::{
		AccentColorListener, KeyboardHandler, Model, ModelPart, MouseHandler, PanelUI, Spatial,
		Text,
	},
	CustomElement as _, Element, FnWrapper, Migrate, Reify, Transformable as _,
};
use close_button::ExposureButton;
use glam::{vec2, Quat};
use initial_panel_placement::InitialPanelPlacement;
use initial_positioner::InitialPositioner;
use panel_wrapper::PanelWrapper;
use pointer_input::PointerPlane;
use resize_handles::ResizeHandles;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use stardust_xr_fusion::{
	drawable::{TextBounds, TextFit, XAlign, YAlign},
	fields::Shape,
	items::panel::{ChildInfo, Geometry, PanelItem, PanelItemAspect, SurfaceId, ToplevelInfo},
	node::NodeType,
	project_local_resources,
	root::FrameInfo,
	spatial::Transform,
	values::{color::rgba_linear, Color, Vector2},
};
use stardust_xr_molecules::DebugSettings;
use std::{any::Any, f32::consts::FRAC_PI_2};
use touch_input::TouchPlane;
use tracing_subscriber::{layer::SubscriberExt as _, EnvFilter};

pub mod close_button;
pub mod grab_ball;
pub mod initial_panel_placement;
pub mod initial_positioner;
pub mod panel_shell_transfer;
pub mod panel_wrapper;
pub mod pointer_input;
pub mod resize_handles;
pub mod touch_input;

#[tokio::main(flavor = "current_thread")]
async fn main() {
	let registry = tracing_subscriber::registry();
	#[cfg(feature = "tracy")]
	let registry = registry.with(tracing_tracy::TracyLayer::default());
	tracing::subscriber::set_global_default(
		registry
			.with(EnvFilter::from_default_env())
			.with(tracing_subscriber::fmt::layer().compact()),
	)
	.unwrap();

	run::<State>(&[&project_local_resources!("res")]).await
}

pub fn add_child(children: &mut Vec<ChildState>, child_info: ChildInfo) {
	match &child_info.parent {
		SurfaceId::Toplevel(_) => {
			children.push(ChildState {
				info: child_info,
				children: Vec::new(),
			});
		}
		SurfaceId::Child(parent_id) => {
			add_to_parent(
				children,
				*parent_id,
				ChildState {
					info: child_info,
					children: Vec::new(),
				},
			);
		}
	}
}

fn add_to_parent(children: &mut [ChildState], parent_id: u64, new_child: ChildState) {
	for child in children.iter_mut() {
		if child.info.id == parent_id {
			child.children.push(new_child);
			return;
		}
		add_to_parent(&mut child.children, parent_id, new_child.clone());
	}
}
pub fn update_child_geometry(children: &mut [ChildState], id: u64, geometry: Geometry) {
	for child in children.iter_mut() {
		if child.info.id == id {
			child.info.geometry = geometry;
			return;
		}
		update_child_geometry(&mut child.children, id, geometry.clone());
	}
}
pub fn remove_child(children: &mut Vec<ChildState>, id: u64) {
	children.retain_mut(|child| {
		if child.info.id == id {
			return false;
		}
		remove_child(&mut child.children, id);
		true
	});
}
pub fn process_initial_children(children: Vec<ChildInfo>) -> Vec<ChildState> {
	let mut child_states = Vec::new();
	for child in children {
		add_child(&mut child_states, child);
	}
	child_states
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
	#[serde(skip)]
	elapsed_time: f32,
	toplevel_preferences: FxHashMap<String, f32>,
	mouse_scroll_multiplier: f32,
	#[serde(skip)]
	toplevels: FxHashMap<u64, ToplevelState>,
	// acceptors: FxHashMap<u64, (PanelItemAcceptor, Field)>,
}
impl Default for State {
	fn default() -> Self {
		State {
			elapsed_time: 0.0,
			toplevels: FxHashMap::default(),
			toplevel_preferences: FxHashMap::default(),
			mouse_scroll_multiplier: 20.0,
		}
	}
}
impl Migrate for State {
	type Old = Self;
}
impl ClientState for State {
	const APP_ID: &str = "org.stardustxr.flatland";

	fn on_frame(&mut self, info: &FrameInfo) {
		#[cfg(feature = "tracy")]
		{
			use tracing::info;
			info!("frame info {info:#?}");
			tracy_client::frame_mark();
		}
		self.elapsed_time = info.elapsed;
	}
	fn reify(&self) -> asteroids::Element<Self> {
		let panel_ui = PanelUI::<State> {
			on_create_item: FnWrapper(Box::new(|state, item, data| {
				state.toplevels.insert(
					item.id(),
					ToplevelState {
						enabled: true,
						accent_color: rgba_linear!(1.0, 1.0, 1.0, 1.0),
						panel_item: item,
						info: data.toplevel,
						cursor_pos: [0.0; 2].into(),
						cursor: None,
						children: process_initial_children(data.children),
						density: 3000.0,
						mouse_scroll_multiplier: state.mouse_scroll_multiplier,
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
		let toplevel_group = Spatial::default().build().children(toplevels);
		Spatial::default()
			.build()
			.children([panel_ui, toplevel_group])
	}
}

#[derive(Debug, Clone)]
pub struct ChildState {
	info: ChildInfo,
	children: Vec<ChildState>,
}

#[derive(Debug)]
pub struct ToplevelState {
	enabled: bool,
	accent_color: Color,
	panel_item: PanelItem,
	info: ToplevelInfo,
	/// in px
	cursor_pos: Vector2<f32>,
	cursor: Option<Geometry>,
	children: Vec<ChildState>,
	density: f32, //pixels per meter
	mouse_scroll_multiplier: f32,
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

		let cursor_model = self.cursor.as_ref().map(|geometry| {
			let cursor_pos = vec2(self.cursor_pos.x, self.cursor_pos.y);
			let geometry_origin = vec2(geometry.origin.x as f32, geometry.origin.y as f32);
			let geometry_size_half = vec2(geometry.size.x as f32, geometry.size.y as f32) / 2.0;
			let panel_size_px_half = vec2(self.info.size.x as f32, self.info.size.y as f32) / 2.0;

			dbg!(geometry);
			let pos_px = cursor_pos - panel_size_px_half + geometry_size_half - geometry_origin;
			let pos_m = pos_px * vec2(1.0, -1.0) / self.density;

			Model::namespaced("flatland", "panel")
				.part(ModelPart::new("Panel").apply_panel_item_cursor(self.panel_item.clone()))
				.pos([pos_m.x, pos_m.y, 0.001])
				.scl([
					geometry.size.x as f32 / self.density,
					geometry.size.y as f32 / self.density,
					panel_thickness,
				])
				.build()
		});

		let shape = Shape::Box(
			[
				self.info.size.x as f32 / self.density,
				self.info.size.y as f32 / self.density,
				panel_thickness,
			]
			.into(),
		);
		// keyboard handler
		let keyboard_handler = KeyboardHandler::<Self>::new(shape.clone(), |state, key_data| {
			let _ = state.panel_item.keyboard_key(
				SurfaceId::Toplevel(()),
				key_data.keymap_id,
				key_data.key,
				key_data.pressed,
			);
		})
		.build();
		// mouse handler
		let mouse_handler = MouseHandler::<Self>::new(
			shape,
			|state, button, pressed| {
				let _ = state
					.panel_item
					.pointer_button(SurfaceId::Toplevel(()), button, pressed);
			},
			|state, motion| {
				state.cursor_pos.x += motion.x;
				state.cursor_pos.y += motion.y;
				state.cursor_pos.x = state.cursor_pos.x.clamp(0.0, state.info.size.x as f32);
				state.cursor_pos.y = state.cursor_pos.y.clamp(0.0, state.info.size.y as f32);
				let _ = state
					.panel_item
					.pointer_motion(SurfaceId::Toplevel(()), state.cursor_pos);
			},
			|state, scroll_discrete| {
				let _ = state.panel_item.pointer_scroll(
					SurfaceId::Toplevel(()),
					[0.0; 2],
					[
						scroll_discrete.x * state.mouse_scroll_multiplier,
						scroll_discrete.y * state.mouse_scroll_multiplier,
					], // negative because this is surface-local coords
				);
			},
			|state, scroll_continuous| {
				let _ = state.panel_item.pointer_scroll(
					SurfaceId::Toplevel(()),
					[
						scroll_continuous.x * state.mouse_scroll_multiplier,
						scroll_continuous.y * state.mouse_scroll_multiplier,
					], // negative because this is surface-local coords
					[0.0; 2],
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
				let pixel_pos = [pos.x * state.density, pos.y * state.density];
				state.cursor_pos = pixel_pos.into();
				state.cursor_pos.x = state.cursor_pos.x.clamp(0.0, state.info.size.x as f32);
				state.cursor_pos.y = state.cursor_pos.y.clamp(0.0, state.info.size.y as f32);
				let _ = state
					.panel_item
					.pointer_motion(SurfaceId::Toplevel(()), pixel_pos);
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
			.debug_line_settings(DebugSettings {
				line_color: self.accent_color,
				..Default::default()
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
			.map(|id| id.split('.').next_back().unwrap_or_default());
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

		// for child in self.info.

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
				state.cursor_pos.x = state.cursor_pos.x.clamp(0.0, size[0] as f32);
				state.cursor_pos.y = state.cursor_pos.y.clamp(0.0, size[1] as f32);
			})),
		}
		.build()
		.children([
			close_button,
			text,
			model,
			cursor_model.unwrap_or_else(|| Spatial::default().build()),
			keyboard_handler,
			mouse_handler,
			pointer_plane,
			touch_plane,
			Spatial::default()
				.build()
				.children(self.reify_children(&self.children, panel_thickness)),
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
			.on_set_cursor(|state, geometry| {
				state.cursor.replace(geometry);
			})
			.on_hide_cursor(|state| {
				state.cursor.take();
			})
			.on_create_child(|state, _id, child_info| add_child(&mut state.children, child_info))
			.on_reposition_child(|state, id, geometry| {
				update_child_geometry(&mut state.children, id, geometry)
			})
			.on_destroy_child(|state, id| remove_child(&mut state.children, id))
			.build();

		let accent_color_listener =
			AccentColorListener::new(|state: &mut ToplevelState, accent_color| {
				state.accent_color = accent_color
			})
			.build();

		let panel_spatial_ref = self
			.panel_item
			.clone()
			.as_item()
			.as_spatial()
			.as_spatial_ref();
		let panel_aligner = InitialPanelPlacement.build().children([
			panel_wrapper,
			accent_color_listener,
			resize_handles,
		]);
		InitialPositioner(panel_spatial_ref)
			.build()
			.children([panel_aligner])
	}
}
impl ToplevelState {
	fn reify_children(&self, children: &[ChildState], panel_thickness: f32) -> Vec<Element<Self>> {
		children
			.iter()
			.map(|child| {
				let child_model = child.reify(&self.panel_item, self.density, panel_thickness);
				let mut reified_children = self.reify_children(&child.children, panel_thickness);
				reified_children.push(child_model);
				Spatial::default()
					.pos([
						self.info.size.x as f32 / -2.0 / self.density,
						self.info.size.y as f32 / -2.0 / self.density,
						0.0,
					])
					.build()
					.children(reified_children)
					.identify(&(self.panel_item.id(), child.info.id, child.info.type_id()))
			})
			.collect()
	}
}
impl ChildState {
	fn reify(
		&self,
		panel_item: &PanelItem,
		density: f32,
		panel_thickness: f32,
	) -> Element<ToplevelState> {
		let geometry_origin = vec2(
			self.info.geometry.origin.x as f32,
			self.info.geometry.origin.y as f32,
		);
		let geometry_size = vec2(
			self.info.geometry.size.x as f32,
			self.info.geometry.size.y as f32,
		);
		let origin = (geometry_origin + (geometry_size / 2.0)) / density;
		Model::namespaced("flatland", "panel")
			.part(
				ModelPart::new("Panel")
					.apply_panel_item(panel_item.clone(), SurfaceId::Child(self.info.id)),
			)
			.pos([
				origin.x,
				origin.y,
				panel_thickness * (1.0 + self.info.z_order as f32),
			])
			.scl([
				self.info.geometry.size.x as f32 / density,
				self.info.geometry.size.y as f32 / density,
				panel_thickness,
			])
			.build()
	}
}
