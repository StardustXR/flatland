use stardust_xr_panels::panel_item::{ChildState as ChildInfo, Geometry, SurfaceId};

#[derive(Debug)]
pub struct ChildState {
	pub info: ChildInfo,
	pub children: Vec<ChildState>,
}

pub fn add_child(children: &mut Vec<ChildState>, info: ChildInfo) {
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
pub fn update_child_geometry(children: &mut [ChildState], id: u64, geometry: Geometry) {
	match find_mut(children, id) {
		Some(child) => child.info.geometry = geometry,
		None => tracing::warn!(id, "update_child_geometry: surface not found"),
	}
}
pub fn remove_child(children: &mut Vec<ChildState>, id: u64) {
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
