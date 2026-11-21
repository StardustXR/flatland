use crate::layout::Layout;
use stardust_xr_fusion::types::Size2;
use stardust_xr_panels::panel_item::{ChildState as ChildInfo, Geometry, SurfaceId};
use std::sync::Arc;

#[derive(Debug)]
pub struct ChildState {
	pub info: ChildInfo,
	pub children: Vec<ChildState>,
}

/// the tree as the app sent it, re-placed on every change so the layout is never stale
#[derive(Debug)]
pub struct SurfaceTree {
	size: Size2,
	children: Vec<ChildState>,
	layout: Arc<Layout>,
}
impl SurfaceTree {
	pub fn new(size: Size2) -> Self {
		SurfaceTree {
			size,
			children: Vec::new(),
			layout: Arc::new(Layout::new(size, &[])),
		}
	}
	pub fn layout(&self) -> &Arc<Layout> {
		&self.layout
	}
	pub fn set_size(&mut self, size: Size2) {
		self.size = size;
		self.layout = Arc::new(Layout::new(self.size, &self.children));
	}
	pub fn add(&mut self, info: ChildInfo) {
		add_child(&mut self.children, info);
		self.layout = Arc::new(Layout::new(self.size, &self.children));
	}
	pub fn move_child(&mut self, id: u64, geometry: Geometry) {
		update_child_geometry(&mut self.children, id, geometry);
		self.layout = Arc::new(Layout::new(self.size, &self.children));
	}
	pub fn remove(&mut self, id: u64) {
		remove_child(&mut self.children, id);
		self.layout = Arc::new(Layout::new(self.size, &self.children));
	}
}

fn add_child(children: &mut Vec<ChildState>, info: ChildInfo) {
	let siblings = match info.parent {
		SurfaceId::Toplevel => children,
		SurfaceId::Child { id } => match find_mut(children, id) {
			Some(parent) => &mut parent.children,
			None => {
				tracing::warn!(
					child_id = info.id,
					parent_id = id,
					"add_child: parent surface not found, dropping child"
				);
				return;
			}
		},
	};
	siblings.push(ChildState {
		info,
		children: Vec::new(),
	});
}
fn update_child_geometry(children: &mut [ChildState], id: u64, geometry: Geometry) {
	match find_mut(children, id) {
		Some(child) => child.info.geometry = geometry,
		None => tracing::warn!(id, "update_child_geometry: surface not found"),
	}
}
fn remove_child(children: &mut Vec<ChildState>, id: u64) {
	if !remove_child_inner(children, id) {
		tracing::warn!(id, "remove_child: surface not found");
	}
}

fn find_mut(children: &mut [ChildState], id: u64) -> Option<&mut ChildState> {
	for child in children {
		if child.info.id == id {
			return Some(child);
		}
		if let Some(found) = find_mut(&mut child.children, id) {
			return Some(found);
		}
	}
	None
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
