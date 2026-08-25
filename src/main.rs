use close_button::ExposureButton;
use glam::{Quat, Vec3, vec2, vec3};
use gluon::Node;
use initial_panel_placement::InitialPanelPlacement;
use pointer_input::PointerPlane;
use resize_handles::ResizeHandles;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use stardust_xr_asteroids::{
	Context, CustomElement, Element, Entity, FnWrapper, Migrate, Reify, Tasker, Transformable as _,
	client::{ClientState, run},
	components::{Derezzable, KeyboardHandler, MouseHandler},
	elements::{Handle, Model, Spatial, Text},
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
	SurfaceUpdateTarget,
};
use stardust_xr_panel_item_asteroids::{
	panel_item_acceptor::PanelItemAcceptor,
	panel_shell::{PanelShell, PanelShellHandler},
	surface_model::SurfaceModel,
};
use std::f32::consts::FRAC_PI_2;
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

	run::<Flatland>(&[&project_local_resources!("data")])
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
			let parent_id = *parent_id;
			let child_id = child_info.id;
			if !add_to_parent(
				children,
				parent_id,
				ChildState {
					info: child_info,
					children: Vec::new(),
				},
			) {
				tracing::warn!(
					child_id,
					parent_id,
					"add_child: parent surface not found, dropping child"
				);
			}
		}
	}
}

/// Returns whether `parent_id` was found and `new_child` was inserted.
fn add_to_parent(children: &mut [ChildState], parent_id: u64, new_child: ChildState) -> bool {
	for child in children.iter_mut() {
		if child.info.id == parent_id {
			child.children.push(new_child);
			return true;
		}
		if add_to_parent(&mut child.children, parent_id, new_child.clone()) {
			return true;
		}
	}
	false
}
pub fn update_child_geometry(children: &mut [ChildState], id: u64, geometry: Geometry) {
	if !update_child_geometry_inner(children, id, geometry) {
		tracing::warn!(id, "update_child_geometry: surface not found");
	}
}
fn update_child_geometry_inner(children: &mut [ChildState], id: u64, geometry: Geometry) -> bool {
	for child in children.iter_mut() {
		if child.info.id == id {
			child.info.geometry = geometry;
			return true;
		}
		if update_child_geometry_inner(&mut child.children, id, geometry) {
			return true;
		}
	}
	false
}
pub fn remove_child(children: &mut Vec<ChildState>, id: u64) {
	if !remove_child_inner(children, id) {
		tracing::warn!(id, "remove_child: surface not found");
	}
}
fn remove_child_inner(children: &mut Vec<ChildState>, id: u64) -> bool {
	let mut found = false;
	children.retain_mut(|child| {
		if child.info.id == id {
			found = true;
			return false;
		}
		found |= remove_child_inner(&mut child.children, id);
		true
	});
	found
}
pub fn process_initial_children(children: Vec<ChildInfo>) -> Vec<ChildState> {
	let mut child_states = Vec::new();
	for child in children {
		add_child(&mut child_states, child);
	}
	child_states
}

impl Migrate for Flatland {
	type Old = Self;
}
impl ClientState for Flatland {
	const APP_ID: &'static str = "org.stardustxr.Flatland";

	fn initial_state_update(&mut self) {
		use clap::Parser;

		/// Flat slab panel shell to place in the environment with basic operation
		#[derive(clap::Parser)]
		#[command(version, about, long_about = None)]
		struct Args {
			#[arg(short, long, default_value_t = false)]
			derez_with_item: bool,
		}

		let args = Args::parse();
		self.derez_with_item = args.derez_with_item;
	}
}
impl Default for Flatland {
	fn default() -> Self {
		Self {
			pose: Posef::default(),
			size: [0.21, 0.16].into(),

			panel_item: None,
			density: 3000.0,
			mouse_scroll_multiplier: 1.0,
			derez_with_item: false,
			derez_with_item_temp: false,
		}
	}
}

#[derive(Debug, Clone)]
pub struct ChildState {
	info: ChildInfo,
	children: Vec<ChildState>,
}

#[derive(Debug)]
struct PanelItem {
	pub shell: Shell,
	pub item: stardust_xr_panel_item::panel_item::PanelItem,
	release_pos_offset: Vec3,

	pub parent: Option<u64>,
	pub title: Option<String>,
	pub app_id: Option<String>,
	pub min_size: Option<Size2>,
	pub max_size: Option<Size2>,

