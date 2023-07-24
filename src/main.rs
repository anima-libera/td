mod coords;
mod renderer;
use rand::Rng;

use crate::coords::*;
use crate::renderer::*;

#[derive(Clone)]
enum Ground {
	Grass { visual_variant: u8 },
	Path { forward: DxDy, backward: DxDy },
	Water,
}
impl Ground {
	fn is_path(&self) -> bool {
		matches!(self, Ground::Path { .. })
	}
	fn is_water(&self) -> bool {
		matches!(self, Ground::Water)
	}
}

#[derive(Clone)]
struct Tile {
	ground: Ground,
}
impl Tile {
	fn has_path(&self) -> bool {
		self.ground.is_path()
	}
	fn has_water(&self) -> bool {
		self.ground.is_water()
	}
}

struct Map {
	grid: Grid<Tile>,
	/// The y coordinate of the path on the right of the generated area.
	right_path_y: i32,
}

impl Map {
	fn draw_tile_ground_at(&self, renderer: &mut Renderer, coords: Coords, dst: Rect) {
		let ground = self.grid.get(coords).unwrap().ground.clone();
		match ground {
			Ground::Grass { visual_variant } => {
				let sprite = Rect::tile((visual_variant as i32, 0).into(), 16);
				renderer.draw_sprite(dst, sprite, DrawSpriteEffects::none());
			},
			Ground::Path { forward, backward } => {
				let (sprite_coords, flip_horizontally, flip_vertically, flip_diagonally_id) =
					if forward.dy == 0 && backward.dy == 0 {
						// Horizontal
						((4, 0), false, false, false)
					} else if forward.dx == 0 && backward.dx == 0 {
						// Vertical
						((4, 0), false, false, true)
					} else if (forward == DxDy::from((0, -1)) && backward == DxDy::from((-1, 0)))
						|| (backward == DxDy::from((0, -1)) && forward == DxDy::from((-1, 0)))
					{
						// From left to top
						((5, 0), false, false, false)
					} else if (forward == DxDy::from((0, 1)) && backward == DxDy::from((-1, 0)))
						|| (backward == DxDy::from((0, 1)) && forward == DxDy::from((-1, 0)))
					{
						// From left to bottom
						((5, 0), false, true, false)
					} else if (forward == DxDy::from((0, -1)) && backward == DxDy::from((1, 0)))
						|| (backward == DxDy::from((0, -1)) && forward == DxDy::from((1, 0)))
					{
						// From top to right
						((5, 0), true, false, false)
					} else if (forward == DxDy::from((0, 1)) && backward == DxDy::from((1, 0)))
						|| (backward == DxDy::from((0, 1)) && forward == DxDy::from((1, 0)))
					{
						// From bottom to right
						((5, 0), true, true, false)
					} else {
						unreachable!()
					};
				let sprite = Rect::tile(sprite_coords.into(), 16);
				renderer.draw_sprite(
					dst,
					sprite,
					DrawSpriteEffects {
						flip_horizontally,
						flip_vertically,
						flip_diagonally_id,
						paint: None,
					},
				);
			},
			Ground::Water => {
				let there_is_water_on_the_top =
					if let Some(tile_on_the_top) = self.grid.get(coords + DxDy::from((0, -1))) {
						tile_on_the_top.has_water()
					} else {
						true
					};
				let there_is_water_on_the_left =
					if let Some(tile_on_the_left) = self.grid.get(coords + DxDy::from((-1, 0))) {
						tile_on_the_left.has_water()
					} else {
						true
					};
				let sprite_coords_x = 6
					+ if there_is_water_on_the_top { 0 } else { 1 }
					+ if there_is_water_on_the_left { 0 } else { 2 };
				let sprite = Rect::tile((sprite_coords_x, 0).into(), 16);
				renderer.draw_sprite(dst, sprite, DrawSpriteEffects::none());
			},
		}
	}

	fn generate_chunk_on_the_right(&mut self) {
		let chunk = Chunk::generate(self.right_path_y);
		let grid = std::mem::replace(&mut self.grid, Grid::of_size_zero());
		let grid = grid.add_to_right(chunk.grid);
		self.grid = grid;
		self.right_path_y = chunk.right_path_y;
	}
}

/// A pice of world that can be generated independently.
struct Chunk {
	/// A 10x10 grid.
	grid: Grid<Tile>,
	/// The y coordinate of the path on the right of the chunk.
	right_path_y: i32,
}

