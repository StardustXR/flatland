use crate::layout::Layout;
use glam::Vec2;
use stardust_xr_fusion::types::Timestamp;
use stardust_xr_panels::panel_item::{PanelItem, SurfaceId};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pointer2DDepth {
	/// relative input, eases up and down between surfaces
	Blend,
	/// absolute input sits on the plane it was aimed at, blending would slide it off the ray
	On(SurfaceId),
}

#[derive(Debug, Clone)]
pub struct Pointer2D {
	/// toplevel px
	pub pos: Vec2,
	/// stays put while the cursor crosses gaps between surfaces
	pub target: SurfaceId,

	pub layout: Arc<Layout>,
	/// px past the hull
	margin: f32,
}
impl Pointer2D {
	pub fn new(layout: Arc<Layout>, margin: f32) -> Self {
		Pointer2D {
			pos: Vec2::ZERO,
			target: SurfaceId::Toplevel,
			layout,
			margin,
		}
	}
	pub fn set_layout(&mut self, layout: Arc<Layout>) {
		self.pos = layout.clamp(self.pos, self.margin);
		if layout.get(self.target).is_none() {
			self.target = SurfaceId::Toplevel;
		}
		self.layout = layout;
	}
	pub fn origin(&self, id: SurfaceId) -> Option<Vec2> {
		self.layout.get(id).map(|s| s.min)
	}
	pub fn move_to(&mut self, p: Vec2) {
		self.pos = self.layout.clamp(p, self.margin);
		if let Some(id) = self.layout.target(self.pos) {
			self.target = id;
		}
	}
	pub fn move_by(&mut self, item: &PanelItem, d: Vec2, timestamp: Option<Timestamp>) {
		self.move_to(self.pos + d);
		self.send_motion(item, Some(d), timestamp);
	}
	pub fn send_motion(&self, item: &PanelItem, delta: Option<Vec2>, timestamp: Option<Timestamp>) {
		let Some(local) = self.origin(self.target).map(|o| self.pos - o) else {
			return;
		};
		let _ = item.pointer_motion(
			self.target,
			delta.map(|d| [d.x, d.y].into()),
			[local.x, local.y].into(),
			timestamp,
		);
	}
}
