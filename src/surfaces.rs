use crate::{
	Flatland, Shell,
	surface_input::{input_planes, keyboard_handler, mouse_handler},
	surface_tree::ChildState,
	touch_input::TouchHover,
};
use glam::{Vec2, Vec3, vec2};
use rustc_hash::FxHashMap;
use stardust_xr_asteroids::{
	ClientState as _, CustomElement as _, Element, Entity, Transformable as _,
	elements::{Lines, Model},
};
use stardust_xr_fusion::{
	fields::Shape,
	types::{Color, Resource, Size2},
};
use stardust_xr_molecules::DebugSettings;
use stardust_xr_panels::panel_item::{Geometry, SurfaceId};
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
	pub shell: Option<&'a Shell>,
	pub layer: Layer,
	pub touch_hovers: &'a FxHashMap<SurfaceId, Vec<TouchHover>>,
	pub accent: Color,
	pub thickness: f32,
	pub density: f32,
	pub scroll_multiplier: f32,
}
impl Surfaces<'_> {
	pub fn toplevel(&self, size: Size2, children: &[ChildState]) -> impl Element<Flatland> + use<> {
		let geometry = Geometry {
			origin: [0; 2].into(),
			size,
		};
		self.surface(SurfaceId::Toplevel, size, geometry, true, 0, children)
	}

	fn surface(
		&self,
		id: SurfaceId,
		parent_size: Size2,
		geometry: Geometry,
		has_input: bool,
		z_offset: i32,
		children: &[ChildState],
	) -> impl Element<Flatland> + use<> {
		let (pos, size) = self.layout(parent_size, geometry, z_offset);
		let (visuals, planes) = match self.layer {
			Layer::Screen { planes } => (true, planes),
			Layer::SplitInput => (false, true),
		};
		let debug = (self.layer == Layer::SplitInput).then(DebugSettings::default);
		let scl = size.extend(self.thickness);

		Entity::new(Shape::Box { size: scl.into() })
			.pos(pos)
			.component(has_input.then(|| keyboard_handler(self.shell, id)))
			.component(has_input.then(|| mouse_handler(self.shell, id, self.scroll_multiplier)))
			.build()
			.maybe_child(self.shell.filter(|_| visuals).map(|shell| {
				SurfaceModel::new(shell, id, panel_resource(), "Panel")
					.scl(scl)
					.build()
			}))
			.maybe_child((visuals && self.shell.is_none()).then(|| {
				Model::namespaced(Flatland::APP_ID, "panel")
					.scl(scl)
					.build()
			}))
			.maybe_child(
				(has_input && planes).then(|| input_planes(id, size, self.thickness, debug)),
			)
			.maybe_child(
				self.touch_hovers
					.get(&id)
					.filter(|_| !planes)
					.map(|hovers| {
						Lines::new(hovers.iter().map(|h| h.ring(self.thickness, self.accent)))
							.build()
					}),
			)
			.stable_children(
				children
					.iter()
					.map(|child| {
						let element = self.surface(
							SurfaceId::Child { id: child.info.id },
							geometry.size,
							child.info.geometry,
							!child.info.input_regions.is_empty(),
							1,
							&child.children,
						);
						(child.info.id, element)
					})
					.collect::<FxHashMap<_, _>>(),
			)
			.dynamic()
	}

	fn layout(&self, parent_size: Size2, geometry: Geometry, z_offset: i32) -> (Vec3, Vec2) {
		let parent = vec2(parent_size.x as f32, parent_size.y as f32);
		let origin = vec2(geometry.origin.x as f32, geometry.origin.y as f32);
		let size = vec2(geometry.size.x as f32, geometry.size.y as f32);
		// surfaces are placed by their top left corner in +Y down pixels, spatials by their center
		let pos = (origin - parent / 2.0 + size / 2.0) * vec2(1.0, -1.0) / self.density;
		(
			pos.extend(self.thickness * z_offset as f32),
			size / self.density,
		)
	}
}