impl Chunk {
	fn generate(path_y: i32) -> Chunk {
		let (mut grid, right_path_y) = 'try_new_path: loop {
			// Initialize with only grass.
			let mut grid = Grid::new((10, 10).into(), |_coords: Coords| Tile {
				ground: Ground::Grass {
					visual_variant: if rand::thread_rng().gen_range(0..4) == 0 {
						rand::thread_rng().gen_range(1..4)
					} else {
						0
					},
				},
			});

			// We generate the path by moving `cur_head` around randomly and drawing the path.
			// If it doesn't work then we just try again until it works >w<.
			let mut prev_head: Coords = (-1, path_y).into();
			let mut cur_head: Coords = (0, path_y).into();
			loop {
				if cur_head.x == grid.dims.w - 1 {
					let backward = prev_head - cur_head;
					grid.get_mut(cur_head).unwrap().ground =
						Ground::Path { forward: (1, 0).into(), backward };
					break;
				}

				let dxdy = DxDy::iter_4_directions()
					.nth(rand::thread_rng().gen_range(0..4))
					.unwrap();
				if grid
					.get(cur_head + dxdy)
					.is_some_and(|tile| !tile.has_path())
				{
					let backward = prev_head - cur_head;
					let forward = dxdy;
					grid.get_mut(cur_head).unwrap().ground = Ground::Path { forward, backward };
					prev_head = cur_head;
					cur_head += dxdy;
				} else {
					continue 'try_new_path;
				}
			}
			break (grid, cur_head.y);
		};

		// Generate some water.
		while rand::thread_rng().gen_range(0..3) == 0 {
			let mut coords = (
				rand::thread_rng().gen_range(0..grid.dims.w),
				rand::thread_rng().gen_range(0..grid.dims.h),
			)
				.into();
			loop {
				let tile = grid.get_mut(coords).unwrap();
				if tile.has_path() || tile.has_water() || rand::thread_rng().gen_range(0..3) == 0 {
					break;
				}
				tile.ground = Ground::Water;
				let dxdy = DxDy::iter_4_directions()
					.nth(rand::thread_rng().gen_range(0..4))
					.unwrap();
				if grid.get(coords + dxdy).is_some_and(|tile| !tile.has_path()) {
					coords += dxdy;
				}
			}
		}

		Chunk { grid, right_path_y }
	}
}

fn main() {
	env_logger::init();
	let event_loop = winit::event_loop::EventLoop::new();
	let window = winit::window::WindowBuilder::new()
		.with_title("Defend the caravan")
		.with_inner_size(winit::dpi::PhysicalSize::new(800, 800))
		.with_maximized(true)
		.build(&event_loop)
		.unwrap();

	// Center the window
	let screen_size = window.available_monitors().next().unwrap().size();
	let window_outer_size = window.outer_size();
	window.set_outer_position(winit::dpi::PhysicalPosition::new(
		screen_size.width / 2 - window_outer_size.width / 2,
		screen_size.height / 2 - window_outer_size.height / 2,
	));

	let mut renderer = Renderer::new(&window, Color::rgb_u8(30, 30, 50));

	let mut map = Map {
		grid: Grid::of_size_zero(),
		right_path_y: rand::thread_rng().gen_range(1..9),
	};

	map.generate_chunk_on_the_right();
	map.generate_chunk_on_the_right();
	map.generate_chunk_on_the_right();

	let mut last_time = std::time::Instant::now();

	use winit::event::*;
	event_loop.run(move |event, _, control_flow| match event {
		Event::WindowEvent { ref event, window_id } if window_id == window.id() => match event {
			WindowEvent::CloseRequested
			| WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::Escape),
						..
					},
				..
			} => {
				*control_flow = winit::event_loop::ControlFlow::Exit;
			},

			WindowEvent::Resized(new_size) => {
				renderer.resized((*new_size).into());
				window.request_redraw();
			},

			_ => {},
		},

		Event::MainEventsCleared => {
			let now = std::time::Instant::now();
			let dt = now.duration_since(last_time);
			last_time = now;
			let fps = 1.0 / dt.as_secs_f32();

			renderer.clear();

			Font {
				size_factor: 3,
				horizontal_spacing: 2,
				space_width: 7,
				foreground: Color::WHITE,
				background: Some(Color::BLACK),
				margins: (3, 3).into(),
			}
			.draw_text_line(&mut renderer, &format!("fps: {fps}"), (0, 0).into())
			.unwrap();

			for coords in map.grid.dims.iter() {
				let dst = Rect::xywh(8 * 8 * coords.x, 8 * 8 * coords.y, 8 * 8, 8 * 8);
				map.draw_tile_ground_at(&mut renderer, coords, dst);
			}

			window.request_redraw();
		},

		Event::RedrawRequested(_) => {
			renderer.render();
		},

		_ => {},
	});
}
