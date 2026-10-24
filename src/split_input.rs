use crate::{Flatland, PANEL_THICKNESS, resize_handles::ResizeHandles, surfaces::Layer};
use glam::{Mat4, Quat, Vec3, vec2, vec3};
use serde::{Deserialize, Serialize};
use stardust_xr_asteroids::{
	CloneFnWrapper, Context, Curve, CustomElement as _, Element, Entity, Transformable as _,
	components::Derezzable,
	curve,
	elements::{Handle, Lines},
};
use stardust_xr_fusion::{
	fields::Shape,
	types::{Posef, rgba_linear},
};
use stardust_xr_molecules::lines::{LineExt as _, circle, line_from_points};
use std::{f32::consts::FRAC_PI_2, sync::Arc};

#[derive(Debug, Serialize, Deserialize)]
pub struct InputSurface {
	pub pose: Posef,
}

/// a quick ease out so a merge reads as snapping into the screen
fn merge_curve() -> Curve {
	curve(|t: f32| (t < 0.1).then(|| 1.0 - (1.0 - t / 0.1).powi(2)))
}

fn mix_pose(from: &Posef, to: &Posef, t: f32) -> Posef {
	Posef {
		position: Vec3::from(from.position).lerp(to.position.into(), t).into(),
		orientation: Quat::from(from.orientation)
			.slerp(to.orientation.into(), t)
			.into(),
	}
}

impl Flatland {
	pub fn split_handle(&self) -> Option<impl Element<Self> + use<>> {
		(self.split_input_surface.is_none() || self.pulling_split).then(|| {
			Handle::new(
				[-self.screen_size.x / 2.0, -self.screen_size.y / 2.0, 0.0],
				|state: &mut Self, pos| {
					state.pulling_split = true;
					state.pull_split(pos.into());
				},
			)
			.on_release(|state: &mut Self, _| state.pulling_split = false)
			.head_offset([0.0, -0.02, 0.0])
			.build()
		})
	}

	pub fn split_input(&self, context: &Context) -> Option<impl Element<Self> + use<>> {
		let split = self.split_input_surface.as_ref()?;
		let size = self.screen_size;
		Some(
			ResizeHandles::<Self> {
				pose: split.pose,
				size,
				min_size: Some(size),
				max_size: Some(size),
				on_change: CloneFnWrapper(Arc::new(|state, pose, _| state.move_split(pose))),
			}
			.build()
			.vanish(|t| {
				t.pose(self.screen_pose, merge_curve().via(mix_pose))
					.size(self.screen_size, merge_curve())
			})
			.child(
				Entity::new(Shape::Box {
					size: [size.x, size.y, PANEL_THICKNESS].into(),
				})
				.component(Derezzable::<Self>::new(|state| state.set_split(None)))
				.build(),
			)
			.child(self.surfaces(context, Layer::SplitInput))
			.maybe_child(self.split_cursor()),
		)
	}

	fn split_cursor(&self) -> Option<impl Element<Self> + use<>> {
		let item = self.panel_item.as_ref()?;
		item.cursor?;
		let screen = vec2(self.size_px().x as f32, self.size_px().y as f32);
		let pos = (vec2(item.cursor_pos.x, item.cursor_pos.y) - screen / 2.0) * vec2(1.0, -1.0)
			/ self.density;
		let ring = circle(16, 0.0, 0.003)
			.thickness(0.001)
			.color(rgba_linear!(1.0, 1.0, 1.0, 1.0))
			.transform(Mat4::from_rotation_x(FRAC_PI_2));
		Some(Lines::new([ring]).pos(pos.extend(0.001)).build())
	}

	pub fn split_tethers(&self) -> Option<impl Element<Self> + use<>> {
		let split = self.split_input_surface.as_ref()?;
		let corner = |pose: Posef, x: f32, y: f32| {
			let offset = vec3(x * self.screen_size.x, y * self.screen_size.y, 0.0) / 2.0;
			Vec3::from(pose.position) + Quat::from(pose.orientation) * offset
		};
		let near = Vec3::from(split.pose.position).distance(self.screen_pose.position.into()) < 0.1;
		// (x, split y, screen y), near the screen each corner leads to its match
		let tethers: &[(f32, f32, f32)] = if near {
			&[
				(-1.0, 1.0, 1.0),
				(1.0, 1.0, 1.0),
				(-1.0, -1.0, -1.0),
				(1.0, -1.0, -1.0),
			]
		} else {
			&[(-1.0, 1.0, -1.0), (1.0, 1.0, -1.0)]
		};
		let tethers = tethers.iter().map(|&(x, split_y, screen_y)| {
			line_from_points(vec![
				corner(split.pose, x, split_y),
				corner(self.screen_pose, x, screen_y),
			])
			.thickness(0.001)
			.color(rgba_linear!(1.0, 1.0, 1.0, 0.15))
		});
		Some(Lines::new(tethers).build())
	}

	fn pull_split(&mut self, pos: Vec3) {
		let d = pos - vec3(-self.screen_size.x, -self.screen_size.y, 0.0) / 2.0;
		if d.length() < 0.05 {
			self.set_split(None);
			return;
		}
		let gap = 0.03;
		// hinged on the bottom edge, so only the drag direction in the yz plane matters
		let r = Quat::from_rotation_x(-d.z.atan2(-d.y));
		let center = vec3(0.0, -self.screen_size.y / 2.0, 0.0)
			+ r * vec3(0.0, -(gap + self.screen_size.y / 2.0), 0.0);
		let q = Quat::from(self.screen_pose.orientation);
		self.set_split(Some(InputSurface {
			pose: Posef {
				position: (Vec3::from(self.screen_pose.position) + q * center).into(),
				orientation: (q * r).into(),
			},
		}));
	}

	fn set_split(&mut self, split: Option<InputSurface>) {
		// the touch planes that reported these are about to be replaced
		if split.is_some() != self.split_input_surface.is_some() {
			self.touch_hovers.clear();
		}
		self.split_input_surface = split;
	}

	fn move_split(&mut self, pose: Posef) {
		let d = Vec3::from(pose.position).distance(self.screen_pose.position.into());
		if d < 0.05 {
			self.set_split(None);
		} else if let Some(split) = &mut self.split_input_surface {
			split.pose = pose;
		}
	}
}
