use binderbinder::binder_object::BinderObject;
use close_button::ExposureButton;
use glam::{Quat, vec2};
use initial_panel_placement::InitialPanelPlacement;
use pointer_input::PointerPlane;
use resize_handles::ResizeHandles;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use stardust_xr_asteroids::{
	Context, CustomElement, Element, FnWrapper, Migrate, Reify, Tasker, Transformable as _,
	client::{ClientState, run},
	elements::{Derezzable, KeyboardHandler, Model, MouseHandler, Spatial, Text},
};
use stardust_xr_fusion::{
	drawable::{TextBounds, TextFit, XAlign, YAlign},
	fields::Shape,
	project_local_resources,
	spatial::Transform,
	types::{Posef, Resource, Size2, Timestamp, Vec2F},
};
use stardust_xr_panel_item::panel_item::{
	ChildState as ChildInfo, Geometry, ModifierState, Rect, ScrollSource, SurfaceId,
	SurfaceUpdateTarget, ToplevelState as ToplevelInfo,
};
use stardust_xr_panel_item_asteroids::{
	panel_item_acceptor::PanelItemAcceptor,
	panel_shell::{PanelShell, PanelShellHandler},
	surface_model::SurfaceModel,
};
use std::{f32::consts::FRAC_PI_2, process};
use touch_input::TouchPlane;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _};

pub mod close_button;
pub mod grab_ball;
pub mod initial_panel_placement;
// pub mod panel_shell_transfer;
pub mod pointer_input;
pub mod resize_handles;
pub mod touch_input;

// #[tokio::main(flavor = "current_thread")]
#[tokio::main]
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

	run::<ToplevelState>(&[&project_local_resources!("data")])
		.await
		.unwrap()
}

