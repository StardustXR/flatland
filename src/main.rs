use binderbinder::binder_object::BinderObject;
use close_button::ExposureButton;
use glam::{Quat, vec2, vec3};
use initial_panel_placement::InitialPanelPlacement;
use pion_binder::PionBinderDevice;
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
	values::{ResourceID, Vector2},
};
use stardust_xr_panel_item::protocol::{
	ChildState as ChildInfo, Geometry, KeymapId, Rect, ScrollSource, SurfaceId,
	SurfaceUpdateTarget, ToplevelState as ToplevelInfo, UVec2, Vec2,
};
use stardust_xr_panel_item_asteroids::{
	panel_item_acceptor::PanelItemAcceptorElement,
	panel_shell::{PanelShell, PanelShellHandler},
	surface_model::SurfaceModel,
};
use std::{f32::consts::FRAC_PI_2, sync::Arc};
use touch_input::TouchPlane;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _};

pub mod close_button;
pub mod grab_ball;
pub mod initial_panel_placement;
pub mod initial_positioner;
pub mod panel_shell_transfer;
pub mod panel_wrapper;
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
	// tokio::spawn(async {
	// 	loop {
	// 		tokio::time::sleep(Duration::from_millis(1000)).await;
	// 		info!("idk");
	// 	}
	// });

	run::<ToplevelState>(&[&project_local_resources!("res")]).await
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
	const APP_ID: &'static str = "org.stardustxr.flatland";
}
impl Default for ToplevelState {
	fn default() -> Self {
		Self {
			binder_dev: Default::default(),
			panel_shell: Default::default(),
			info: default_toplevel_info(),
			cursor_pos: [0.0; 2].into(),
			cursor: Default::default(),
			children: Default::default(),
			density: 3000.0,
			mouse_scroll_multiplier: Default::default(),
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

fn default_toplevel_info() -> ToplevelInfo {
	ToplevelInfo {
		parent: None,
		title: None,
		app_id: None,
		size: UVec2 { x: 600, y: 800 },
		min_size: None,
		max_size: None,
	}
}

type Shell = Arc<BinderObject<PanelShellHandler>>;
#[derive(Debug, Serialize, Deserialize)]
pub struct ToplevelState {
	#[serde(skip)]
	binder_dev: PionBinderDevice,
	#[serde(skip)]
	panel_shell: Option<Shell>,
	#[serde(skip, default = "default_toplevel_info")]
	info: ToplevelInfo,
	/// in px
	cursor_pos: Vector2<f32>,
	#[serde(skip)]
	cursor: Option<Geometry>,
	#[serde(skip)]
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
	fn reify(
		&self,
		context: &Context,
		tasks: impl Tasker<Self>,
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

		// InitialPositioner(
		// 	self.panel_item
		// 		.clone()
		// 		.as_item()
		// 		.as_spatial()
		// 		.as_spatial_ref(),
		// )
		// .build()
		// .child(
		InitialPanelPlacement
			.build()
			.maybe_child(self.panel_shell.as_ref().map(|shell| {
				PanelShell::new(&shell)
					.on_toplevel_resolution_changed(|state: &mut Self, _item, size| {
						state.info.size = size.into();
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
					current_size: self.size_meters(),
					min_size: self
						.info
						.min_size
						.map(|s| [s.x as f32 / self.density, s.y as f32 / self.density].into()),
					max_size: self
						.info
						.max_size
						.map(|s| [s.x as f32 / self.density, s.y as f32 / self.density].into()),
					on_size_changed: FnWrapper(Box::new(|state, size_meters| {
						let size = Vector2::from([
							(size_meters.x * state.density) as u32,
							(size_meters.y * state.density) as u32,
						]);
						if let Some(shell) = state.panel_shell.as_ref() {
							shell.item().request_toplevel_resize(size.into());
						}
						state.info.size = size.into();
						state.cursor_pos.x = state.cursor_pos.x.clamp(0.0, size.x as f32);
						state.cursor_pos.y = state.cursor_pos.y.clamp(0.0, size.y as f32);
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
								shell.item().close_toplevel();
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
					&self.binder_dev,
					SurfaceId::Toplevel,
					self.info.size,
					Geometry {
						origin: Vector2::from([0; 2]).into(),
						size: self.info.size.into(),
					},
					&[Rect {
						origin: Vec2 { x: 0.0, y: 0.0 },
						size: Vec2 { x: 1.0, y: 1.0 },
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
									&self.binder_dev,
									self.info.size.into(),
									&self.panel_shell,
									panel_thickness,
									self.density,
								),
							)
						})
						.collect(),
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
								ResourceID::new_namespaced("flatland", "panel"),
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
		binder_dev: &PionBinderDevice,
		parent_size: Vector2<u32>,
		panel_item: &Option<Shell>,
		panel_thickness: f32,
		density: f32,
	) -> impl Element<ToplevelState> {
		reify_surface(
			panel_item,
			binder_dev,
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
							binder_dev,
							self.info.geometry.size.into(),
							panel_item,
							panel_thickness,
							density,
						),
					)
				})
				.collect(),
		)
		.dynamic()
	}
}

#[allow(clippy::too_many_arguments)]
fn reify_surface<E: Element<ToplevelState>>(
	panel_item: &Option<Shell>,
	binder_dev: &PionBinderDevice,
	surface_id: SurfaceId,
	parent_size: impl Into<Vector2<u32>>,
	geometry: Geometry,
	input_areas: &[Rect],
	z_offset: i32,
	thickness: f32,
	density: f32,
	children: FxHashMap<u64, E>,
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
			Derezzable::<ToplevelState>::new(
				|state| {
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
				ResourceID::new_namespaced("flatland", "panel"),
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
			Model::namespaced("flatland", "panel")
				.scl([
					geometry.size.x as f32 / density,
					geometry.size.y as f32 / density,
					thickness,
				])
				.build()
		}))
		.maybe_child(panel_item.is_none().then(|| {
			PanelItemAcceptorElement::<ToplevelState>::new(
				binder_dev,
				shape.clone(),
				|state, shell| {
					state.panel_shell.replace(shell);
				},
			)
			.build()
		}))
		// inputs
		.maybe_child((!input_areas.is_empty()).then(move || {
			Spatial::default()
				.build()
				.child(
					KeyboardHandler::<ToplevelState>::new(shape.clone(), move |state, key_data| {
						if let Some(shell) = state.panel_shell.as_ref() {
							shell.item().key(
								surface_id,
								KeymapId {
									id: key_data.keymap_id,
								},
								key_data.key,
								key_data.pressed,
							);
						}
					})
					.build(),
				)
				.child(
					MouseHandler::<ToplevelState>::new(
						shape,
						move |state, button, pressed| {
							if let Some(shell) = state.panel_shell.as_ref() {
								let _ = shell.item().pointer_button(surface_id, button, pressed);
							}
						},
						move |state, motion| {
							if let Some(shell) = state.panel_shell.as_ref() {
								let _ = shell.item().relative_pointer_motion(
									surface_id,
									Vector2::from([motion.x, -motion.y]).into(),
								);
							}
							state.cursor_pos.x += motion.x;
							state.cursor_pos.y -= motion.y;
							state.cursor_pos.x =
								state.cursor_pos.x.clamp(0.0, state.info.size.x as f32);
							state.cursor_pos.y =
								state.cursor_pos.y.clamp(0.0, state.info.size.y as f32);
							if let Some(shell) = state.panel_shell.as_ref() {
								let _ = shell
									.item()
									.absolute_pointer_motion(surface_id, state.cursor_pos.into());
							}
						},
						move |state, scroll_discrete| {
							if let Some(shell) = state.panel_shell.as_ref() {
								shell.item().pointer_scroll_discrete(
									surface_id,
									Vector2::from([
										scroll_discrete.x * state.mouse_scroll_multiplier,
										-scroll_discrete.y * state.mouse_scroll_multiplier,
									])
									.into(),
									// TODO: forward this over the non-spatial-input protocol
									ScrollSource::Wheel,
								);
							}
						},
						move |state, scroll_continuous| {
							if let Some(shell) = state.panel_shell.as_ref() {
								shell.item().pointer_scroll_pixels(
									surface_id,
									Vector2::from([
										scroll_continuous.x * state.mouse_scroll_multiplier,
										-scroll_continuous.y * state.mouse_scroll_multiplier,
									])
									.into(),
									// TODO: forward this over the non-spatial-input protocol
									ScrollSource::Wheel,
								);
							}
						},
					)
					.build(),
				)
				.child(
					PointerPlane::<ToplevelState>::default()
						.physical_size([size_meters.x, size_meters.y])
						.thickness(thickness)
						.on_mouse_button(move |state, button, pressed| {
							if let Some(shell) = state.panel_shell.as_ref() {
								let _ = shell.item().pointer_button(surface_id, button, pressed);
							}
						})
						.on_pointer_motion(move |state, pos| {
							let pixel_pos = [pos.x * state.density, pos.y * state.density];
							state.cursor_pos = pixel_pos.into();
							state.cursor_pos.x =
								state.cursor_pos.x.clamp(0.0, state.info.size.x as f32);
							state.cursor_pos.y =
								state.cursor_pos.y.clamp(0.0, state.info.size.y as f32);
							if let Some(shell) = state.panel_shell.as_ref() {
								let _ = shell
									.item()
									.absolute_pointer_motion(surface_id, state.cursor_pos.into());
							}
						})
						.on_scroll(move |state, scroll| {
							if let Some(scroll_continuous) = scroll.scroll_continuous
								&& let Some(shell) = state.panel_shell.as_ref()
							{
								shell.item().pointer_scroll_pixels(
									surface_id,
									Vector2::from([
										scroll_continuous.x * state.mouse_scroll_multiplier,
										-scroll_continuous.y * state.mouse_scroll_multiplier,
									])
									.into(),
									ScrollSource::Continuous,
								);
							}
							if let Some(scroll_discrete) = scroll.scroll_discrete
								&& let Some(shell) = state.panel_shell.as_ref()
							{
								shell.item().pointer_scroll_pixels(
									surface_id,
									Vector2::from([
										scroll_discrete.x * state.mouse_scroll_multiplier,
										-scroll_discrete.y * state.mouse_scroll_multiplier,
									])
									.into(),
									ScrollSource::Continuous,
								);
							}
							// TODO: figure out how to send this only when scroll actually stops,
							// instead of every frame without scroll
							if scroll.scroll_continuous.is_none()
								&& scroll.scroll_discrete.is_none()
								&& let Some(shell) = state.panel_shell.as_ref()
							{
								shell.item().pointer_scroll_stop(surface_id);
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
									Vector2::from([
										position.x * state.density,
										position.y * state.density,
									])
									.into(),
								);
							}
						})
						.on_touch_move(|state, id, position| {
							if let Some(shell) = state.panel_shell.as_ref() {
								let _ = shell.item().touch_move(
									id,
									Vector2::from([
										position.x * state.density,
										position.y * state.density,
									])
									.into(),
								);
							}
						})
						.on_touch_up(|state, id| {
							if let Some(shell) = state.panel_shell.as_ref() {
								let _ = shell.item().touch_up(id);
							}
						})
						.build(),
				)
		}))
		.stable_children(children)
}
