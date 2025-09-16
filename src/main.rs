use stardust_xr_asteroids::{
	client::{run, ClientState},
	elements::{
		AccentColorListener, KeyboardHandler, Model, ModelPart, MouseHandler, PanelUI, Spatial,
		Text,
	},
	CustomElement, Element, FnWrapper, Identifiable, Migrate, Reify, Transformable as _,
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
use std::{any::Any, f32::consts::FRAC_PI_2, hash::Hash};
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
		update_child_geometry(&mut child.children, id, geometry);
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
			mouse_scroll_multiplier: 10.0,
		}
	}
}
impl Migrate for State {
	type Old = Self;
}
impl ClientState for State {
	const APP_ID: &'static str = "org.stardustxr.flatland";

	fn on_frame(&mut self, info: &FrameInfo) {
		#[cfg(feature = "tracy")]
		{
			use tracing::info;
			info!("frame info {info:#?}");
			tracy_client::frame_mark();
		}
		self.elapsed_time = info.elapsed;
	}
}
impl Reify for State {
	fn reify(&self) -> impl stardust_xr_asteroids::Element<Self> {
		PanelUI::<State> {
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
		.build()
		.children(self.toplevels.iter().filter_map(|(uid, t)| {
			let uid = *uid;
			// self.toplevels.get_mut(&uid)?;
			t.enabled
				.then(|| t.reify_substate(move |s: &mut Self| s.toplevels.get_mut(&uid)))
		}))
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
	fn reify(&self) -> impl stardust_xr_asteroids::Element<Self> {
		let panel_thickness = 0.01;

		let app_name = self
			.info
			.app_id
			.as_ref()
			.map(|id| id.split('.').next_back().unwrap_or_default());
		let title_text = match (&self.info.title, app_name) {
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

		InitialPositioner(
			self.panel_item
				.clone()
				.as_item()
				.as_spatial()
				.as_spatial_ref(),
		)
		.build()
		.identify(&self.panel_item.id())
		.child(
			InitialPanelPlacement
				.build()
				.child(
					PanelWrapper::<Self>::new(self.panel_item.clone())
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
						.on_create_child(|state, _id, child_info| {
							add_child(&mut state.children, child_info)
						})
						.on_reposition_child(|state, id, geometry| {
							update_child_geometry(&mut state.children, id, geometry)
						})
						.on_destroy_child(|state, id| remove_child(&mut state.children, id))
						.build(),
				)
				.child(
					AccentColorListener::new(|state: &mut ToplevelState, accent_color| {
						state.accent_color = accent_color
					})
					.build(),
				)
				.child(
					ResizeHandles::<ToplevelState> {
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
					.child(
						// Close button
						ExposureButton::<Self> {
							transform: Transform::from_translation([
								self.size_meters().x / 2.0,
								self.size_meters().y / -2.0,
								panel_thickness / 2.0,
							]),
							thickness: panel_thickness,
							gain: 2.0,
							on_click: FnWrapper(Box::new(|state: &mut Self| {
								let _ = state.panel_item.close_toplevel();
							})),
						}
						.build(),
					)
					.child(
						// Side text
						Text::new(title_text)
							.character_height(panel_thickness * 0.75)
							.align_x(XAlign::Left)
							.align_y(YAlign::Center)
							.bounds(TextBounds {
								bounds: [self.size_meters().y, panel_thickness].into(),
								fit: TextFit::Squeeze,
								anchor_align_x: XAlign::Left,
								anchor_align_y: YAlign::Bottom,
							})
							.pos([
								self.size_meters().x / 2.0 + 0.0005,
								self.size_meters().y / 2.0 - 0.001,
								panel_thickness / 2.0,
							])
							.rot(
								Quat::from_rotation_z(-FRAC_PI_2)
									* Quat::from_rotation_x(-FRAC_PI_2),
							)
							.build(),
					)
					.child(reify_surface(
						&self.panel_item,
						SurfaceId::Toplevel(()),
						self.info.size,
						Geometry {
							origin: [0; 2].into(),
							size: self.info.size,
						},
						true,
						0,
						panel_thickness,
						self.density,
						&self.panel_item.id(),
						self.children
							.iter()
							.map(|child| {
								child.reify(
									self.info.size,
									&self.panel_item,
									panel_thickness,
									self.density,
								)
							})
							.collect::<Vec<_>>(),
					))
					.children(
						// cursor
						self.cursor.as_ref().map(|geometry| {
							let cursor_pos = vec2(self.cursor_pos.x, self.cursor_pos.y);
							let geometry_origin =
								vec2(geometry.origin.x as f32, geometry.origin.y as f32);
							let geometry_size_half =
								vec2(geometry.size.x as f32, geometry.size.y as f32) / 2.0;
							let panel_size_px_half =
								vec2(self.info.size.x as f32, self.info.size.y as f32) / 2.0;

							let pos_px = cursor_pos - panel_size_px_half + geometry_size_half
								- geometry_origin;
							let pos_m = pos_px * vec2(1.0, -1.0) / self.density;

							Model::namespaced("flatland", "panel")
								.part(
									ModelPart::new("Panel")
										.apply_panel_item_cursor(self.panel_item.clone()),
								)
								.pos([pos_m.x, pos_m.y, 0.001])
								.scl([
									geometry.size.x as f32 / self.density,
									geometry.size.y as f32 / self.density,
									panel_thickness,
								])
								.build()
								.identify(&"cursor".to_string())
						}),
					),
				),
		)
	}
}
impl ChildState {
	fn reify(
		&self,
		parent_size: Vector2<u32>,
		panel_item: &PanelItem,
		panel_thickness: f32,
		density: f32,
	) -> impl Element<ToplevelState> {
		reify_surface(
			panel_item,
			SurfaceId::Child(self.info.id),
			parent_size,
			self.info.geometry,
			self.info.receives_input,
			1,
			panel_thickness,
			density,
			&(panel_item.id(), self.info.id, self.info.type_id()),
			self.children
				.iter()
				.map(|child| {
					child
						.reify(
							self.info.geometry.size,
							panel_item,
							panel_thickness,
							density,
						)
						.heap()
				})
				.collect::<Vec<_>>(),
		)
	}
}

#[allow(clippy::too_many_arguments)]
fn reify_surface<E: Element<ToplevelState>>(
	panel_item: &PanelItem,
	surface_id: SurfaceId,
	parent_size: impl Into<Vector2<u32>>,
	geometry: Geometry,
	input: bool,
	z_offset: i32,
	thickness: f32,
	density: f32,
	id: &impl Hash,
	children: Vec<E>,
) -> impl Element<ToplevelState> {
	let parent_size = parent_size.into();
	let parent_origin_meters = vec2(
		parent_size.x as f32 / density / 2.0,
		parent_size.y as f32 / density / -2.0,
	);
	let origin_meters = vec2(
		geometry.origin.x as f32 / density,
		-geometry.origin.y as f32 / density,
	);
	let size_meters = vec2(
		geometry.size.x as f32 / density,
		geometry.size.y as f32 / density,
	);

	let shape = Shape::Box([size_meters.x, size_meters.y, thickness].into());
	Spatial::default()
		.pos(
			(origin_meters - parent_origin_meters + (size_meters / vec2(2.0, -2.0)))
				.extend(thickness * (z_offset as f32)),
		)
		.build()
		.child(
			Model::namespaced("flatland", "panel")
				.part(ModelPart::new("Panel").apply_panel_item(panel_item.clone(), surface_id))
				.scl([
					geometry.size.x as f32 / density,
					geometry.size.y as f32 / density,
					thickness,
				])
				.build(),
		)
		// inputs
		.maybe_child((input && children.is_empty()).then(move || {
			Spatial::default()
				.build()
				.child(
					KeyboardHandler::<ToplevelState>::new(shape.clone(), move |state, key_data| {
						let _ = state.panel_item.keyboard_key(
							surface_id,
							key_data.keymap_id,
							key_data.key,
							key_data.pressed,
						);
					})
					.build(),
				)
				.child(
					MouseHandler::<ToplevelState>::new(
						shape,
						move |state, button, pressed| {
							let _ = state.panel_item.pointer_button(surface_id, button, pressed);
						},
						move |state, motion| {
							state.cursor_pos.x += motion.x;
							state.cursor_pos.y += motion.y;
							state.cursor_pos.x =
								state.cursor_pos.x.clamp(0.0, state.info.size.x as f32);
							state.cursor_pos.y =
								state.cursor_pos.y.clamp(0.0, state.info.size.y as f32);
							let _ = state
								.panel_item
								.pointer_motion(surface_id, state.cursor_pos);
						},
						move |state, scroll_discrete| {
							let _ = state.panel_item.pointer_scroll(
								surface_id,
								[0.0; 2],
								[scroll_discrete.x, -scroll_discrete.y],
							);
						},
						move |state, scroll_continuous| {
							let _ = state.panel_item.pointer_scroll(
								surface_id,
								[scroll_continuous.x, -scroll_continuous.y],
								[0.0; 2],
							);
						},
					)
					.build(),
				)
				.child(
					PointerPlane::<ToplevelState>::default()
						.physical_size([size_meters.x, size_meters.y])
						.thickness(thickness)
						.on_mouse_button(move |state, button, pressed| {
							let _ = state.panel_item.pointer_button(surface_id, button, pressed);
						})
						.on_pointer_motion(move |state, pos| {
							let pixel_pos = [pos.x * state.density, pos.y * state.density];
							state.cursor_pos = pixel_pos.into();
							let _ = state.panel_item.pointer_motion(surface_id, pixel_pos);
						})
						.on_scroll(move |state, scroll| {
							let _ = match (scroll.scroll_continuous, scroll.scroll_discrete) {
								(None, None) => state.panel_item.pointer_stop_scroll(surface_id),
								(None, Some(steps)) => state.panel_item.pointer_scroll(
									surface_id,
									[0.0; 2],
									[steps.x, -steps.y],
								),
								(Some(continuous), None) => state.panel_item.pointer_scroll(
									surface_id,
									[continuous.x, -continuous.y],
									[0.0; 2],
								),
								(Some(continuous), Some(steps)) => state.panel_item.pointer_scroll(
									surface_id,
									[continuous.x, -continuous.y],
									[steps.x, -steps.y],
								),
							};
						})
						.build(),
				)
				.child(
					TouchPlane::<ToplevelState>::default()
						.physical_size([size_meters.x, size_meters.y])
						.thickness(thickness)
						.on_touch_down(move |state, id, position| {
							let _ = state.panel_item.touch_down(
								surface_id,
								id,
								[position.x * state.density, position.y * state.density],
							);
						})
						.on_touch_move(|state, id, position| {
							let _ = state.panel_item.touch_move(
								id,
								[position.x * state.density, position.y * state.density],
							);
						})
						.on_touch_up(|state, id| {
							let _ = state.panel_item.touch_up(id);
						})
						.build(),
				)
		}))
		.children(children)
		.identify(id)
}
