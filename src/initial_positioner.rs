use asteroids::{Context, ElementTrait, ValidState};
use stardust_xr_fusion::{
	node::NodeError,
	spatial::{Spatial, SpatialAspect, SpatialRef, Transform},
};

#[derive(Debug, PartialEq)]
pub struct InitialPositioner(pub SpatialRef);
impl<State: ValidState> ElementTrait<State> for InitialPositioner {
	type Inner = Spatial;
	type Resource = ();
	type Error = NodeError;

	fn create_inner(
		&self,
		parent_space: &SpatialRef,
		_context: &Context,
		_resource: &mut Self::Resource,
	) -> Result<Self::Inner, Self::Error> {
		let spatial = Spatial::create(parent_space, Transform::identity(), false)?;
		spatial.set_relative_transform(&self.0, Transform::identity())?;
		Ok(spatial)
	}

	fn update(
		&self,
		_old_decl: &Self,
		_state: &mut State,
		_inner: &mut Self::Inner,
		_resource: &mut Self::Resource,
	) {
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.clone().as_spatial_ref()
	}
}
