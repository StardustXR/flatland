use crate::{
	Flatland, Panel,
	layout::Placed,
	surface_input::{input_planes, keyboard_handler, mouse_handler},
	touch_input::TouchHover,
};
use glam::{Vec2, vec2};
use rustc_hash::FxHashMap;
use stardust_xr_asteroids::{
	ClientState as _, CustomElement as _, Element, Entity, Transformable as _,
	elements::{Lines, Model, Spatial},
};
use stardust_xr_fusion::{
	fields::Shape,
	types::{Color, Resource, Size2},
};
use stardust_xr_molecules::DebugSettings;
use stardust_xr_panels::panel_item::SurfaceId;
use stardust_xr_panels_asteroids::surface_model::SurfaceModel;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layer {
	/// the app's pixels, with the touch and pointer planes unless they're split off
	Screen { planes: bool },
	/// only the touch and pointer planes, outlined since there's nothing else to see
	SplitInput,
}

pub fn panel_resource() -> Resource {
	Resource::Namespaced {
		namespace: Flatland::APP_ID.into(),
		path: "panel".into(),
	}
}

pub struct Surfaces<'a> {
	pub panel: Option<&'a Panel>,
	pub layer: Layer,
	pub touch_hovers: &'a FxHashMap<SurfaceId, Vec<TouchHover>>,
	pub accent: Color,
	pub density: f32,
	pub scroll_multiplier: f32,
}
impl Surfaces<'_> {
	pub fn build(&self, size: Size2, placed: &[Placed]) -> impl Element<Flatland> + use<> {
		let toplevel = vec2(size.x as f32, size.y as f32);
		Spatial::default().build().stable_children(
			placed
				.iter()
				.map(|s| (s.id, self.surface(toplevel, s)))
				.collect::<FxHashMap<_, _>>(),
		)
	}

	fn surface(&self, toplevel: Vec2, s: &Placed) -> impl Element<Flatland> + use<> {
		let id = s.id;
		let has_input = !s.input.is_empty();
		let size = (s.max - s.min) / self.density;
		// px are +Y down from the top left corner, spatials +Y up from the center
		let pos = (s.min + s.max - toplevel) / 2.0 * vec2(1.0, -1.0) / self.density;
		let (visuals, planes) = match self.layer {
			Layer::Screen { planes } => (true, planes),
			Layer::SplitInput => (false, true),
		};
		let debug = (self.layer == Layer::SplitInput).then(DebugSettings::default);
		let t = s.thickness;
		let scl = size.extend(t);

		Entity::new(Shape::Box { size: scl.into() })
			.pos(pos.extend(s.front - t / 2.0))
			.component(has_input.then(|| keyboard_handler(self.panel.map(|i| &i.shell), id)))
			.component(
				self.panel
					.filter(|_| has_input)
					.map(|p| mouse_handler(&p.item, p.pointer.clone(), self.scroll_multiplier)),
			)
			.build()
			.maybe_child(
				self.panel
					.filter(|_| visuals)
					.map(|i| &i.shell)
					.map(|shell| {
						SurfaceModel::new(shell, id, panel_resource(), "Panel")
							.scl(scl)
							.build()
					}),
			)
			.maybe_child((visuals && self.panel.is_none()).then(|| {
				Model::namespaced(Flatland::APP_ID, "panel")
					.scl(scl)
					.build()
			}))
			.maybe_child((has_input && planes).then(|| input_planes(id, size, t, debug)))
			.maybe_child(
				self.touch_hovers
					.get(&id)
					.filter(|_| !planes)
					.map(|hovers| {
						Lines::new(hovers.iter().map(|h| h.ring(t, self.accent))).build()
					}),
			)
	}
}
