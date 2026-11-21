use close_button::ExposureButton;
use glam::{Quat, Vec2, Vec3, vec2, vec3};
use gluon_ipc::Node;
use initial_panel_placement::InitialPanelPlacement;
use pointer_2d::{Pointer2D, Pointer2DDepth};
use resize_handles::ResizeHandles;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use split_input::InputSurface;
use stardust_xr_asteroids::{
	CloneFnWrapper, Context, CustomElement, Element, Entity, FnWrapper, Migrate, Reify, Tasker,
	Transformable as _,
	client::{ClientState, run},
	components::Derezzable,
	elements::{Handle, Spatial, Text},
	spring,
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
use surface_tree::SurfaceTree;
use surfaces::{Layer, Surfaces, panel_resource};
use tokio::sync::watch;
use touch_input::TouchHover;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _};

pub mod close_button;
pub mod grab_ball;
pub mod initial_panel_placement;
pub mod layout;
pub mod pointer_2d;
// pub mod panel_shell_transfer;
pub mod pointer_input;
pub mod resize_handles;
pub mod split_input;
pub mod surface_input;
pub mod surface_tree;
pub mod surfaces;
pub mod touch_input;

const TOPLEVEL_THICKNESS: f32 = 0.01;
const CHILD_THICKNESS: f32 = 0.005;
const INPUTLESS_CHILD_THICKNESS: f32 = 0.005;
const CURSOR_THICKNESS: f32 = 0.001;
/// how far past the surfaces' hull the cursor can go, in meters
const CURSOR_HULL_MARGIN: f32 = 0.002;
/// how far from a surface the cursor starts rising onto it, in meters
const CURSOR_DEPTH_MARGIN: f32 = 0.004;
/// period of the spring the cursor glides to a new depth on, in seconds
const CURSOR_DEPTH_SPRING: f32 = 0.1;

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
pub struct Panel {
	shell: Shell,
	item: stardust_xr_panels::panel_item::PanelItem,
	release_pos_offset: Vec3,

	title: Option<String>,
	app_id: Option<String>,
	min_size: Option<Size2>,
	max_size: Option<Size2>,

	/// shared with the async mouse path so input reaches the app without waiting on a frame
	pointer: watch::Sender<Pointer2D>,
	cursor: Option<Geometry>,
	cursor_depth: Pointer2DDepth,

