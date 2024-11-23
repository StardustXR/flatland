use std::{marker::PhantomData, ops::Range};

use asteroids::{custom::ElementTrait, ValidState};
use stardust_xr_fusion::{
	node::NodeError,
	spatial::Transform,
	values::{
		color::{color_space::LinearRgb, rgba_linear, Rgba},
		Vector2,
	},
};
use stardust_xr_molecules::{
	hover_plane::{HoverPlane, HoverPlaneSettings},
	VisualDebug,
};

#[derive_where::derive_where(Debug, Clone, PartialEq)]
pub struct HoverPlaneElement<State, F, F2>
where
	State: ValidState,
	F: Sized + Send + Sync + 'static + Fn(&mut State, Vector2<f32>) + Clone,
	F2: Sized + Send + Sync + 'static + Fn(&mut State, Vector2<f32>, f32) + Clone,
{
	pub density: f32,
	pub thickness: f32,
	pub resolution: Vector2<u32>,
	pub distance_range: Range<f32>,
	pub line_start_thickness: f32,
	pub line_start_color_hover: Rgba<f32, LinearRgb>,
	pub line_start_color_interact: Rgba<f32, LinearRgb>,
	pub line_end_thickness: f32,
	pub line_end_color_hover: Rgba<f32, LinearRgb>,
	pub line_end_color_interact: Rgba<f32, LinearRgb>,
	#[derive_where(skip(Debug, EqHashOrd))]
	pub on_hover: Option<F>,
	#[derive_where(skip(Debug, EqHashOrd))]
	pub on_interact: Option<F2>,
	// Why rust? State is clearly used by F and F2?!
	pub _state: PhantomData<State>,
}

impl<State, F, F2> ElementTrait<State> for HoverPlaneElement<State, F, F2>
where
	State: ValidState,
	F: Sized + Send + Sync + 'static + Fn(&mut State, Vector2<f32>) + Clone,
	F2: Sized + Send + Sync + 'static + Fn(&mut State, Vector2<f32>, f32) + Clone,
{
	type Inner = HoverPlane;

	type Error = NodeError;

	fn create_inner(
		&self,
		parent_space: &stardust_xr_fusion::spatial::SpatialRef,
	) -> Result<Self::Inner, Self::Error> {
		HoverPlane::create(
			parent_space,
			Transform::none(),
			[
				(self.resolution.x as f32 / self.density),
				(self.resolution.y as f32 / self.density),
			],
			self.thickness,
			0.0..self.resolution.x as f32,
			0.0..self.resolution.y as f32,
			HoverPlaneSettings {
				distance_range: self.distance_range.clone(),
				line_start_thickness: self.line_start_thickness,
				line_start_color_hover: self.line_start_color_hover,
				line_start_color_interact: self.line_start_color_interact,
				line_end_thickness: self.line_end_thickness,
				line_end_color_hover: self.line_end_color_hover,
				line_end_color_interact: self.line_end_color_interact,
			},
		)
	}

	fn update(&self, _old_decl: &Self, state: &mut State, inner: &mut Self::Inner) {
		_ = inner.set_size([
			(self.resolution.x as f32 / self.density),
			(self.resolution.y as f32 / self.density),
		]);
		inner.x_range = 0.0..self.resolution.x as f32;
		inner.y_range = 0.0..self.resolution.y as f32;
		inner.update();
		if let Some(actor) = inner.interact_status().actor() {
			if inner.interact_status().actor_started() {
				let (point, distance) = inner.interact_point(actor);

				if let Some(on_interact) = self.on_interact.as_ref() {
					on_interact(state, point, distance);
				}
			}
		}
		inner.set_debug(Some(stardust_xr_molecules::DebugSettings {
			line_thickness: 0.001,
			line_color: rgba_linear!(1.0, 0.0, 1.0, 1.0),
		}));
		for point in inner.current_hover_points() {
			if let Some(on_hover) = self.on_hover.as_ref() {
				on_hover(state, point)
			}
		}
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> stardust_xr_fusion::spatial::SpatialRef {
		inner.root().clone().as_spatial_ref()
	}
}
