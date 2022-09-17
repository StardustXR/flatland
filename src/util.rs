use std::ops::{Add, Div, Mul, Sub};

use mint::Vector2;

pub trait MapNumber<T> {
	type Output;

	fn map(self, from_range: (T, T), to_range: (T, T)) -> Self::Output;
}
impl<
		T: Copy + Add<T, Output = T> + Sub<T, Output = T> + Mul<T, Output = T> + Div<T, Output = T>,
	> MapNumber<T> for T
{
	type Output = Self;

	fn map(self, from_range: (T, T), to_range: (T, T)) -> T {
		to_range.0
			+ (self - from_range.0) * (to_range.1 - to_range.0) / (from_range.1 - from_range.0)
	}
}

impl<T: MapNumber<T, Output = T> + Copy> MapNumber<T> for Vector2<T> {
	type Output = Vector2<T>;

	fn map(self, from_range: (T, T), to_range: (T, T)) -> Vector2<T> {
		Vector2::from([
			self.x.map(from_range, to_range),
			self.y.map(from_range, to_range),
		])
	}
}