pub fn add_child(children: &mut Vec<ChildState>, child_info: ChildInfo) {
	match &child_info.parent {
		SurfaceId::Toplevel => {
			children.push(ChildState {
				info: child_info,
				children: Vec::new(),
			});
		}
		SurfaceId::Child { id: parent_id } => {
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

impl Migrate for ToplevelState {
	type Old = Self;
}
impl ClientState for ToplevelState {
	const APP_ID: &'static str = "org.stardustxr.Flatland";
}
impl Default for ToplevelState {
	fn default() -> Self {
		Self {
			panel_shell: Default::default(),
			info: default_toplevel_info(),
			cursor_pos: [0.0; 2].into(),
			cursor: Default::default(),
			children: Default::default(),
			density: 3000.0,
			mouse_scroll_multiplier: 1.0,
			exit_on_disconnect: false,
			pose: Posef::default(),
		}
	}
}
// impl Reify for ToplevelState {
// 	fn reify(&self) -> impl stardust_xr_asteroids::Element<Self> {
// 		PanelUI::<State> {
// 			on_create_item: FnWrapper(Box::new(|state, item, data| {
// 				state.toplevels.insert(
// 					item.id(),
// 					ToplevelState {
// 						enabled: true,
// 						panel_item: item,
// 						info: data.toplevel,
// 						cursor_pos: [0.0; 2].into(),
// 						cursor: None,
// 						children: process_initial_children(data.children),
// 						density: 3000.0,
// 						mouse_scroll_multiplier: state.mouse_scroll_multiplier,
// 					},
// 				);
// 			})),
// 			on_create_acceptor: FnWrapper(Box::new(|_, _, _| {})),
// 			on_capture_item: FnWrapper(Box::new(|state, panel_id, _| {
// 				let Some(toplevel) = state.toplevels.get_mut(&panel_id) else {
// 					return;
// 				};
// 				toplevel.enabled = false;
// 			})),
// 			on_release_item: FnWrapper(Box::new(|state, panel_id, _| {
// 				let Some(toplevel) = state.toplevels.get_mut(&panel_id) else {
// 					return;
// 				};
// 				toplevel.enabled = true;
// 			})),
// 			on_destroy_item: FnWrapper(Box::new(|state, id| {
// 				state.toplevels.remove(&id);
// 			})),
// 			on_destroy_acceptor: FnWrapper(Box::new(|_, _| {})),
// 		}
// 		.build()
// 		.stable_children(self.toplevels.iter().filter_map(|(uid, t)| {
// 			let uid = *uid;
// 			// self.toplevels.get_mut(&uid)?;
// 			if !t.enabled {
// 				return None;
// 			}
// 			Some((
// 				uid,
// 				t.reify_substate(move |s: &mut Self| s.toplevels.get_mut(&uid)),
// 			))
// 		}))
// 	}
// }

#[derive(Debug, Clone)]
pub struct ChildState {
	info: ChildInfo,
	children: Vec<ChildState>,
}

const fn default_toplevel_info() -> ToplevelInfo {
	ToplevelInfo {
		parent: None,
		title: None,
		app_id: None,
		size: Size2 { x: 600, y: 800 },
		min_size: None,
		max_size: None,
	}
}

type Shell = BinderObject<PanelShellHandler>;
#[derive(Debug, Serialize, Deserialize)]
pub struct ToplevelState {
	#[serde(skip)]
	panel_shell: Option<Shell>,
	#[serde(skip, default = "default_toplevel_info")]
	info: ToplevelInfo,
	/// in px
	cursor_pos: Vec2F,
	#[serde(skip)]
	cursor: Option<Geometry>,
	#[serde(skip)]
	children: Vec<ChildState>,
	density: f32, //pixels per meter
	mouse_scroll_multiplier: f32,
	#[serde(skip)]
	exit_on_disconnect: bool,
	#[serde(skip)]
	pose: Posef,
}
impl ToplevelState {
	#[inline]
	pub fn size_meters(&self) -> Vec2F {
		[
			self.info.size.x as f32 / self.density,
			self.info.size.y as f32 / self.density,
		]
		.into()
	}
	fn set_pointer(
		&mut self,
		surface_id: SurfaceId,
		motion: Option<Vec2F>,
		new_pos: impl Into<Vec2F>,
		timestamp: Option<Timestamp>,
	) {
		self.cursor_pos = new_pos.into();
		self.clamp_pointer();
		if let Some(shell) = self.panel_shell.as_ref() {
			let _ = shell
				.item()
				.pointer_motion(surface_id, motion, self.cursor_pos, timestamp);
		}
	}
	fn clamp_pointer(&mut self) {
		self.cursor_pos.x = self
			.cursor_pos
			.x
			.clamp(0.0, self.info.size.x.saturating_sub(1) as f32);
		self.cursor_pos.y = self
			.cursor_pos
			.y
			.clamp(0.0, self.info.size.y.saturating_sub(1) as f32);
	}
	fn resize(&mut self, new_size: impl Into<Size2>) {
		let old_size = self.info.size;
		self.info.size = new_size.into();
		fn clamp(v: &mut u32, min: u32, max: u32) {
			*v = (*v).clamp(min, max);
		}
		let min_size = self.info.min_size.unwrap_or([0; 2].into());
		let max_size = self.info.max_size.unwrap_or([u32::MAX; 2].into());
		clamp(&mut self.info.size.x, min_size.x, max_size.x);
		clamp(&mut self.info.size.y, min_size.y, max_size.y);
		tracing::info!(?min_size,?max_size,?self.info.size,"clamping size");
		if (old_size.x != self.info.size.x || old_size.y != self.info.size.y)
			&& let Some(shell) = self.panel_shell.as_ref()
		{
			shell
				.item()
				.request_toplevel_resize(self.info.size)
				.unwrap();
		}
	}
}
impl Reify for ToplevelState {
	fn reify(
		&self,
		_context: &Context,
		_tasks: impl Tasker<Self>,
	) -> impl stardust_xr_asteroids::Element<Self> {
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
		InitialPanelPlacement
			.build()
			.maybe_child(self.panel_shell.as_ref().map(|shell| {
				PanelShell::new(shell, |state: &mut Self| {
					_ = state.panel_shell.take();
					if state.exit_on_disconnect {
						process::exit(0);
					}
                    // TODO: move size onto main state
                    let size = state.info.size;
                    // reset panel item specific state
                    state.info = default_toplevel_info();
                    state.info.size = size;
				})
				.on_toplevel_resolution_changed(|state: &mut Self, _item, size| {
					state.info.size = size;
				})
				.on_toplevel_max_size_changed(|state: &mut Self, _item, size| {
					state.info.max_size = size;
					// state.resize(state.info.size);
				})
				.on_toplevel_min_size_changed(|state: &mut Self, _item, size| {
					state.info.min_size = size;
					// state.resize(state.info.size);
				})
				.on_toplevel_app_id_changed(|state: &mut Self, _, app_id| {
					state.info.app_id.replace(app_id);
				})
				.on_toplevel_title_changed(|state: &mut Self, _, title| {
					state.info.title.replace(title);
				})
				.cursor_visuals_changed(|state: &mut Self, _, geometry| {
					state.cursor = geometry;
				})
				.new_child(|state: &mut Self, _, child_state| {
					add_child(&mut state.children, child_state);
				})
				.child_moved(|state: &mut Self, _, id, geometry| {
					update_child_geometry(&mut state.children, id, geometry);
				})
				.child_removed(|state: &mut Self, _, id| {
					remove_child(&mut state.children, id);
				})
				.build()
			}))
			.child(
				ResizeHandles::<ToplevelState> {
					reparentable: true,
					pose: self.pose,
					size: self.size_meters(),
					min_size: self
						.info
						.min_size
						.map(|s| [s.x as f32 / self.density, s.y as f32 / self.density].into()),
					max_size: self
						.info
						.max_size
						.map(|s| [s.x as f32 / self.density, s.y as f32 / self.density].into()),
					on_change: FnWrapper(Box::new(|state, pose, size_meters| {
						state.pose = pose;
						let size = [
							(size_meters.x * state.density) as u32,
							(size_meters.y * state.density) as u32,
						];
						state.resize(size);
						state.clamp_pointer();
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
							if let Some(shell) = state.panel_shell.as_ref() {
								shell.item().close_toplevel().unwrap();
							}
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
						.rot(Quat::from_rotation_z(-FRAC_PI_2) * Quat::from_rotation_x(-FRAC_PI_2))
						.build(),
				)
				.child(reify_surface(
					&self.panel_shell,
					SurfaceId::Toplevel,
					self.info.size,
					Geometry {
						origin: [0; 2].into(),
						size: self.info.size,
					},
					&[Rect {
						origin: [0.0; 2].into(),
						size: [1.0; 2].into(),
					}],
					0,
					panel_thickness,
					self.density,
					self.children
						.iter()
						.map(|child| {
							(
								child.info.id,
								child.reify(
									self.info.size,
									&self.panel_shell,
									panel_thickness,
									self.density,
									self.mouse_scroll_multiplier,
								),
							)
						})
						.collect(),
					self.mouse_scroll_multiplier,
				))
				.maybe_child(
					// cursor
					self.cursor
						.as_ref()
						.and_then(|v| Some((v, self.panel_shell.as_ref()?)))
						.map(|(geometry, shell)| {
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

							SurfaceModel::new(
								shell,
								SurfaceUpdateTarget::Cursor,
								Resource::Namespaced {
									namespace: ToplevelState::APP_ID.into(),
									path: "panel".into(),
								},
								"Panel",
							)
							.pos([pos_m.x, pos_m.y, 0.001])
							.scl([
								geometry.size.x as f32 / self.density,
								geometry.size.y as f32 / self.density,
								panel_thickness,
							])
							.build()
						}),
				),
			)
		// )
	}
}
impl ChildState {
	fn reify(
		&self,
		parent_size: Size2,
		panel_item: &Option<Shell>,
		panel_thickness: f32,
		density: f32,
		scroll_multiplier: f32,
	) -> impl Element<ToplevelState> {
		reify_surface(
			panel_item,
			SurfaceId::Child { id: self.info.id },
			parent_size,
			self.info.geometry,
			&self.info.input_regions,
			1,
			panel_thickness,
			density,
			self.children
				.iter()
				.map(|child| {
					(
						child.info.id,
						child.reify(
							self.info.geometry.size,
							panel_item,
							panel_thickness,
							density,
							scroll_multiplier,
						),
					)
				})
				.collect(),
			scroll_multiplier,
		)
		.dynamic()
	}
}

#[allow(clippy::too_many_arguments)]
fn reify_surface<S: Into<Size2>, E: Element<ToplevelState>>(
	panel_item: &Option<Shell>,
	surface_id: SurfaceId,
	parent_size: S,
	geometry: Geometry,
	input_areas: &[Rect],
	z_offset: i32,
	thickness: f32,
	density: f32,
	children: FxHashMap<u64, E>,
	scroll_multiplier: f32,
) -> impl Element<ToplevelState> + use<S, E> {
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

	let shape = Shape::Box {
		size: [size_meters.x, size_meters.y, thickness].into(),
	};
	Spatial::default()
		.pos(
			(origin_meters - parent_origin_meters + (size_meters / vec2(2.0, -2.0)))
				.extend(thickness * (z_offset as f32)),
		)
		.build()
		.child(
			Derezzable::<ToplevelState>::new(
				|state| {
					state.exit_on_disconnect = true;
					if let Some(shell) = state.panel_shell.as_ref() {
						_ = shell.item().close_toplevel();
					}
				},
				shape.clone(),
			)
			.build(),
		)
		.maybe_child(panel_item.as_ref().map(|item| {
			SurfaceModel::new(
				item,
				surface_id,
				Resource::Namespaced {
					namespace: ToplevelState::APP_ID.into(),
					path: "panel".into(),
				},
				"Panel",
			)
			.scl([
				geometry.size.x as f32 / density,
				geometry.size.y as f32 / density,
				thickness,
			])
			.build()
		}))
		.maybe_child(panel_item.is_none().then(|| {
			Model::namespaced(ToplevelState::APP_ID, "panel")
				.scl([
					geometry.size.x as f32 / density,
					geometry.size.y as f32 / density,
					thickness,
				])
				.build()
		}))
		.maybe_child(panel_item.is_none().then(|| {
			PanelItemAcceptor::<ToplevelState>::new(shape.clone(), |state, shell| {
				_ = shell.item().request_toplevel_resize(state.info.size);
				state.panel_shell.replace(shell);
			})
			.build()
		}))
		// inputs
		.maybe_child((!input_areas.is_empty()).then(move || {
			Spatial::default()
				.build()
				.child(
					KeyboardHandler::<ToplevelState>::new(shape.clone())
						.on_key_async({
							let panel_item = panel_item.as_ref().map(|v| v.item().clone());
							move |key_event, timestamp| {
								if let Some(item) = &panel_item {
									item.key(
										surface_id,
										key_event.keycode,
										key_event.pressed,
										ModifierState {
											depressed: key_event.modifiers.depressed,
											latched: key_event.modifiers.latched,
											locked: key_event.modifiers.locked,
										},
										key_event.keymap,
										timestamp,
									)
									.unwrap();
								}
							}
						})
						.build(),
				)
				.child(
					MouseHandler::<ToplevelState>::new(shape)
						.on_button_async({
							let panel_item = panel_item.as_ref().map(|v| v.item().clone());
							move |button, pressed, timestamp| {
								if let Some(item) = &panel_item {
									let _ =
										item.pointer_button(surface_id, button, pressed, timestamp);
								}
							}
						})
						.on_motion(move |state, motion, timestamp| {
							let new_pos =
								[state.cursor_pos.x + motion.x, state.cursor_pos.y - motion.y];
							state.set_pointer(
								surface_id,
								Some([motion.x, -motion.y].into()),
								new_pos,
								timestamp,
							);
						})
						.on_scroll_discrete_async({
							let panel_item = panel_item.as_ref().map(|v| v.item().clone());
							move |scroll_discrete, source, timestamp| {
								use stardust_xr_asteroids::elements::ScrollSource as MoleculesSource;
								if let Some(item) = &panel_item {
									item.pointer_scroll_discrete(
										surface_id,
										[
											scroll_discrete.x * scroll_multiplier,
											-scroll_discrete.y * scroll_multiplier,
										]
										.into(),
										match source {
											MoleculesSource::Wheel => ScrollSource::Wheel,
											MoleculesSource::Finger => ScrollSource::Touch,
											MoleculesSource::Continuous => ScrollSource::Continuous,
											MoleculesSource::WheelTilt => ScrollSource::WheelTilt,
										},
										timestamp,
									)
									.unwrap();
								}
							}
						})
						.on_scroll_continuous_async({
							let panel_item = panel_item.as_ref().map(|v| v.item().clone());
							move |scroll_continuous, source, timestamp| {
								use stardust_xr_asteroids::elements::ScrollSource as MoleculesSource;
								if let Some(item) = &panel_item {
									item.pointer_scroll_pixels(
										surface_id,
										[
											scroll_continuous.x * scroll_multiplier,
											-scroll_continuous.y * scroll_multiplier,
										]
										.into(),
										match source {
											MoleculesSource::Wheel => ScrollSource::Wheel,
											MoleculesSource::Finger => ScrollSource::Touch,
											MoleculesSource::Continuous => ScrollSource::Continuous,
											MoleculesSource::WheelTilt => ScrollSource::WheelTilt,
										},
										timestamp,
									)
									.unwrap();
								}
							}
						})
						.build(),
				)
				.child(
					PointerPlane::<ToplevelState>::default()
						.physical_size([size_meters.x, size_meters.y])
						.thickness(thickness)
						.on_mouse_button(move |state, button, pressed| {
							if let Some(shell) = state.panel_shell.as_ref() {
								// TODO: somehow get a timestamp for this?
								let _ = shell
									.item()
									.pointer_button(surface_id, button, pressed, None);
							}
						})
						.on_pointer_motion(move |state, pos| {
							let pixel_pos = [pos.x * state.density, pos.y * state.density];
							state.set_pointer(surface_id, None, pixel_pos, None);
						})
						.on_scroll(move |state, scroll| {
							if let Some(scroll_continuous) = scroll.scroll_continuous
								&& let Some(shell) = state.panel_shell.as_ref()
							{
								shell
									.item()
									.pointer_scroll_pixels(
										surface_id,
										[
											scroll_continuous.x * state.mouse_scroll_multiplier,
											-scroll_continuous.y * state.mouse_scroll_multiplier,
										]
										.into(),
										ScrollSource::Continuous,
										// TODO: somehow get a timestamp for this?
										None,
									)
									.unwrap();
							}
							if let Some(scroll_discrete) = scroll.scroll_discrete
								&& let Some(shell) = state.panel_shell.as_ref()
							{
								shell
									.item()
									.pointer_scroll_pixels(
										surface_id,
										[
											scroll_discrete.x * state.mouse_scroll_multiplier,
											-scroll_discrete.y * state.mouse_scroll_multiplier,
										]
										.into(),
										ScrollSource::Continuous,
										// TODO: somehow get a timestamp for this?
										None,
									)
									.unwrap();
							}
							// TODO: figure out how to send this only when scroll actually stops,
							// instead of every frame without scroll
							if scroll.scroll_continuous.is_none()
								&& scroll.scroll_discrete.is_none()
								&& let Some(shell) = state.panel_shell.as_ref()
							{
								// TODO: somehow get a timestamp for this?
								shell.item().pointer_scroll_stop(surface_id, None).unwrap();
							}
						})
						.build(),
				)
				.child(
					TouchPlane::<ToplevelState>::default()
						.physical_size([size_meters.x, size_meters.y])
						.thickness(thickness)
						.on_touch_down(move |state, id, position| {
							if let Some(shell) = state.panel_shell.as_ref() {
								let _ = shell.item().touch_down(
									surface_id,
									id,
									[position.x * state.density, position.y * state.density].into(),
									// TODO: somehow get a timestamp for this?
									None,
								);
							}
						})
						.on_touch_move(|state, id, position| {
							if let Some(shell) = state.panel_shell.as_ref() {
								let _ = shell.item().touch_move(
									id,
									[position.x * state.density, position.y * state.density].into(),
									// TODO: somehow get a timestamp for this?
									None,
								);
							}
						})
						.on_touch_up(|state, id| {
							if let Some(shell) = state.panel_shell.as_ref() {
								// TODO: somehow get a timestamp for this?
								let _ = shell.item().touch_up(id, None);
							}
						})
						.build(),
				)
		}))
		.stable_children(children)
}
