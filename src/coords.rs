#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Coords {
	pub x: i32,
	pub y: i32,
}
impl From<(i32, i32)> for Coords {
	fn from((x, y): (i32, i32)) -> Coords {
		Coords { x, y }
	}
}
impl std::ops::Mul<i32> for Coords {
	type Output = Coords;
	fn mul(mut self, rhs: i32) -> Coords {
		self.x *= rhs;
		self.y *= rhs;
		self
	}
}
impl std::ops::Add<DxDy> for Coords {
	type Output = Coords;
	fn add(mut self, rhs: DxDy) -> Coords {
		self.x += rhs.dx;
		self.y += rhs.dy;
		self
	}
}
impl std::ops::AddAssign<DxDy> for Coords {
	fn add_assign(&mut self, rhs: DxDy) {
		self.x += rhs.dx;
		self.y += rhs.dy;
	}
}
impl std::ops::Sub<DxDy> for Coords {
	type Output = Coords;
	fn sub(mut self, rhs: DxDy) -> Coords {
		self.x -= rhs.dx;
		self.y -= rhs.dy;
		self
	}
}
impl std::ops::SubAssign<DxDy> for Coords {
	fn sub_assign(&mut self, rhs: DxDy) {
		self.x -= rhs.dx;
		self.y -= rhs.dy;
	}
}
impl std::ops::Sub<Coords> for Coords {
	type Output = DxDy;
	fn sub(mut self, rhs: Coords) -> DxDy {
		DxDy { dx: self.x - rhs.x, dy: self.y - rhs.y }
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DxDy {
	pub dx: i32,
	pub dy: i32,
}
impl DxDy {
	pub fn iter_4_directions() -> impl Iterator<Item = DxDy> {
		[(0, -1).into(), (1, 0).into(), (0, 1).into(), (-1, 0).into()].into_iter()
	}
}
impl From<(i32, i32)> for DxDy {
	fn from((dx, dy): (i32, i32)) -> DxDy {
		DxDy { dx, dy }
	}
}
impl From<Dimensions> for DxDy {
	fn from(dims: Dimensions) -> DxDy {
		DxDy { dx: dims.w, dy: dims.h }
	}
}
impl From<Coords> for DxDy {
	fn from(coords: Coords) -> DxDy {
		DxDy { dx: coords.x, dy: coords.y }
	}
}
impl std::ops::Neg for DxDy {
	type Output = DxDy;
	fn neg(mut self) -> DxDy {
		self.dx *= -1;
		self.dy *= -1;
		self
	}
}
impl std::ops::Mul<i32> for DxDy {
	type Output = DxDy;
	fn mul(mut self, rhs: i32) -> DxDy {
		self.dx *= rhs;
		self.dy *= rhs;
		self
	}
}

#[derive(Clone, Copy)]
pub struct CoordsF {
	pub x: f32,
	pub y: f32,
}
impl From<(f32, f32)> for CoordsF {
	fn from((x, y): (f32, f32)) -> CoordsF {
		CoordsF { x, y }
	}
}
impl std::ops::Add for CoordsF {
	type Output = CoordsF;
	fn add(mut self, rhs: CoordsF) -> CoordsF {
		self.x += rhs.x;
		self.y += rhs.y;
		self
	}
}
impl std::ops::Mul<f32> for CoordsF {
	type Output = CoordsF;
	fn mul(mut self, rhs: f32) -> CoordsF {
		self.x *= rhs;
		self.y *= rhs;
		self
	}
}
impl CoordsF {
	pub fn as_dxdy(self) -> DxDy {
		DxDy { dx: self.x.round() as i32, dy: self.y.round() as i32 }
	}
}

#[derive(Clone, Copy)]
pub struct Dimensions {
	pub w: i32,
	pub h: i32,
}
impl From<(i32, i32)> for Dimensions {
	fn from((w, h): (i32, i32)) -> Dimensions {
		Dimensions { w, h }
	}
}
impl From<winit::dpi::PhysicalSize<u32>> for Dimensions {
	fn from(size: winit::dpi::PhysicalSize<u32>) -> Dimensions {
		Dimensions { w: size.width as i32, h: size.height as i32 }
	}
}
impl Dimensions {
	pub fn square(side: i32) -> Dimensions {
		Dimensions { w: side, h: side }
	}

	pub fn area(self) -> usize {
		self.w as usize * self.h as usize
	}

	pub fn contains(self, coords: Coords) -> bool {
		0 <= coords.x && coords.x < self.w && 0 <= coords.y && coords.y < self.h
	}

	pub fn index_of_coords(self, coords: Coords) -> Option<usize> {
		if self.contains(coords) {
			Some((coords.y * self.w + coords.x) as usize)
		} else {
			None
		}
	}
}
impl std::ops::Mul<i32> for Dimensions {
	type Output = Dimensions;
	fn mul(mut self, rhs: i32) -> Dimensions {
		self.w *= rhs;
		self.h *= rhs;
		self
	}
}
impl std::ops::Add<DxDy> for Dimensions {
	type Output = Dimensions;
	fn add(mut self, rhs: DxDy) -> Dimensions {
		self.w += rhs.dx;
		self.h += rhs.dy;
		self
	}
}
impl std::ops::Sub<DxDy> for Dimensions {
	type Output = Dimensions;
	fn sub(mut self, rhs: DxDy) -> Dimensions {
		self.w -= rhs.dx;
		self.h -= rhs.dy;
		self
	}
}
impl Dimensions {
	pub fn iter(self) -> IterCoordsRect {
		IterCoordsRect::with_rect(Rect { top_left: (0, 0).into(), dims: self })
	}
}

pub struct IterCoordsRect {
	current: Coords,
	rect: Rect,
}
impl IterCoordsRect {
	pub fn with_rect(rect: Rect) -> IterCoordsRect {
		IterCoordsRect { current: rect.top_left, rect }
	}
}
impl Iterator for IterCoordsRect {
	type Item = Coords;
	fn next(&mut self) -> Option<Coords> {
		let coords = self.current;
		self.current.x += 1;
		if !self.rect.contains(self.current) {
			self.current.x = self.rect.left();
			self.current.y += 1;
		}
		if self.rect.contains(coords) {
			Some(coords)
		} else {
			None
		}
	}
}

#[derive(Clone, Copy)]
pub struct Rect {
	pub top_left: Coords,
	pub dims: Dimensions,
}
impl Rect {
	pub fn xywh(x: i32, y: i32, w: i32, h: i32) -> Rect {
		Rect { top_left: (x, y).into(), dims: (w, h).into() }
	}

	pub fn tile(coords: Coords, tiles_side: i32) -> Rect {
		Rect {
			top_left: Coords { x: coords.x * tiles_side, y: coords.y * tiles_side },
			dims: Dimensions::square(tiles_side),
		}
	}

	pub fn top(self) -> i32 {
		self.top_left.y
	}
	pub fn left(self) -> i32 {
		self.top_left.x
	}
	pub fn bottom_excluded(self) -> i32 {
		self.top_left.y + self.dims.h
	}
	pub fn right_excluded(self) -> i32 {
		self.top_left.x + self.dims.w
	}

	pub fn contains(self, coords: Coords) -> bool {
		self.left() <= coords.x
			&& coords.x < self.right_excluded()
			&& self.top() <= coords.y
			&& coords.y < self.bottom_excluded()
	}

	pub fn iter(self) -> IterCoordsRect {
		IterCoordsRect::with_rect(self)
	}

	pub fn add_margin(self, margin: i32) -> Rect {
		Rect {
			top_left: self.top_left - DxDy::from((margin, margin)),
			dims: self.dims + (margin * 2, margin * 2).into(),
		}
	}
}

#[derive(Clone)]
pub struct Grid<T> {
	pub dims: Dimensions,
	content: Vec<T>,
}

impl<T> Grid<T> {
	pub fn new(dims: Dimensions, initializer: impl FnMut(Coords) -> T) -> Grid<T> {
		Grid { dims, content: dims.iter().map(initializer).collect() }
	}

	pub fn get(&self, coords: Coords) -> Option<&T> {
		if let Some(index) = self.dims.index_of_coords(coords) {
			self.content.get(index)
		} else {
			None
		}
	}
	pub fn get_mut(&mut self, coords: Coords) -> Option<&mut T> {
		if let Some(index) = self.dims.index_of_coords(coords) {
			self.content.get_mut(index)
		} else {
			None
		}
	}
}
