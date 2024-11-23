use asteroids::custom::ElementTrait;
use derive_setters::Setters;
use glam::{vec2, vec3, Vec2};
use input_event_codes::BTN_LEFT;
use lazy_static::lazy_static;
use stardust_xr_fusion::{
	drawable::Model,
	items::panel::{Geometry, PanelItem, PanelItemAspect, SurfaceId},
	node::NodeError,
	spatial::{Spatial, SpatialAspect, Transform},
	values::{
		color::{color_space::LinearRgb, rgba_linear, AlphaColor, Rgb},
		ResourceID, Vector2,
	},
};
use tracing::{info, warn};

use crate::{
	surface_input::{HoverPlaneElement, TouchPlaneElement},
	ToplevelState,
};

lazy_static! {
	pub static ref PANEL_RESOURCE: ResourceID = ResourceID::new_namespaced("flatland", "panel");
}

#[derive(Setters, Debug, Clone, PartialEq)]
#[setters(into, strip_option)]
pub struct SurfaceElement {
	pub initial_resolution: Vector2<u32>,
	pub receives_input: bool,
	pub item: PanelItem,
	pub id: SurfaceId,
	/// Pixels per meter
	pub density: f32,
	pub thickness: f32,
	pub child_thickness: f32,
	pub children: Vec<ChildSurfaceData>,
	pub highlight_color: Rgb<f32, LinearRgb>,
	pub parent_thickness: Option<f32>,
}

#[derive(Setters, Debug, Clone, PartialEq)]
pub struct ChildSurfaceData {
	pub id: SurfaceId,
	pub geometry: Geometry,
}

pub struct Surface {
	root: Spatial,
	item: PanelItem,
	model: Model,
}

impl SurfaceElement {
	pub fn new_child_element(&self, child_data: &ChildSurfaceData) -> SurfaceElement {
		SurfaceElement {
			initial_resolution: child_data.geometry.size,
			receives_input: self.receives_input,
			item: self.item.clone(),
			id: child_data.id.clone(),
			density: self.density,
			thickness: self.child_thickness,
			child_thickness: self.child_thickness,
			highlight_color: self.highlight_color,
			parent_thickness: Some(self.thickness),
			children: vec![],
		}
	}
}

impl ElementTrait<ToplevelState> for SurfaceElement {
	type Inner = Surface;

	type Error = NodeError;

	fn create_inner(
		&self,
		parent_space: &stardust_xr_fusion::spatial::SpatialRef,
	) -> Result<Self::Inner, Self::Error> {
		self.item.set_spatial_parent_in_place(parent_space);
		let physical_size: Vec2 = vec2(
			self.initial_resolution.x as f32,
			self.initial_resolution.y as f32,
		) / self.density;
		let transform = match self.parent_thickness {
			Some(v) => Transform::from_translation([0.0, 0.0, -v]),
			None => Transform::none(),
		};
		let root = Spatial::create(&self.item, transform, false)?;
		let panel_size = vec3(physical_size.x, physical_size.y, self.thickness);
		let model = Model::create(
			&root,
			Transform::from_translation_scale(panel_size * vec3(0.0, 0.0, -0.5), panel_size),
			&PANEL_RESOURCE,
		)?;
		self.item
			.apply_surface_material(self.id.clone(), &model.part("Panel")?)?;
		let surface = Surface {
			root,
			item: self.item.clone(),
			model,
		};
		Ok(surface)
	}

	fn update(&self, old_decl: &Self, _state: &mut ToplevelState, inner: &mut Self::Inner) {
		if self.initial_resolution != old_decl.initial_resolution {
			let physical_size: Vec2 = vec2(
				self.initial_resolution.x as f32,
				self.initial_resolution.y as f32,
			) / self.density;
			let panel_size = vec3(physical_size.x, physical_size.y, self.thickness);
			if let Err(err) = inner
				.model
				.set_local_transform(Transform::from_translation_scale(
					panel_size * vec3(0.0, 0.0, -0.5),
					panel_size,
				)) {
				warn!("error while applying new scale to surface model: {err}");
			}
			_ = inner.item.set_toplevel_size(self.initial_resolution);
		}
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> stardust_xr_fusion::spatial::SpatialRef {
		inner.root.clone().as_spatial_ref()
	}

	fn frame(&self, _info: &stardust_xr_fusion::root::FrameInfo, _inner: &mut Self::Inner) {}

	fn build(self) -> asteroids::Element<ToplevelState> {
		let mut iter = self
			.children
			.iter()
			.map(|v| self.new_child_element(v))
			.map(|v| v.build())
			.collect::<Vec<_>>();
		iter.push(
			HoverPlaneElement {
				density: self.density,
				thickness: self.thickness,
				resolution: self.initial_resolution,
				distance_range: 0.05..0.2,
				line_start_thickness: 0.001,
				line_start_color_hover: rgba_linear!(1.0, 1.0, 1.0, 0.1),
				line_start_color_interact: AlphaColor {
					c: self.highlight_color,
					a: 0.1,
				},
				line_end_thickness: 0.005,
				line_end_color_hover: rgba_linear!(1.0, 1.0, 1.0, 1.0),
				line_end_color_interact: AlphaColor {
					c: self.highlight_color,
					a: 1.0,
				},
				on_hover: Some({
					let surface_id = self.id.clone();
					move |state: &mut ToplevelState, point| {
						_ = state.panel_item.pointer_motion(surface_id.clone(), point);
					}
				}),
				on_interact: Some({
					let surface_id = self.id.clone();
					move |state: &mut ToplevelState, point, _distance| {
						let id = surface_id.clone();
						_ = state.panel_item.pointer_motion(id.clone(), point);
						_ = state
							.panel_item
							.pointer_button(id.clone(), BTN_LEFT!(), true);
						_ = state
							.panel_item
							.pointer_button(id.clone(), BTN_LEFT!(), false);
					}
				}),
				debug_color: None,
				_state: Default::default(),
			}
			.build(),
		);
		iter.push(
			TouchPlaneElement {
				density: self.density,
				thickness: self.thickness,
				resolution: self.initial_resolution,
				on_added: Some({
					let surface_id = self.id.clone();
					move |state: &mut ToplevelState, touch_id, point, depth| {
						let id = surface_id.clone();
						_ = state.panel_item.touch_down(id, touch_id, point)
					}
				}),
				on_move: Some({
					move |state: &mut ToplevelState, touch_id, point, depth| {
						_ = state.panel_item.touch_move(touch_id, point)
					}
				}),
				on_removed: Some({
					move |state: &mut ToplevelState, touch_id, point, depth| {
						_ = state.panel_item.touch_up(touch_id)
					}
				}),
				debug_color: None,
				_state: Default::default(),
			}
			.build(),
		);
		self.with_children(iter)
	}
}
