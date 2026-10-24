use close_button::ExposureButton;
use glam::{Quat, Vec3, vec2, vec3};
use gluon_ipc::Node;
use initial_panel_placement::InitialPanelPlacement;
use resize_handles::ResizeHandles;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use split_input::InputSurface;
use stardust_xr_asteroids::{
	CloneFnWrapper, Context, CustomElement, Element, Entity, FnWrapper, Migrate, Reify, Tasker,
	Transformable as _,
	client::{ClientState, run},
	components::{Derezzable, Poseable},
	elements::{Handle, Text},
};
use stardust_xr_fusion::{
	drawable::{TextBounds, TextFit, XAlign, YAlign},
	fields::Shape,
	project_local_resources,
	spatial::Transform,
	types::{Posef, Size2, Timestamp, Vec2F},
};
use stardust_xr_panels::panel_item::{Geometry, SurfaceId, SurfaceUpdateTarget};
use stardust_xr_panels_asteroids::{
	panel_item_acceptor::PanelItemAcceptor,
	panel_shell::{PanelShell, PanelShellHandler},
	surface_model::SurfaceModel,
};
use std::{f32::consts::FRAC_PI_2, sync::Arc};
use surface_tree::{ChildState, add_child, remove_child, update_child_geometry};
use surfaces::{Layer, Surfaces, panel_resource};
use touch_input::TouchHover;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _};

pub mod close_button;
pub mod grab_ball;
pub mod initial_panel_placement;
// pub mod panel_shell_transfer;
pub mod pointer_input;
pub mod resize_handles;
pub mod split_input;
pub mod surface_input;
pub mod surface_tree;
pub mod surfaces;
pub mod touch_input;

const PANEL_THICKNESS: f32 = 0.01;

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

type Shell = Node<PanelShellHandler>;

#[derive(Debug)]
struct PanelItem {
	shell: Shell,
	item: stardust_xr_panels::panel_item::PanelItem,
	release_pos_offset: Vec3,

	title: Option<String>,
	app_id: Option<String>,
	min_size: Option<Size2>,
	max_size: Option<Size2>,

	/// in px
	cursor_pos: Vec2F,
	cursor: Option<Geometry>,

