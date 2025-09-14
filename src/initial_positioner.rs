use asteroids::{Context, CreateInnerInfo, CustomElement, ValidState};
use stardust_xr_fusion::{
	node::NodeError,
	spatial::{Spatial, SpatialAspect, SpatialRef, Transform},
};

#[derive(Debug, PartialEq)]
pub struct InitialPositioner(pub SpatialRef);
impl<State: ValidState> CustomElement<State> for InitialPositioner {
	type Inner = Spatial;
	type Resource = ();
	type Error = NodeError;

	fn create_inner(
		&self,
		_context: &Context,
		info: CreateInnerInfo,
		_resource: &mut Self::Resource,
	) -> Result<Self::Inner, Self::Error> {
		let spatial = Spatial::create(info.parent_space, Transform::identity(), false)?;
		spatial.set_relative_transform(&self.0, Transform::identity())?;
		Ok(spatial)
	}

	fn diff(&self, _old_self: &Self, _inner: &mut Self::Inner, _resource: &mut Self::Resource) {}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.clone().as_spatial_ref()
	}
}
