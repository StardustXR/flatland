use crate::{
	CHILD_THICKNESS, INPUTLESS_CHILD_THICKNESS, TOPLEVEL_THICKNESS, surface_tree::ChildState,
};
use glam::{Vec2, vec2};
use stardust_xr_fusion::types::Size2;
use stardust_xr_panels::panel_item::SurfaceId;

/// nudges stacked siblings apart so they don't z-fight
const Z_STEP: f32 = 0.0005;

/// a surface flattened out of the tree, in toplevel px (+Y down) and meters of depth
#[derive(Debug, Clone)]
pub struct Placed {
	pub id: SurfaceId,
	pub min: Vec2,
	pub max: Vec2,
	pub input: Vec<(Vec2, Vec2)>,

	pub thickness: f32,
	pub front: f32,
	/// where the cursor rises from as it nears this surface
	pub under: f32,
}

#[derive(Debug)]
pub struct Layout {
	pub placed: Vec<Placed>,
	/// the area the cursor can roam, spanning every surface with input including the gaps between them
	pub hull: Vec<Vec2>,
}
impl Layout {
	pub fn new(size: Size2, children: &[ChildState]) -> Self {
		let placed = place(size, children);
		Layout {
			hull: hull(&placed),
			placed,
		}
	}
	pub fn get(&self, id: SurfaceId) -> Option<&Placed> {
		self.placed.iter().find(|s| s.id == id)
	}
	pub fn clamp(&self, p: Vec2, margin: f32) -> Vec2 {
		let hull = &self.hull;
		if hull.len() < 3 {
			return hull.first().copied().unwrap_or(p);
		}
		let edges = || hull.iter().zip(hull.iter().cycle().skip(1));
		if edges().all(|(&a, &b)| cross(a, b, p) >= 0.0) {
			return p;
		}
		let q = edges()
			.map(|(&a, &b)| {
				let ab = b - a;
				a + ab * ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
			})
			.min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
			.unwrap();
		q + (p - q).clamp_length_max(margin)
	}
	/// topmost surface whose input region the hotspot is in
	pub fn target(&self, p: Vec2) -> Option<SurfaceId> {
		self.placed
			.iter()
			.filter(|s| {
				s.input
					.iter()
					.any(|&(min, max)| p.cmpge(min).all() && p.cmple(max).all())
			})
			.max_by(|a, b| a.front.total_cmp(&b.front))
			.map(|s| s.id)
	}
	/// the cursor rises onto anything within `margin` px of its rect before it gets there,
	/// so it never sinks into a surface it's crossing onto
	pub fn depth(&self, min: Vec2, max: Vec2, margin: f32) -> f32 {
		self.placed
			.iter()
			.map(|s| {
				let gap = (s.min - max).max(min - s.max).max(Vec2::ZERO).length();
				let w = if gap <= 0.0 {
					1.0
				} else {
					(1.0 - gap / margin).max(0.0)
				};
				let w = w * w * (3.0 - 2.0 * w);
				s.under + (s.front - s.under) * w
			})
			.fold(f32::MIN, f32::max)
	}
}

fn place(size: Size2, children: &[ChildState]) -> Vec<Placed> {
	let max = vec2(size.x as f32, size.y as f32);
	let t = TOPLEVEL_THICKNESS;
	let mut placed = vec![Placed {
		id: SurfaceId::Toplevel,
		min: Vec2::ZERO,
		max,
		input: vec![(Vec2::ZERO, max)],
		thickness: t,
		front: t / 2.0,
		under: t / 2.0,
	}];
	place_children(&mut placed, 0, children);
	placed
}
fn place_children(placed: &mut Vec<Placed>, parent: usize, children: &[ChildState]) {
	let (origin, pf, pu) = {
		let p = &placed[parent];
		(p.min, p.front, p.under)
	};
	let mut sorted = children.iter().collect::<Vec<_>>();
	sorted.sort_by_key(|c| c.info.z_order);
	let below = sorted.iter().filter(|c| c.info.z_order < 0).count();
	for (i, child) in sorted.into_iter().enumerate() {
		let g = child.info.geometry;
		let min = origin + vec2(g.origin.x as f32, g.origin.y as f32);
		let t = if child.info.input_regions.is_empty() {
			INPUTLESS_CHILD_THICKNESS
		} else {
			CHILD_THICKNESS
		};
		// below the parent recesses just behind its front face so the parent covers it
		let (front, under) = if i < below {
			(pf - Z_STEP * (below - i) as f32, pu)
		} else {
			(pf + t + Z_STEP * (i - below) as f32, pf)
		};
		placed.push(Placed {
			id: SurfaceId::Child { id: child.info.id },
			min,
			max: min + vec2(g.size.x as f32, g.size.y as f32),
			input: child
				.info
				.input_regions
				.iter()
				.map(|r| {
					let o = min + vec2(r.origin.x, r.origin.y);
					(o, o + vec2(r.size.x, r.size.y))
				})
				.collect(),
			thickness: t,
			front,
			under,
		});
		place_children(placed, placed.len() - 1, &child.children);
	}
}

fn cross(o: Vec2, a: Vec2, b: Vec2) -> f32 {
	(a - o).perp_dot(b - o)
}

fn hull(placed: &[Placed]) -> Vec<Vec2> {
	let mut pts = placed
		.iter()
		.filter(|s| !s.input.is_empty())
		.flat_map(|s| [s.min, vec2(s.max.x, s.min.y), s.max, vec2(s.min.x, s.max.y)])
		.collect::<Vec<_>>();
	pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
	pts.dedup();
	if pts.len() < 3 {
		return pts;
	}

	let mut h: Vec<Vec2> = Vec::with_capacity(pts.len() * 2);
	for &p in &pts {
		while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
			h.pop();
		}
		h.push(p);
	}
	let lower = h.len() + 1;
	for &p in pts.iter().rev().skip(1) {
		while h.len() >= lower && cross(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
			h.pop();
		}
		h.push(p);
	}
	h.pop();
	h
}