	children: Vec<ChildState>,
}
impl PanelItem {
	fn new(shell: Shell) -> Self {
		PanelItem {
			item: shell.item().clone(),
			shell,
			release_pos_offset: Vec3::ZERO,
			title: None,
			app_id: None,
			min_size: None,
			max_size: None,
			cursor_pos: [0.0; 2].into(),
			cursor: None,
			children: Vec::new(),
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Flatland {
	/// meters
	screen_size: Vec2F,
	screen_pose: Posef,
	// if the input is split away from the panel, where is it?
	split_input_surface: Option<InputSurface>,
	#[serde(skip)]
	pulling_split: bool,
	#[serde(skip)]
	touch_hovers: FxHashMap<SurfaceId, Vec<TouchHover>>,

	derez_with_item: bool,
	#[serde(skip)]
	derez_with_item_temp: bool,

	#[serde(skip)]
	panel_item: Option<PanelItem>,

	/// pixels per meter
	density: f32,
	mouse_scroll_multiplier: f32,
}
impl Default for Flatland {
	fn default() -> Self {
		Self {
			screen_size: [0.21, 0.16].into(),
			screen_pose: Posef::default(),
			split_input_surface: None,
			pulling_split: false,
			touch_hovers: FxHashMap::default(),

			derez_with_item: false,
			derez_with_item_temp: false,

			panel_item: None,

			density: 3000.0,
			mouse_scroll_multiplier: 1.0,
		}
	}
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

		self.derez_with_item = Args::parse().derez_with_item;
	}
}
impl Flatland {
	fn size_px(&self) -> Size2 {
		Size2 {
			x: (self.screen_size.x * self.density) as u32,
			y: (self.screen_size.y * self.density) as u32,
		}
	}
	fn px_to_m(&self, size: Size2) -> Vec2F {
		[size.x as f32 / self.density, size.y as f32 / self.density].into()
	}
	fn set_size_px(&mut self, size: impl Into<Size2>) {
		self.screen_size = self.px_to_m(size.into());
		self.clamp_pointer();
	}
	fn resize(&mut self, new_size: impl Into<Vec2F>) {
		let old_size_px = self.size_px();
		self.screen_size = new_size.into();
		let Some(item) = self.panel_item.as_ref() else {
			return;
		};
		let mut size_px = self.size_px();
		let min_size = item.min_size.unwrap_or([0; 2].into());
		let max_size = item.max_size.unwrap_or([u32::MAX; 2].into());
		size_px.x = size_px.x.clamp(min_size.x, max_size.x);
		size_px.y = size_px.y.clamp(min_size.y, max_size.y);

		if old_size_px.x != size_px.x || old_size_px.y != size_px.y {
			_ = item.item.request_toplevel_resize(size_px);
		}

		self.set_size_px(size_px);
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
		if let Some(item) = self.panel_item.as_mut() {
			item.cursor_pos = new_pos.into();
			let _ = item
				.item
				.pointer_motion(surface_id, motion, item.cursor_pos, timestamp);
		}
		self.clamp_pointer();
	}
	fn clamp_pointer(&mut self) {
		let size_px = self.size_px();
		let Some(item) = self.panel_item.as_mut() else {
			return;
		};
		item.cursor_pos.x = item
			.cursor_pos
			.x
			.clamp(0.0, size_px.x.saturating_sub(1) as f32);
		item.cursor_pos.y = item
			.cursor_pos
			.y
			.clamp(0.0, size_px.y.saturating_sub(1) as f32);
	}

	fn title(&self) -> String {
		let Some(item) = &self.panel_item else {
			return String::new();
		};
		let app_name = item
			.app_id
			.as_ref()
			.map(|id| id.split('.').next_back().unwrap_or_default());
		match (item.title.as_deref(), app_name) {
			(Some(title), Some(app_name)) if title != app_name => format!("{title} - {app_name}"),
			(Some(title), _) => title.to_string(),
			(None, Some(app_name)) => app_name.to_string(),
			(None, None) => String::new(),
		}
	}
}

impl Reify for Flatland {
	fn reify(
		&self,
		context: &Context,
		_tasks: impl Tasker<Self>,
		_props: (),
	) -> impl Element<Self> {
		InitialPanelPlacement
			.build()
			.maybe_child(self.split_tethers())
			.maybe_child(self.split_input(context))
			.child(self.screen(context))
	}
}
impl Flatland {
	fn screen(&self, context: &Context) -> impl Element<Self> + use<> {
		let item = self.panel_item.as_ref();
		ResizeHandles::<Self> {
			pose: self.screen_pose,
			size: self.screen_size,
			min_size: item.and_then(|i| i.min_size).map(|s| self.px_to_m(s)),
			max_size: item.and_then(|i| i.max_size).map(|s| self.px_to_m(s)),
			on_change: CloneFnWrapper(Arc::new(|state, pose, size| {
				state.screen_pose = pose;
				state.resize(size);
			})),
		}
		.build()
		.child(self.slab(context))
		.maybe_child(self.eject_handle())
		.maybe_child(self.split_handle())
		.maybe_child(self.panel_shell(context))
		.child(self.close_button(context))
		.child(self.side_text())
		.child(self.surfaces(
			context,
			Layer::Screen {
				planes: self.split_input_surface.is_none(),
			},
		))
		.maybe_child(self.cursor())
	}

	fn slab(&self, context: &Context) -> impl Element<Self> + use<> {
		let context = context.clone();
		Entity::new(Shape::Box {
			size: [self.screen_size.x, self.screen_size.y, PANEL_THICKNESS].into(),
		})
		.component(Poseable::new(|state: &mut Self, pose| {
			state.screen_pose = pose
		}))
		.component(Derezzable::<Self>::new(move |state| {
			if let Some(item) = &state.panel_item {
				state.derez_with_item_temp = true;
				_ = item.item.close_toplevel();
			} else {
				context.stop();
			}
		}))
		.component(PanelItemAcceptor::<Self>::new(|state, shell| {
			_ = shell.item().request_toplevel_resize(state.size_px());
			state.panel_item = Some(PanelItem::new(shell));
		}))
		.build()
	}

	fn eject_handle(&self) -> Option<impl Element<Self> + use<>> {
		self.panel_item.is_some().then(|| {
			Handle::new(
				[0.0, -self.screen_size.y / 2.0, 0.0],
				|state: &mut Self, pos| {
					if let Some(item) = &mut state.panel_item {
						item.release_pos_offset = pos.into();
					}
				},
			)
			.on_release(|state: &mut Self, _| state.panel_item = None)
			.head_offset([0.0, -0.02, 0.0])
			.build()
		})
	}

	fn panel_shell(&self, context: &Context) -> Option<impl Element<Self> + use<>> {
		let item = self.panel_item.as_ref()?;
		let context = context.clone();
		Some(
			PanelShell::new(&item.shell, move |state: &mut Self| {
				state.panel_item = None;
				if state.derez_with_item || state.derez_with_item_temp {
					context.stop();
				}
			})
			.on_toplevel_resolution_changed(|state: &mut Self, _, size| state.set_size_px(size))
			.on_toplevel_max_size_changed(|state: &mut Self, _, size| {
				if let Some(item) = &mut state.panel_item {
					item.max_size = size;
				}
				state.resize(state.screen_size);
			})
			.on_toplevel_min_size_changed(|state: &mut Self, _, size| {
				if let Some(item) = &mut state.panel_item {
					item.min_size = size;
				}
				state.resize(state.screen_size);
			})
			.on_toplevel_app_id_changed(|state: &mut Self, _, app_id| {
				if let Some(item) = &mut state.panel_item {
					item.app_id = Some(app_id);
				}
			})
			.on_toplevel_title_changed(|state: &mut Self, _, title| {
				if let Some(item) = &mut state.panel_item {
					item.title = Some(title);
				}
			})
			.cursor_visuals_changed(|state: &mut Self, _, geometry| {
				if let Some(item) = &mut state.panel_item {
					item.cursor = geometry;
				}
			})
			.new_child(|state: &mut Self, _, info| {
				if let Some(item) = &mut state.panel_item {
					add_child(&mut item.children, info);
				}
			})
			.child_moved(|state: &mut Self, _, id, geometry| {
				if let Some(item) = &mut state.panel_item {
					update_child_geometry(&mut item.children, id, geometry);
				}
			})
			.child_removed(|state: &mut Self, _, id| {
				if let Some(item) = &mut state.panel_item {
					remove_child(&mut item.children, id);
				}
			})
			.pos(vec3(0.0, 0.0, PANEL_THICKNESS) + item.release_pos_offset)
			.build(),
		)
	}

	fn close_button(&self, context: &Context) -> impl Element<Self> + use<> {
		let context = context.clone();
		ExposureButton::<Self> {
			transform: Transform::from_translation([
				self.screen_size.x / 2.0,
				self.screen_size.y / -2.0,
				PANEL_THICKNESS / 2.0,
			]),
			thickness: PANEL_THICKNESS,
			gain: 2.0,
			on_click: FnWrapper(Box::new(move |state: &mut Self| {
				if let Some(item) = &state.panel_item {
					_ = item.item.close_toplevel();
				} else {
					context.stop();
				}
			})),
		}
		.build()
	}

	fn side_text(&self) -> impl Element<Self> + use<> {
		Text::new(self.title())
			.character_height(PANEL_THICKNESS * 0.75)
			.align_x(XAlign::Left)
			.align_y(YAlign::Center)
			.bounds(TextBounds {
				bounds: [self.screen_size.y, PANEL_THICKNESS].into(),
				fit: TextFit::Squeeze,
				anchor_align_x: XAlign::Left,
				anchor_align_y: YAlign::Bottom,
			})
			.pos([
				self.screen_size.x / 2.0 + 0.0005,
				self.screen_size.y / 2.0 - 0.001,
				PANEL_THICKNESS / 2.0,
			])
			.rot(Quat::from_rotation_z(-FRAC_PI_2) * Quat::from_rotation_x(-FRAC_PI_2))
			.build()
	}

	fn surfaces(&self, context: &Context, layer: Layer) -> impl Element<Self> + use<> {
		Surfaces {
			shell: self.panel_item.as_ref().map(|i| &i.shell),
			layer,
			touch_hovers: &self.touch_hovers,
			accent: context.accent_color.color(),
			thickness: PANEL_THICKNESS,
			density: self.density,
			scroll_multiplier: self.mouse_scroll_multiplier,
		}
		.toplevel(
			self.size_px(),
			self.panel_item.as_ref().map_or(&[], |i| &i.children),
		)
	}

	fn cursor(&self) -> Option<impl Element<Self> + use<>> {
		let item = self.panel_item.as_ref()?;
		let geometry = item.cursor?;
		let screen = vec2(self.size_px().x as f32, self.size_px().y as f32);
		let origin = vec2(geometry.origin.x as f32, geometry.origin.y as f32);
		let size = vec2(geometry.size.x as f32, geometry.size.y as f32);
		let pos_px =
			vec2(item.cursor_pos.x, item.cursor_pos.y) - screen / 2.0 + size / 2.0 - origin;
		let pos = pos_px * vec2(1.0, -1.0) / self.density;
		Some(
			SurfaceModel::new(
				&item.shell,
				SurfaceUpdateTarget::Cursor,
				panel_resource(),
				"Panel",
			)
			.pos(pos.extend(0.001))
			.scl((size / self.density).extend(PANEL_THICKNESS))
			.build(),
		)
	}
}