	tree: SurfaceTree,
}
impl Panel {
	fn new(shell: Shell, size: Size2, margin: f32) -> Self {
		let tree = SurfaceTree::new(size);
		Panel {
			pointer: watch::Sender::new(Pointer2D::new(tree.layout().clone(), margin)),
			tree,
			item: shell.item().clone(),
			shell,
			release_pos_offset: Vec3::ZERO,
			title: None,
			app_id: None,
			min_size: None,
			max_size: None,
			cursor: None,
			cursor_depth: Pointer2DDepth::Blend,
		}
	}
	/// the only way to change the tree, so the pointer always sees its layout
	fn edit_tree(&mut self, f: impl FnOnce(&mut SurfaceTree)) {
		f(&mut self.tree);
		let layout = self.tree.layout().clone();
		self.pointer.send_modify(|c| c.set_layout(layout));
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
	panel: Option<Panel>,

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

			panel: None,

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
		let size_px = self.size_px();
		if let Some(item) = &mut self.panel {
			item.edit_tree(|t| t.set_size(size_px));
		}
	}
	fn resize(&mut self, new_size: impl Into<Vec2F>) {
		let old_size_px = self.size_px();
		self.screen_size = new_size.into();
		let Some(item) = self.panel.as_ref() else {
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

	/// pos is local to the surface
	fn set_pointer(
		&mut self,
		surface_id: SurfaceId,
		pos: impl Into<Vec2F>,
		timestamp: Option<Timestamp>,
	) {
		let Some(item) = self.panel.as_mut() else {
			return;
		};
		item.cursor_depth = Pointer2DDepth::On(surface_id);
		let pos = pos.into();
		item.pointer.send_modify(|c| {
			let origin = c.origin(surface_id).unwrap_or(Vec2::ZERO);
			c.move_to(origin + vec2(pos.x, pos.y));
			c.send_motion(&item.item, None, timestamp);
		});
	}

	fn title(&self) -> String {
		let Some(item) = &self.panel else {
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
		let item = self.panel.as_ref();
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
			size: [self.screen_size.x, self.screen_size.y, TOPLEVEL_THICKNESS].into(),
		})
		// no Poseable here, ResizeHandles already serves one for this plane in the right space
		.component(Derezzable::<Self>::new(move |state| {
			if let Some(item) = &state.panel {
				state.derez_with_item_temp = true;
				_ = item.item.close_toplevel();
			} else {
				context.stop();
			}
		}))
		.component(PanelItemAcceptor::<Self>::new(
			|state, shell, spawned_for_item| {
				_ = shell.item().request_toplevel_resize(state.size_px());
				state.derez_with_item_temp |= spawned_for_item;
				let margin = CURSOR_HULL_MARGIN * state.density;
				state.panel = Some(Panel::new(shell, state.size_px(), margin));
			},
		))
		.build()
	}

	fn eject_handle(&self) -> Option<impl Element<Self> + use<>> {
		self.panel.is_some().then(|| {
			Handle::new(
				[0.0, -self.screen_size.y / 2.0, 0.0],
				|state: &mut Self, pos| {
					if let Some(item) = &mut state.panel {
						item.release_pos_offset = pos.into();
					}
				},
			)
			.on_release(|state: &mut Self, _| state.panel = None)
			.head_offset([0.0, -0.02, 0.0])
			.build()
		})
	}

	fn panel_shell(&self, context: &Context) -> Option<impl Element<Self> + use<>> {
		let item = self.panel.as_ref()?;
		let context = context.clone();
		Some(
			PanelShell::new(&item.shell, move |state: &mut Self| {
				state.panel = None;
				if state.derez_with_item || state.derez_with_item_temp {
					context.stop();
				}
			})
			.on_toplevel_resolution_changed(|state: &mut Self, _, size| state.set_size_px(size))
			.on_toplevel_max_size_changed(|state: &mut Self, _, size| {
				if let Some(item) = &mut state.panel {
					item.max_size = size;
				}
				state.resize(state.screen_size);
			})
			.on_toplevel_min_size_changed(|state: &mut Self, _, size| {
				if let Some(item) = &mut state.panel {
					item.min_size = size;
				}
				state.resize(state.screen_size);
			})
			.on_toplevel_app_id_changed(|state: &mut Self, _, app_id| {
				if let Some(item) = &mut state.panel {
					item.app_id = Some(app_id);
				}
			})
			.on_toplevel_title_changed(|state: &mut Self, _, title| {
				if let Some(item) = &mut state.panel {
					item.title = Some(title);
				}
			})
			.cursor_visuals_changed(|state: &mut Self, _, geometry| {
				if let Some(item) = &mut state.panel {
					item.cursor = geometry;
				}
			})
			.new_child(|state: &mut Self, _, info| {
				if let Some(item) = &mut state.panel {
					item.edit_tree(|t| t.add(info));
				}
			})
			.child_moved(|state: &mut Self, _, id, geometry| {
				if let Some(item) = &mut state.panel {
					item.edit_tree(|t| t.move_child(id, geometry));
				}
			})
			.child_removed(|state: &mut Self, _, id| {
				if let Some(item) = &mut state.panel {
					item.edit_tree(|t| t.remove(id));
				}
			})
			.pos(vec3(0.0, 0.0, TOPLEVEL_THICKNESS) + item.release_pos_offset)
			.build(),
		)
	}

	fn close_button(&self, context: &Context) -> impl Element<Self> + use<> {
		let context = context.clone();
		ExposureButton::<Self> {
			transform: Transform::from_translation([
				self.screen_size.x / 2.0,
				self.screen_size.y / -2.0,
				TOPLEVEL_THICKNESS / 2.0,
			]),
			thickness: TOPLEVEL_THICKNESS,
			gain: 2.0,
			on_click: FnWrapper(Box::new(move |state: &mut Self| {
				if let Some(item) = &state.panel {
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
			.character_height(TOPLEVEL_THICKNESS * 0.75)
			.align_x(XAlign::Left)
			.align_y(YAlign::Center)
			.bounds(TextBounds {
				bounds: [self.screen_size.y, TOPLEVEL_THICKNESS].into(),
				fit: TextFit::Squeeze,
				anchor_align_x: XAlign::Left,
				anchor_align_y: YAlign::Bottom,
			})
			.pos([
				self.screen_size.x / 2.0 + 0.0005,
				self.screen_size.y / 2.0 - 0.001,
				TOPLEVEL_THICKNESS / 2.0,
			])
			.rot(Quat::from_rotation_z(-FRAC_PI_2) * Quat::from_rotation_x(-FRAC_PI_2))
			.build()
	}

	fn surfaces(&self, context: &Context, layer: Layer) -> impl Element<Self> + use<> {
		let layout = match &self.panel {
			Some(panel) => panel.tree.layout().clone(),
			None => SurfaceTree::new(self.size_px()).layout().clone(),
		};
		Surfaces {
			panel: self.panel.as_ref(),
			layer,
			touch_hovers: &self.touch_hovers,
			accent: context.accent_color.color(),
			density: self.density,
			scroll_multiplier: self.mouse_scroll_multiplier,
		}
		.build(self.size_px(), &layout.placed)
	}

	fn cursor_depth(&self) -> Option<f32> {
		let item = self.panel.as_ref()?;
		let geometry = item.cursor?;
		let pointer = item.pointer.borrow();
		if let Pointer2DDepth::On(id) = item.cursor_depth
			&& let Some(s) = pointer.layout.get(id)
		{
			return Some(s.front);
		}
		let min = pointer.pos - vec2(geometry.origin.x as f32, geometry.origin.y as f32);
		let size = vec2(geometry.size.x as f32, geometry.size.y as f32);
		let margin = CURSOR_DEPTH_MARGIN * self.density;
		Some(pointer.layout.depth(min, min + size, margin))
	}
	fn cursor(&self) -> Option<impl Element<Self> + use<>> {
		let item = self.panel.as_ref()?;
		let geometry = item.cursor?;
		let screen = vec2(self.size_px().x as f32, self.size_px().y as f32);
		let origin = vec2(geometry.origin.x as f32, geometry.origin.y as f32);
		let size = vec2(geometry.size.x as f32, geometry.size.y as f32);
		let min = item.pointer.borrow().pos - origin;
		let pos = (min - screen / 2.0 + size / 2.0) * vec2(1.0, -1.0) / self.density;
		let z = self.cursor_depth()?;
		// a spring since the target moves every frame the mouse does, an eased curve would
		// restart each time and never arrive
		let blend = item.cursor_depth == Pointer2DDepth::Blend;
		Some(
			Spatial::default()
				.pos(vec3(0.0, 0.0, z))
				.build()
				.animation(|a| {
					if blend {
						a._0(spring(CURSOR_DEPTH_SPRING, 0.0))
					} else {
						a
					}
				})
				.child(
					SurfaceModel::new(
						&item.shell,
						SurfaceUpdateTarget::Cursor,
						panel_resource(),
						"Panel",
					)
					.pos(pos.extend(CURSOR_THICKNESS / 2.0))
					.scl((size / self.density).extend(CURSOR_THICKNESS))
					.build(),
				),
		)
	}
}
