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
	fn is_grass(&self) -> bool {
		matches!(self, Ground::Grass { .. })
	}
}

#[derive(Clone)]
enum Obj {
	Caravan,
	Tree,
	Crystal,
}

#[derive(Clone)]
struct Tile {
	ground: Ground,
	obj: Option<Obj>,
}
impl Tile {
	fn has_path(&self) -> bool {
		self.ground.is_path()
	}
	fn has_water(&self) -> bool {
		self.ground.is_water()
	}
	fn has_caravan(&self) -> bool {
		self
			.obj
			.as_ref()
			.is_some_and(|obj| matches!(obj, Obj::Caravan))
	}
	fn is_empty_grass(&self) -> bool {
		self.obj.is_none() && self.ground.is_grass()
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

	fn draw_tile_obj_at(&self, renderer: &mut Renderer, coords: Coords, dst: Rect) {
		match self.grid.get(coords).and_then(|tile| tile.obj.as_ref()) {
			None => {},
			Some(obj) => {
				draw_obj(renderer, obj, dst);
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

fn draw_obj(renderer: &mut Renderer, obj: &Obj, mut dst: Rect) {
	match obj {
		Obj::Caravan => {
			let sprite = Rect::tile((7, 2).into(), 16);
			dst.top_left.y -= dst.dims.h * 3 / 16;
			renderer.draw_sprite(dst, sprite, DrawSpriteEffects::none());
		},
		Obj::Tree => {
			let mut sprite = Rect::tile((4, 2).into(), 16);
			sprite.top_left.y -= 16;
			sprite.dims.h += 16;
			dst.top_left.y -= 8 * 8;
			dst.dims.h += 8 * 8;
			dst.top_left.y -= 8 * 8 / 8;
			renderer.draw_sprite(dst, sprite, DrawSpriteEffects::none());
		},
		Obj::Crystal => {
			let mut sprite = Rect::tile((3, 2).into(), 16);
			sprite.top_left.y -= 16;
			sprite.dims.h += 16;
			dst.top_left.y -= 8 * 8;
			dst.dims.h += 8 * 8;
			dst.top_left.y -= 8 * 8 / 8;
			renderer.draw_sprite(dst, sprite, DrawSpriteEffects::none());
		},
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
				obj: None,
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

		// Generate some trees.
		let dims = grid.dims;
		for coords in grid.dims.iter() {
			let tile = grid.get_mut(coords).unwrap();
			if tile.is_empty_grass() {
				let tree_probability = if coords.y == 0 || coords.y == dims.h - 1 {
					0.3
				} else {
					0.05
				};
				if rand::thread_rng().gen_range(0.0..1.0) < tree_probability {
					tile.obj = Some(Obj::Tree);
				}
			}
		}

		// Generate some crystals.
		let dims = grid.dims;
		for coords in grid.dims.iter() {
			let tile = grid.get_mut(coords).unwrap();
			if tile.is_empty_grass() {
				let crystal_probability = if coords.y == 1 || coords.y == dims.h - 2 {
					0.03
				} else {
					0.006
				};
				if rand::thread_rng().gen_range(0.0..1.0) < crystal_probability {
					tile.obj = Some(Obj::Crystal);
				}
			}
		}

		Chunk { grid, right_path_y }
	}
}

enum Action {
	Move { obj: Obj, from: Coords, to: Coords },
	CameraMoveX { from: f32, to: f32 },
}

struct Animation {
	action: Action,
	start: std::time::Instant,
	duration: std::time::Duration,
}

fn linear_interpolation(progress: f32, value_start: f32, value_end: f32) -> f32 {
	value_start + progress * (value_end - value_start)
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

	let left_path_y = rand::thread_rng().gen_range(1..9);
	let mut map = Map { grid: Grid::of_size_zero(), right_path_y: left_path_y };

	while map.grid.dims.w * 8 * 8 < renderer.dims().w {
		map.generate_chunk_on_the_right();
	}

	map.grid.get_mut((0, left_path_y).into()).unwrap().obj = Some(Obj::Caravan);

	let mut current_animation: Option<Animation> = None;

	let mut camera_x = 0.0;

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

				while map.grid.dims.w * 8 * 8 < (camera_x + 1.0) as i32 * 8 * 8 + renderer.dims().w {
					map.generate_chunk_on_the_right();
				}
			},

			WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::Space),
						..
					},
				..
			} if current_animation.is_none() => {
				for coords in map.grid.dims.iter() {
					if map.grid.get(coords).is_some_and(|tile| tile.has_caravan()) {
						if let Ground::Path { forward, .. } = map.grid.get(coords).unwrap().ground {
							let dst_coords = coords + forward;
							current_animation = Some(Animation {
								action: Action::Move {
									obj: map.grid.get_mut(coords).unwrap().obj.take().unwrap(),
									from: coords,
									to: dst_coords,
								},
								start: std::time::Instant::now(),
								duration: std::time::Duration::from_secs_f32(0.05),
							});
							break;
						}
					}
				}
			},

			WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::Return),
						..
					},
				..
			} if current_animation.is_none() => {
				current_animation = Some(Animation {
					action: Action::CameraMoveX { from: camera_x, to: camera_x + 1.0 },
					start: std::time::Instant::now(),
					duration: std::time::Duration::from_secs_f32(0.05),
				});
				while map.grid.dims.w * 8 * 8 < (camera_x + 1.0) as i32 * 8 * 8 + renderer.dims().w {
					map.generate_chunk_on_the_right();
				}
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

			let map_top = renderer.dims().h / 2 - 8 * 8 * map.grid.dims.h / 2;
			let map_left = -(camera_x * 8.0 * 8.0) as i32;

			for coords in map.grid.dims.iter() {
				let dst = Rect::xywh(
					map_left + 8 * 8 * coords.x,
					map_top + 8 * 8 * coords.y,
					8 * 8,
					8 * 8,
				);
				if dst.right_excluded() < 0 || renderer.dims().w < dst.left() {
					continue;
				}
				map.draw_tile_ground_at(&mut renderer, coords, dst);
			}

			for coords in map.grid.dims.iter() {
				let dst = Rect::xywh(
					map_left + 8 * 8 * coords.x,
					map_top + 8 * 8 * coords.y,
					8 * 8,
					8 * 8,
				);
				if dst.right_excluded() < 0 || renderer.dims().w < dst.left() {
					continue;
				}
				map.draw_tile_obj_at(&mut renderer, coords, dst);
			}

			if let Some(anim) = &current_animation {
				let progress = std::time::Instant::now()
					.duration_since(anim.start)
					.as_secs_f32() / anim.duration.as_secs_f32();
				if progress > 1.0 {
					match current_animation.take().unwrap().action {
						Action::Move { obj, to, .. } => map.grid.get_mut(to).unwrap().obj = Some(obj),
						Action::CameraMoveX { to, .. } => camera_x = to,
					}
				} else {
					match &anim.action {
						Action::Move { obj, from, to } => {
							let interp_x = {
								let from_x = map_left + 8 * 8 * from.x;
								let to_x = map_left + 8 * 8 * to.x;
								linear_interpolation(progress, from_x as f32, to_x as f32) as i32
							};
							let interp_y = {
								let from_y = map_top + 8 * 8 * from.y;
								let to_y = map_top + 8 * 8 * to.y;
								linear_interpolation(progress, from_y as f32, to_y as f32) as i32
							};
							let dst = Rect::xywh(interp_x, interp_y, 8 * 8, 8 * 8);
							draw_obj(&mut renderer, obj, dst);
						},
						Action::CameraMoveX { from, to } => {
							camera_x = linear_interpolation(progress, *from, *to);
						},
					}
				}
			}

			window.request_redraw();
		},

		Event::RedrawRequested(_) => {
			renderer.render();
		},

		_ => {},
	});
}