	/// in px
	cursor_pos: Vec2F,
	cursor: Option<Geometry>,

	children: Vec<ChildState>,
}

type Shell = Node<PanelShellHandler>;
#[derive(Debug, Serialize, Deserialize)]
pub struct Flatland {
	/// meters
	size: Vec2F,
	pose: Posef,
	derez_with_item: bool,
	#[serde(skip)]
	derez_with_item_temp: bool,

	#[serde(skip)]
	panel_item: Option<PanelItem>,

	/// pixels per meter
	density: f32,
	mouse_scroll_multiplier: f32,
}
impl Flatland {
	fn size_px(&self) -> Size2 {
		Size2 {
			x: (self.size.x * self.density) as u32,
			y: (self.size.y * self.density) as u32,
		}
	}
	fn set_size_px(&mut self, size: impl Into<Size2>) {
		let size = size.into();
		self.size.x = (size.x as f32) / self.density;
		self.size.y = (size.y as f32) / self.density;
		self.clamp_pointer();
	}

	fn move_pointer(
		&mut self,
		surface_id: SurfaceId,
		delta: impl Into<Vec2F>,
		timestamp: Option<Timestamp>,
	) {
		let Some(cursor_pos) = self.panel_item.as_ref().map(|i| i.cursor_pos) else {
			return;
		};

		let delta = delta.into();
		self.set_pointer(
			surface_id,
			Some(delta),
			[cursor_pos.x + delta.x, cursor_pos.y + delta.y],
			timestamp,
		);
	}
	fn set_pointer(
		&mut self,
		surface_id: SurfaceId,
		motion: Option<Vec2F>,
		new_pos: impl Into<Vec2F>,
		timestamp: Option<Timestamp>,
	) {
		if let Some(shell) = self.panel_item.as_mut() {
			shell.cursor_pos = new_pos.into();
			let _ = shell
				.item
				.pointer_motion(surface_id, motion, shell.cursor_pos, timestamp);
		}
		self.clamp_pointer();
	}
	fn clamp_pointer(&mut self) {
		let size_px = self.size_px();
		let Some(shell) = self.panel_item.as_mut() else {
			return;
		};

		shell.cursor_pos.x = shell
			.cursor_pos
			.x
			.clamp(0.0, size_px.x.saturating_sub(1) as f32);
		shell.cursor_pos.y = shell
			.cursor_pos
			.y
			.clamp(0.0, size_px.y.saturating_sub(1) as f32);
	}
	fn resize(&mut self, new_size: impl Into<Vec2F>) {
		let old_size_px = self.size_px();
		self.size = new_size.into();
		let Some(shell) = self.panel_item.as_ref() else {
			return;
		};
		let mut size_px = self.size_px();
		let min_size = shell.min_size.unwrap_or([0; 2].into());
		let max_size = shell.max_size.unwrap_or([u32::MAX; 2].into());
		size_px.x = size_px.x.clamp(min_size.x, max_size.x);
		size_px.y = size_px.y.clamp(min_size.y, max_size.y);

		if old_size_px.x != size_px.x || old_size_px.y != size_px.y {
			_ = shell.item.request_toplevel_resize(size_px);
		}

		self.set_size_px(size_px);
	}
}
impl Reify for Flatland {
	fn reify(
		&self,
		context: &Context,
		_tasks: impl Tasker<Self>,
	) -> impl stardust_xr_asteroids::Element<Self> {
		let panel_thickness = 0.01;

		let app_name = self
			.panel_item
			.as_ref()
			.and_then(|i| i.app_id.as_ref())
			.map(|id| id.split('.').next_back().unwrap_or_default());
		let title_text = match (
			self.panel_item.as_ref().and_then(|i| i.title.as_ref()),
			app_name,
		) {
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
		InitialPanelPlacement.build().child(
			ResizeHandles::<Flatland> {
				reparentable: true,
				pose: self.pose,
				size: self.size,
				min_size: self
					.panel_item
					.as_ref()
					.and_then(|s| s.min_size)
					.map(|s| [s.x as f32 / self.density, s.y as f32 / self.density].into()),
				max_size: self
					.panel_item
					.as_ref()
					.and_then(|s| s.max_size)
					.map(|s| [s.x as f32 / self.density, s.y as f32 / self.density].into()),
				on_change: FnWrapper(Box::new(|state, pose, size_meters| {
					state.pose = pose;
					state.resize(size_meters);
					state.clamp_pointer();
				})),
			}
			.build()
			.maybe_child(self.panel_item.is_some().then(|| {
				Handle::new([0.0, -self.size.y / 2.0, 0.0], |state: &mut Self, pos| {
					let Some(item) = &mut state.panel_item else {
						return;
					};
					item.release_pos_offset = Vec3::from(pos) - vec3(0.0, -state.size.y / 2.0, 0.0);
				})
				.on_release(|state: &mut Self, _pos| {
					state.panel_item.take();
				})
				.head_offset([0.0, -0.02, 0.0])
				.build()
			}))
			.maybe_child(self.panel_item.as_ref().map(|item| {
				let context = context.clone();
				PanelShell::new(&item.shell, move |state: &mut Self| {
					_ = state.panel_item.take();
					if state.derez_with_item || state.derez_with_item_temp {
						context.stop();
					}
				})
				.on_toplevel_resolution_changed(|state: &mut Self, _item, size| {
					state.set_size_px(size);
				})
				.on_toplevel_max_size_changed(|state: &mut Self, _item, size| {
					let Some(item) = &mut state.panel_item else {
						return;
					};

					item.max_size = size;
					state.resize(state.size);
				})
				.on_toplevel_min_size_changed(|state: &mut Self, _item, size| {
					let Some(item) = &mut state.panel_item else {
						return;
					};
					item.min_size = size;
					state.resize(state.size);
				})
				.on_toplevel_app_id_changed(|state: &mut Self, _, app_id| {
					let Some(item) = &mut state.panel_item else {
						return;
					};
					item.app_id.replace(app_id);
				})
				.on_toplevel_title_changed(|state: &mut Self, _, title| {
					let Some(item) = &mut state.panel_item else {
						return;
					};
					item.title.replace(title);
				})
				.cursor_visuals_changed(|state: &mut Self, _, geometry| {
					let Some(item) = &mut state.panel_item else {
						return;
					};
					item.cursor = geometry;
				})
				.new_child(|state: &mut Self, _, child_state| {
					let Some(item) = &mut state.panel_item else {
						return;
					};
					add_child(&mut item.children, child_state);
				})
				.child_moved(|state: &mut Self, _, id, geometry| {
					let Some(item) = &mut state.panel_item else {
						return;
					};
					update_child_geometry(&mut item.children, id, geometry);
				})
				.child_removed(|state: &mut Self, _, id| {
					let Some(item) = &mut state.panel_item else {
						return;
					};
					remove_child(&mut item.children, id);
				})
				.pos(vec3(0.0, -self.size.y / 2.0, 0.0) + item.release_pos_offset)
				.build()
			}))
			.child(
				// Close button
				ExposureButton::<Self> {
					transform: Transform::from_translation([
						self.size.x / 2.0,
						self.size.y / -2.0,
						panel_thickness / 2.0,
					]),
					thickness: panel_thickness,
					gain: 2.0,
					on_click: FnWrapper(Box::new({
						let context = context.clone();
						move |state: &mut Self| {
							if let Some(item) = state.panel_item.as_ref() {
								_ = item.item.close_toplevel();
							} else {
								context.stop();
							}
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
						bounds: [self.size.y, panel_thickness].into(),
						fit: TextFit::Squeeze,
						anchor_align_x: XAlign::Left,
						anchor_align_y: YAlign::Bottom,
					})
					.pos([
						self.size.x / 2.0 + 0.0005,
						self.size.y / 2.0 - 0.001,
						panel_thickness / 2.0,
					])
					.rot(Quat::from_rotation_z(-FRAC_PI_2) * Quat::from_rotation_x(-FRAC_PI_2))
					.build(),
			)
			.child(reify_surface(
				self.panel_item.as_ref().map(|i| &i.shell),
				SurfaceId::Toplevel,
				self.size_px(),
				Geometry {
					origin: [0; 2].into(),
					size: self.size_px(),
				},
				&[Rect {
					origin: [0.0; 2].into(),
					size: [1.0; 2].into(),
				}],
				0,
				panel_thickness,
				self.density,
				self.panel_item
					.iter()
					.flat_map(|item| {
						item.children.iter().map(|child| {
							(
								child.info.id,
								child.reify(
									self.size_px(),
									Some(&item.shell),
									panel_thickness,
									self.density,
									self.mouse_scroll_multiplier,
								),
							)
						})
					})
					.collect(),
				self.mouse_scroll_multiplier,
			))
			.maybe_child(
				// cursor
				self.panel_item.as_ref().and_then(|item| {
					let cursor_geometry = item.cursor?;
					let cursor_pos = vec2(item.cursor_pos.x, item.cursor_pos.y);
					let geometry_origin = vec2(
						cursor_geometry.origin.x as f32,
						cursor_geometry.origin.y as f32,
					);
					let geometry_size_half =
						vec2(cursor_geometry.size.x as f32, cursor_geometry.size.y as f32) / 2.0;
					let panel_size_px_half =
						vec2(self.size_px().x as f32, self.size_px().y as f32) / 2.0;

					let pos_px =
						cursor_pos - panel_size_px_half + geometry_size_half - geometry_origin;
					let pos_m = pos_px * vec2(1.0, -1.0) / self.density;

					Some(
						SurfaceModel::new(
							&item.shell,
							SurfaceUpdateTarget::Cursor,
							Resource::Namespaced {
								namespace: Flatland::APP_ID.into(),
								path: "panel".into(),
							},
							"Panel",
						)
						.pos([pos_m.x, pos_m.y, 0.001])
						.scl([
							cursor_geometry.size.x as f32 / self.density,
							cursor_geometry.size.y as f32 / self.density,
							panel_thickness,
						])
						.build(),
					)
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
		panel_shell: Option<&Shell>,
		panel_thickness: f32,
		density: f32,
		scroll_multiplier: f32,
	) -> impl Element<Flatland> {
		reify_surface(
			panel_shell,
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
							panel_shell,
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
fn reify_surface<S: Into<Size2>, E: Element<Flatland>>(
	panel_shell: Option<&Shell>,
	surface_id: SurfaceId,
	parent_size: S,
	geometry: Geometry,
	input_areas: &[Rect],
	z_offset: i32,
	thickness: f32,
	density: f32,
	children: FxHashMap<u64, E>,
	scroll_multiplier: f32,
) -> impl Element<Flatland> + use<S, E> {
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

	Entity::new(Shape::Box {
		size: [size_meters.x, size_meters.y, thickness].into(),
	})
	.pos(
		(origin_meters - parent_origin_meters + (size_meters / vec2(2.0, -2.0)))
			.extend(thickness * (z_offset as f32)),
	)
	.component(Derezzable::<Flatland>::new(|state| {
		state.derez_with_item_temp = true;
		if let Some(item) = &state.panel_item {
			_ = item.item.close_toplevel();
		}
	}))
	.component(PanelItemAcceptor::<Flatland>::new(|state, shell| {
		_ = shell.item().request_toplevel_resize(state.size_px());
		state.panel_item.replace(PanelItem {
			item: shell.item().clone(),
			shell,
			release_pos_offset: [0.0, -0.02, 0.0].into(),
			parent: None,
			title: None,
			app_id: None,
			min_size: None,
			max_size: None,
			cursor_pos: [0.0; 2].into(),
			cursor: None,
			children: vec![],
		});
	}))
	.component((!input_areas.is_empty()).then(|| {
		KeyboardHandler::<Flatland>::new().on_key_async({
			let panel_item = panel_shell.as_ref().map(|v| v.item().clone());
			move |key_event, timestamp| {
				if let Some(item) = &panel_item {
					_ = item.key(
						surface_id,
						key_event.keycode,
						key_event.pressed,
						ModifierState {
							depressed: key_event.modifiers.depressed,
							latched: key_event.modifiers.latched,
							locked: key_event.modifiers.locked,
							layout_group: key_event.modifiers.layout_group,
						},
						key_event.keymap,
						timestamp,
					);
				}
			}
		})
	}))
	.component((!input_areas.is_empty()).then(|| {
		MouseHandler::<Flatland>::new()
			.on_button_async({
				let panel_item = panel_shell.as_ref().map(|v| v.item().clone());
				move |button, pressed, timestamp| {
					if let Some(item) = &panel_item {
						let _ = item.pointer_button(surface_id, button, pressed, timestamp);
					}
				}
			})
			.on_motion(move |state, motion, timestamp| {
				state.move_pointer(surface_id, [motion.x, -motion.y], timestamp);
			})
			.on_scroll_discrete_async({
				let panel_item = panel_shell.as_ref().map(|v| v.item().clone());
				move |scroll_discrete, source, timestamp| {
					use stardust_xr_asteroids::components::ScrollSource as MoleculesSource;
					if let Some(item) = &panel_item {
						_ = item.pointer_scroll_discrete(
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
						);
					}
				}
			})
			.on_scroll_continuous_async({
				let panel_item = panel_shell.as_ref().map(|v| v.item().clone());
				move |scroll_continuous, source, timestamp| {
					use stardust_xr_asteroids::components::ScrollSource as MoleculesSource;
					if let Some(item) = &panel_item {
						_ = item.pointer_scroll_pixels(
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
						);
					}
				}
			})
	}))
	.build()
	.maybe_child(panel_shell.as_ref().map(|item| {
		SurfaceModel::new(
			item,
			surface_id,
			Resource::Namespaced {
				namespace: Flatland::APP_ID.into(),
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
	.maybe_child(panel_shell.is_none().then(|| {
		Model::namespaced(Flatland::APP_ID, "panel")
			.scl([
				geometry.size.x as f32 / density,
				geometry.size.y as f32 / density,
				thickness,
			])
			.build()
	}))
	// inputs
	.maybe_child((!input_areas.is_empty()).then(move || {
		Spatial::default()
			.build()
			.child(
				PointerPlane::<Flatland>::default()
					.physical_size([size_meters.x, size_meters.y])
					.thickness(thickness)
					.on_mouse_button(move |state, button, pressed| {
						if let Some(item) = &state.panel_item {
							// TODO: somehow get a timestamp for this?
							let _ = item.item.pointer_button(surface_id, button, pressed, None);
						}
					})
					.on_pointer_motion(move |state, pos| {
						let pixel_pos = [pos.x * state.density, pos.y * state.density];
						state.set_pointer(surface_id, None, pixel_pos, None);
					})
					.on_scroll(move |state, scroll| {
						if let Some(scroll_continuous) = scroll.scroll_continuous
							&& let Some(item) = &state.panel_item
						{
							_ = item.item.pointer_scroll_pixels(
								surface_id,
								[
									scroll_continuous.x * state.mouse_scroll_multiplier,
									-scroll_continuous.y * state.mouse_scroll_multiplier,
								]
								.into(),
								ScrollSource::Continuous,
								// TODO: somehow get a timestamp for this?
								None,
							);
						}
						if let Some(scroll_discrete) = scroll.scroll_discrete
							&& let Some(item) = &state.panel_item
						{
							_ = item.item.pointer_scroll_pixels(
								surface_id,
								[
									scroll_discrete.x * state.mouse_scroll_multiplier,
									-scroll_discrete.y * state.mouse_scroll_multiplier,
								]
								.into(),
								ScrollSource::Continuous,
								// TODO: somehow get a timestamp for this?
								None,
							);
						}
						// TODO: figure out how to send this only when scroll actually stops,
						// instead of every frame without scroll
						if scroll.scroll_continuous.is_none()
							&& scroll.scroll_discrete.is_none()
							&& let Some(item) = &state.panel_item
						{
							// TODO: somehow get a timestamp for this?
							_ = item.item.pointer_scroll_stop(surface_id, None);
						}
					})
					.build(),
			)
			.child(
				TouchPlane::<Flatland>::default()
					.physical_size([size_meters.x, size_meters.y])
					.thickness(thickness)
					.on_touch_down(move |state, id, position| {
						if let Some(item) = &state.panel_item {
							let _ = item.item.touch_down(
								surface_id,
								id,
								[position.x * state.density, position.y * state.density].into(),
								// TODO: somehow get a timestamp for this?
								None,
							);
						}
					})
					.on_touch_move(|state, id, position| {
						if let Some(item) = &state.panel_item {
							let _ = item.item.touch_move(
								id,
								[position.x * state.density, position.y * state.density].into(),
								// TODO: somehow get a timestamp for this?
								None,
							);
						}
					})
					.on_touch_up(|state, id| {
						if let Some(item) = &state.panel_item {
							// TODO: somehow get a timestamp for this?
							let _ = item.item.touch_up(id, None);
						}
					})
					.build(),
			)
	}))
	.stable_children(children)
}
