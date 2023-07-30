mod coords;
mod renderer;

use crate::coords::*;
use crate::renderer::*;

mod rand_wrapper {
	use rand::distributions::uniform::{SampleRange, SampleUniform};

	/// Just a wrapper around `rand::rng::Rng::gen_range`.
	/// It gets a random value in the given range,
	/// using the thread-local RNG given by `rand::thread_rng`.
	pub fn rand_range<T, R>(range: R) -> T
	where
		T: SampleUniform,
		R: SampleRange<T>,
	{
		use rand::{thread_rng, Rng};
		thread_rng().gen_range(range)
	}
}
use crate::rand_wrapper::*;

mod rodio_wrapper {
	use rodio::{Decoder, OutputStream, OutputStreamHandle, Source};
	use std::io::{BufReader, Cursor};

	/// Represent various sound effects embedded in the binary
	/// that can be played by being passed to `AudioPlayer::play_sound_effect`.
	#[derive(Clone, Copy)]
	pub enum SoundEffect {
		Pew,
		Hit,
	}

	impl SoundEffect {
		fn bytes(self) -> &'static [u8] {
			match self {
				SoundEffect::Pew => include_bytes!("../assets/sounds/pew01.wav").as_slice(),
				SoundEffect::Hit => include_bytes!("../assets/sounds/hit01.wav").as_slice(),
			}
		}

		fn volume(self) -> f32 {
			match self {
				SoundEffect::Pew => 0.4,
				SoundEffect::Hit => 0.4,
			}
		}
	}

	/// Just a wrapper around whatever `rodio::OutputStream::try_default` returns.
	pub struct AudioPlayer {
		_stream: OutputStream,
		stream_handle: OutputStreamHandle,
	}

	impl AudioPlayer {
		pub fn new() -> AudioPlayer {
			let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
			AudioPlayer { _stream, stream_handle }
		}

		pub fn play_sound_effect(&self, sound_effect: SoundEffect) {
			// TODO: See if we can call `Decoder::new` only once per sound effect
			// (in `AudioPlayer::new`) instead of here.
			self
				.stream_handle
				.play_raw(
					Decoder::new(BufReader::new(Cursor::new(sound_effect.bytes())))
						.unwrap()
						.convert_samples()
						.amplify(sound_effect.volume()),
				)
				.unwrap();
		}
	}
}
use crate::rodio_wrapper::*;

#[derive(Clone)]
struct Path {
	forward: CoordsDelta,
	backward: CoordsDelta,
	distance: i32,
}

#[derive(Clone)]
enum Ground {
	Grass { visual_variant: u32 },
	Path(Path),
	Water,
}
impl Ground {
	fn is_path(&self) -> bool {
		matches!(self, Ground::Path(_))
	}
	fn is_water(&self) -> bool {
		matches!(self, Ground::Water)
	}
	fn is_grass(&self) -> bool {
		matches!(self, Ground::Grass { .. })
	}

	fn path(&self) -> Option<&Path> {
		if let Ground::Path(path) = self {
			Some(path)
		} else {
			None
		}
	}
}

use std::time::{Duration, Instant};

/// A period over which something happens can be represented with this.
/// It also makes easier to know at which point of the progression we currently are.
///
/// For example, an animation can contain a `TimeProgression` that allows to set its duration
/// and to know at every frame at which point we are in the progression of the animation.
#[derive(Clone)]
struct TimeProgression {
	start: Instant,
	duration: Duration,
}

impl TimeProgression {
	fn new(duration: Duration) -> TimeProgression {
		TimeProgression { start: Instant::now(), duration }
	}

	/// Returns 0.0 if the represented period is just starting, 1.0 if it is just ending,
	/// and some ratio representing the progression when it is between its start and end.
	fn progress(&self) -> f32 {
		Instant::now().duration_since(self.start).as_secs_f32() / self.duration.as_secs_f32()
	}

	fn is_done(&self) -> bool {
		1.0 <= self.progress()
	}
}

/// Squishes a little to appear more alive than rocks.
#[derive(Clone)]
struct AliveAnimation {
	tp: TimeProgression,
}

/// Appear a certain color for a short time, for example flashing red when hit.
#[derive(Clone)]
struct ColoredAnimation {
	tp: TimeProgression,
	color: Color,
}

#[derive(Clone)]
enum Obj {
	Caravan,
	Tree,
	Rock {
		visual_variant: u32,
	},
	Crystal,
	EnemyBasic {
		can_play: bool,
		hp: i32,
		alive_animation: Option<AliveAnimation>,
		colored_animation: Option<ColoredAnimation>,
	},
	TowerBasic {
		can_play: bool,
	},
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
	fn has_enemy(&self) -> bool {
		self
			.obj
			.as_ref()
			.is_some_and(|obj| matches!(obj, Obj::EnemyBasic { .. }))
	}
	fn is_empty_grass(&self) -> bool {
		self.obj.is_none() && self.ground.is_grass()
	}

	fn path(&self) -> Option<&Path> {
		self.ground.path()
	}
}

struct Map {
	grid: Grid<Tile>,
}

impl Map {
	fn draw_tile_ground_at(&self, renderer: &mut Renderer, coords: Coords, dst: Rect) {
		let ground = self.grid.get(coords).unwrap().ground.clone();
		match ground {
			Ground::Grass { visual_variant } => {
				assert!(visual_variant < 4);
				let sprite = Rect::tile((visual_variant as i32, 0).into(), 16);
				renderer.draw_sprite(dst, sprite, DrawSpriteEffects::none());
			},
			Ground::Path(Path { forward, backward, .. }) => {
				// For now we just have a sprite of a streight path and of a L-turn.
				// By flipping them around various axes we can draw all the cases.
				let sprite_straight = (4, 0);
				let sprite_turn = (5, 0);
				fn is_turn(
					forward: CoordsDelta,
					backward: CoordsDelta,
					a: CoordsDelta,
					b: CoordsDelta,
				) -> bool {
					(forward == a && backward == b) || (backward == a && forward == b)
				}
				let (sprite_coords, flip_horizontally, flip_vertically, flip_diagonally_id) =
					if forward.dy == 0 && backward.dy == 0 {
						(sprite_straight, false, false, false) // Horizontal
					} else if forward.dx == 0 && backward.dx == 0 {
						(sprite_straight, false, false, true) // Vertical
					} else if is_turn(forward, backward, CoordsDelta::UP, CoordsDelta::LEFT) {
						(sprite_turn, false, false, false)
					} else if is_turn(forward, backward, CoordsDelta::DOWN, CoordsDelta::LEFT) {
						(sprite_turn, false, true, false)
					} else if is_turn(forward, backward, CoordsDelta::UP, CoordsDelta::RIGHT) {
						(sprite_turn, true, false, false)
					} else if is_turn(forward, backward, CoordsDelta::DOWN, CoordsDelta::RIGHT) {
						(sprite_turn, true, true, false)
					} else {
						panic!(
							"A path may has both its backward ({backward:?}) \
							and its forward ({forward:?}) directions be the same, \
							which doesn't make sense."
						);
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
				// Depending on weather there are water on some adjacent tiles, we render
				// different variants of the base water sprite.
				// This is done to give a sense of depth (the water level is thus
				// percieved as a bit below ground level).
				let there_is_water_on_the_top =
					if let Some(tile_on_the_top) = self.grid.get(coords + CoordsDelta::from((0, -1))) {
						tile_on_the_top.has_water()
					} else {
						false
					};
				let there_is_nothing_on_the_top =
					self.grid.get(coords + CoordsDelta::from((0, -1))).is_none();
				let there_is_ground_on_the_top_left_corner = if let Some(tile_on_the_top_left_corner) =
					self.grid.get(coords + CoordsDelta::from((-1, -1)))
				{
					!tile_on_the_top_left_corner.has_water()
				} else {
					false
				};
				let there_is_water_on_the_left =
					if let Some(tile_on_the_left) = self.grid.get(coords + CoordsDelta::from((-1, 0))) {
						tile_on_the_left.has_water()
					} else {
						true
					};
				let sprite_coords_x = 6
					+ if there_is_nothing_on_the_top {
						2
					} else if there_is_water_on_the_top && there_is_ground_on_the_top_left_corner {
						6
					} else if there_is_water_on_the_top {
						4
					} else {
						0
					} + if there_is_water_on_the_left { 0 } else { 1 };
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

	fn inflict_damage_to_obj_at(&mut self, coords: Coords, damages: i32) {
		let destroy = match self.grid.get_mut(coords).and_then(|tile| tile.obj.as_mut()) {
			None => false,
			Some(Obj::EnemyBasic { ref mut hp, ref mut colored_animation, .. }) => {
				*hp -= damages;
				*colored_animation = Some(ColoredAnimation {
					tp: TimeProgression::new(Duration::from_secs_f32(0.075)),
					color: Color::rgb_u8(255, 0, 0),
				});
				*hp <= 0
			},
			Some(_) => false,
		};
		if destroy {
			self.grid.get_mut(coords).unwrap().obj = None;
		}
	}

	fn rightmost_path_y_and_dist(&self) -> Option<(i32, i32)> {
		if self.grid.dims.w == 0 {
			return None;
		}
		for y in 0..self.grid.dims.h {
			let coords: Coords = (self.grid.dims.w - 1, y).into();
			if let Ground::Path(Path { distance, .. }) = self.grid.get(coords).unwrap().ground {
				return Some((y, distance));
			}
		}
		panic!("could not find a path on the rightmost column");
	}

	fn generate_chunk_on_the_right(&mut self) {
		let chunk = Chunk::generate(self.rightmost_path_y_and_dist());
		let grid = std::mem::replace(&mut self.grid, Grid::of_size_zero());
		let grid = grid.add_to_right(chunk.grid);
		self.grid = grid;
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
		Obj::Rock { visual_variant } => {
			assert!(*visual_variant < 3);
			let sprite = Rect::tile((*visual_variant as i32, 2).into(), 16);
			dst.top_left.y -= dst.dims.h * 3 / 16;
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
		Obj::EnemyBasic { hp, alive_animation, colored_animation, .. } => {
			let sprite = Rect::tile((4, 8).into(), 16);
			dst.top_left.y -= dst.dims.h * 3 / 16;
			let unsquished_dst = dst;
			if let Some(anim) = alive_animation {
				// The "alive" animation is meant to make the enemies look more alive than rocks.
				// When the animation triggers, the sprite of the enemy is supposed to squish a little
				// before coming back to its normal dimensions.
				if !anim.tp.is_done() {
					// At the point when the sprite is the most squished, it is `dst_squish`.
					let squish_x = 0.8;
					let squish_y = 1.2;
					let mut dst_squish = dst;
					dst_squish.top_left.x += (dst.dims.w as f32 * (1.0 - squish_x) / 2.0) as i32;
					dst_squish.dims.w -= (dst.dims.w as f32 * (1.0 - squish_x)) as i32;
					dst_squish.top_left.y += (dst.dims.h as f32 * (1.0 - squish_y)) as i32;
					dst_squish.dims.h -= (dst.dims.h as f32 * (1.0 - squish_y)) as i32;
					// The animation happens in two times: normal -> squished -> normal.
					let progress = anim.tp.progress();
					if progress < 0.5 {
						// Normal -> squihed.
						dst = linear_interpolation_rect(progress * 2.0, dst, dst_squish);
					} else {
						// Squished -> normal.
						dst = linear_interpolation_rect(progress * 2.0 - 1.0, dst_squish, dst);
					}
				}
			}
			let color = if let Some(anim) = colored_animation {
				// Handle the case when the enemy sprite is flashing in some color.
				if !anim.tp.is_done() {
					Some(anim.color)
				} else {
					None
				}
			} else {
				None
			};
			let mut effects = DrawSpriteEffects::none();
			effects.paint = color;
			renderer.draw_sprite(dst, sprite, effects);
			// Now we render the hp counter of the enemy above it (centered),
			// and we make it go up with the squishing during "alive" animations (because its cute!).
			let mut top_center = unsquished_dst.top_left;
			top_center.x += unsquished_dst.dims.w / 2;
			top_center.y += unsquished_dst.dims.h / 10 + (unsquished_dst.dims.h - dst.dims.h);
			Font {
				size_factor: 3,
				horizontal_spacing: 2,
				space_width: 7,
				foreground: color.unwrap_or(Color::WHITE),
				background: Some(Color::BLACK),
				margins: (3, 3).into(),
			}
			.draw_text_line(renderer, &format!("{hp}"), top_center, PinPoint::TOP_CENTER)
			.unwrap();
		},
		Obj::TowerBasic { .. } => {
			let sprite = Rect::tile((8, 4).into(), 16);
			dst.top_left.y -= dst.dims.h * 2 / 16;
			renderer.draw_sprite(dst, sprite, DrawSpriteEffects::none());
		},
	}
}

fn draw_shot(renderer: &mut Renderer, dst: Rect) {
	let sprite = Rect::tile((8, 6).into(), 16);
	renderer.draw_sprite(dst, sprite, DrawSpriteEffects::none());
}

/// A pice of world that can be generated independently.
struct Chunk {
	/// A 10x10 grid.
	grid: Grid<Tile>,
}

impl Chunk {
	fn generate(last_path_y_and_dist: Option<(i32, i32)>) -> Chunk {
		let mut grid = 'try_new_path: loop {
			// Initialize with only grass.
			let mut grid = Grid::new((10, 10).into(), |_coords: Coords| Tile {
				ground: Ground::Grass {
					visual_variant: if rand_range(0..4) == 0 {
						rand_range(1..4)
					} else {
						0
					},
				},
				obj: None,
			});

			// We generate the path by moving `cur_head` around randomly and drawing the path.
			// If it doesn't work then we just try again until it works >w<.
			let (path_y, mut path_dist) = last_path_y_and_dist
				.map(|(y, d)| (y, d + 1))
				.unwrap_or_else(|| (rand_range(0..grid.dims.h), 0));
			let mut prev_head: Coords = (-1, path_y).into();
			let mut cur_head: Coords = (0, path_y).into();
			loop {
				if cur_head.x == grid.dims.w - 1 {
					let backward = prev_head - cur_head;
					grid.get_mut(cur_head).unwrap().ground =
						Ground::Path(Path { forward: (1, 0).into(), backward, distance: path_dist });
					break;
				}

				let direction = CoordsDelta::iter_4_directions()
					.nth(rand_range(0..4))
					.unwrap();
				if grid
					.get(cur_head + direction)
					.is_some_and(|tile| !tile.has_path())
				{
					let backward = prev_head - cur_head;
					let forward = direction;
					grid.get_mut(cur_head).unwrap().ground =
						Ground::Path(Path { forward, backward, distance: path_dist });
					path_dist += 1;
					prev_head = cur_head;
					cur_head += direction;
				} else {
					continue 'try_new_path;
				}
			}
			break grid;
		};

		// Generate some water.
		while rand_range(0..3) == 0 {
			let mut coords = (rand_range(0..grid.dims.w), rand_range(0..grid.dims.h)).into();
			loop {
				let tile = grid.get_mut(coords).unwrap();
				if tile.has_path() || tile.has_water() || rand_range(0..3) == 0 {
					break;
				}
				tile.ground = Ground::Water;
				let dxdy = CoordsDelta::iter_4_directions()
					.nth(rand_range(0..4))
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
				if rand_range(0.0..1.0) < tree_probability {
					tile.obj = Some(Obj::Tree);
				}
			}
		}

		// Generate some rocks.
		for coords in grid.dims.iter() {
			let tile = grid.get_mut(coords).unwrap();
			if tile.is_empty_grass() {
				let rock_probability = 0.05;
				if rand_range(0.0..1.0) < rock_probability {
					tile.obj = Some(Obj::Rock { visual_variant: rand_range(0..3) });
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
				if rand_range(0.0..1.0) < crystal_probability {
					tile.obj = Some(Obj::Crystal);
				}
			}
		}

		// Generate some enemies.
		for coords in grid.dims.iter() {
			let tile = grid.get_mut(coords).unwrap();
			if tile.has_path() {
				let enemy_probability = 0.4;
				if rand_range(0.0..1.0) < enemy_probability {
					tile.obj = Some(Obj::EnemyBasic {
						can_play: false,
						hp: 8,
						alive_animation: None,
						colored_animation: None,
					});
				}
			}
		}

		Chunk { grid }
	}
}

enum Action {
	Move { obj: Obj, from: Coords, to: Coords },
	CameraMoveX { from: f32, to: f32 },
	Appear { obj: Obj, to: Coords },
	Disappear { obj: Obj, from: Coords },
	Shoot { from: Coords, to: Coords },
}

struct Animation {
	action: Action,
	tp: TimeProgression,
}

/// When `progress` is 0.0 it returns `value_start`, 1.0 returns `value_end`
/// and inbetween it does a linear interpolation (no way !!!).
fn linear_interpolation(progress: f32, value_start: f32, value_end: f32) -> f32 {
	value_start + progress * (value_end - value_start)
}

fn linear_interpolation_rect(progress: f32, value_start: Rect, value_end: Rect) -> Rect {
	Rect::xywh(
		linear_interpolation(
			progress,
			value_start.top_left.x as f32,
			value_end.top_left.x as f32,
		)
		.round() as i32,
		linear_interpolation(
			progress,
			value_start.top_left.y as f32,
			value_end.top_left.y as f32,
		)
		.round() as i32,
		linear_interpolation(progress, value_start.dims.w as f32, value_end.dims.w as f32).round()
			as i32,
		linear_interpolation(progress, value_start.dims.h as f32, value_end.dims.h as f32).round()
			as i32,
	)
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

	let mut renderer = Renderer::new(&window, Color::rgb_u8(80, 80, 200));

	let audio_player = AudioPlayer::new();

	let mut map = Map { grid: Grid::of_size_zero() };

	while map.grid.dims.w * 8 * 8 < renderer.dims().w {
		map.generate_chunk_on_the_right();
	}

	for x in 0..15 {
		for y in 0..map.grid.dims.h {
			let coords = (x, y).into();
			if map.grid.get(coords).unwrap().has_enemy() {
				map.grid.get_mut(coords).unwrap().obj = None;
			}
		}
	}

	for y in 0..map.grid.dims.h {
		let coords = (0, y).into();
		if let Ground::Path(Path { distance: 0, .. }) = map.grid.get(coords).unwrap().ground {
			map.grid.get_mut(coords).unwrap().obj = Some(Obj::Caravan);
		}
	}

	#[derive(PartialEq, Eq)]
	enum Phase {
		Player,
		Enemy,
		Tower,
		GameOver,
	}
	let mut phase = Phase::Player;

	let mut turn_counter = 0;
	let mut distance_traveled = 0;
	let mut crystal_amount = 20;

	let mut current_animation: Option<Animation> = None;
	let mut end_player_phase_after_animation = false;
	let mut end_player_phase_right_now = false;

	let mut camera_x = 0.0;

	let mut cursor_position = Coords::from((0, 0));
	let mut hovered_tile_coords: Option<Coords> = None;
	let mut selected_tile_coords: Option<Coords> = None;

	let mut display_path_dist = false;

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

				while map.grid.dims.w * 8 * 8 <= (camera_x + 1.0) as i32 * 8 * 8 + renderer.dims().w + 1
				{
					map.generate_chunk_on_the_right();
				}
			},

			WindowEvent::CursorMoved { position, .. } => {
				cursor_position = (position.x.floor() as i32, position.y.floor() as i32).into();

				let map_top = renderer.dims().h / 2 - 8 * 8 * map.grid.dims.h / 2;
				let map_left = -(camera_x * 8.0 * 8.0) as i32;
				let coords = Coords::from((
					((cursor_position.x as f64 - map_left as f64) / (8.0 * 8.0)).floor() as i32,
					((cursor_position.y as f64 - map_top as f64) / (8.0 * 8.0)).floor() as i32,
				));
				if map.grid.dims.contains(coords) {
					hovered_tile_coords = Some(coords);
				} else {
					hovered_tile_coords = None;
				}
			},

			WindowEvent::CursorLeft { .. } => {
				hovered_tile_coords = None;
			},

			WindowEvent::MouseInput {
				state: ElementState::Pressed,
				button: MouseButton::Left,
				..
			} => {
				#[allow(clippy::unnecessary_unwrap)] // `if let &&` is not stable yet you nincompoop
				if selected_tile_coords.is_some()
					&& selected_tile_coords == hovered_tile_coords
					&& current_animation.is_none()
					&& phase == Phase::Player
				{
					let tile = map.grid.get(selected_tile_coords.unwrap()).unwrap().clone();
					let tower_price = 10;
					if tile.obj.is_none() && !tile.has_water() && crystal_amount >= tower_price {
						// Place a tower on empty ground.
						current_animation = Some(Animation {
							action: Action::Appear {
								obj: Obj::TowerBasic { can_play: false },
								to: selected_tile_coords.unwrap(),
							},
							tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
						});
						crystal_amount -= tower_price;
						end_player_phase_after_animation = true;
					} else if matches!(tile.obj, Some(Obj::Crystal))
						&& current_animation.is_none()
						&& phase == Phase::Player
					{
						// Mine the crystal.
						current_animation = Some(Animation {
							action: Action::Disappear {
								obj: map
									.grid
									.get_mut(selected_tile_coords.unwrap())
									.unwrap()
									.obj
									.take()
									.unwrap(),
								from: selected_tile_coords.unwrap(),
							},
							tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
						});
						crystal_amount += 30;
						end_player_phase_after_animation = true;
					}
				} else {
					selected_tile_coords = hovered_tile_coords;
				}
			},

			WindowEvent::MouseInput {
				state: ElementState::Pressed,
				button: MouseButton::Right,
				..
			} => {
				selected_tile_coords = None;
			},

			WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::Space),
						..
					},
				..
			} if current_animation.is_none() && phase == Phase::Player => {
				for coords in map.grid.dims.iter() {
					if map.grid.get(coords).is_some_and(|tile| tile.has_caravan()) {
						if let Some(Path { forward, distance, .. }) =
							map.grid.get(coords).unwrap().path().cloned()
						{
							let dst_coords = coords + forward;
							if map.grid.get(dst_coords).unwrap().obj.is_none() {
								current_animation = Some(Animation {
									action: Action::Move {
										obj: map.grid.get_mut(coords).unwrap().obj.take().unwrap(),
										from: coords,
										to: dst_coords,
									},
									tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
								});
								distance_traveled = distance + 1;
								end_player_phase_after_animation = true;
							}
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
			} if current_animation.is_none() && phase == Phase::Player => {
				current_animation = Some(Animation {
					action: Action::CameraMoveX { from: camera_x, to: camera_x + 1.0 },
					tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
				});
				while map.grid.dims.w * 8 * 8 <= (camera_x + 1.0) as i32 * 8 * 8 + renderer.dims().w + 1
				{
					map.generate_chunk_on_the_right();
				}
				end_player_phase_after_animation = true;
			},

			WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::S),
						..
					},
				..
			} if current_animation.is_none() && phase == Phase::Player => {
				end_player_phase_right_now = true;
			},

			WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state,
						virtual_keycode: Some(VirtualKeyCode::RShift | VirtualKeyCode::LShift),
						..
					},
				..
			} => {
				display_path_dist = *state == ElementState::Pressed;
			},

			_ => {},
		},

		Event::MainEventsCleared => {
			let now = std::time::Instant::now();
			let dt = now.duration_since(last_time);
			last_time = now;
			let fps = 1.0 / dt.as_secs_f32();

			//std::thread::sleep(Duration::from_secs_f32(0.003));

			// Enemy alive animations.
			for coords in map.grid.dims.iter() {
				if let Some(Obj::EnemyBasic { alive_animation, .. }) =
					&mut map.grid.get_mut(coords).unwrap().obj
				{
					if let Some(anim) = alive_animation {
						let progress = anim.tp.progress();
						if progress > 10.0 {
							// We wait until way too long after the end of the animation to remove it
							// so that there is a kind of cooldown for the animation per enemy.
							*alive_animation = None;
						}
					} else if rand_range(0.0..0.1) < 0.001 {
						*alive_animation = Some(AliveAnimation {
							tp: TimeProgression::new(Duration::from_secs_f32(0.3)),
						});
					}
				}
			}

			renderer.clear();

			Font {
				size_factor: 2,
				horizontal_spacing: 2,
				space_width: 7,
				foreground: Color::WHITE,
				background: Some(Color::BLACK),
				margins: (3, 3).into(),
			}
			.draw_text_line(
				&mut renderer,
				&format!("fps: {fps}"),
				(0, 0).into(),
				PinPoint::TOP_LEFT,
			)
			.unwrap();

			let font_white_3 = Font {
				size_factor: 3,
				horizontal_spacing: 2,
				space_width: 7,
				foreground: Color::WHITE,
				background: None,
				margins: (0, 0).into(),
			};

			{
				let text_rect = font_white_3
					.draw_text_line(
						&mut renderer,
						&format!("{crystal_amount}"),
						(0, 30).into(),
						PinPoint::TOP_LEFT,
					)
					.unwrap();
				let crystal_symbol_dst = Rect::xywh(
					text_rect.right_excluded() + 5,
					text_rect.top() - 8 * 3 / 2 + text_rect.dims.h / 2,
					8 * 3,
					8 * 3,
				);
				renderer.draw_sprite(
					crystal_symbol_dst,
					Rect::xywh(0, 23, 6, 6),
					DrawSpriteEffects::none(),
				);
			}

			font_white_3
				.draw_text_line(
					&mut renderer,
					&format!("turn {turn_counter}"),
					(0, 60).into(),
					PinPoint::TOP_LEFT,
				)
				.unwrap();
			font_white_3
				.draw_text_line(
					&mut renderer,
					&format!("traveled {distance_traveled} tiles"),
					(0, 80).into(),
					PinPoint::TOP_LEFT,
				)
				.unwrap();

			if phase != Phase::GameOver {
				font_white_3
					.draw_text_line(
						&mut renderer,
						&format!(
							"{} phase",
							match phase {
								Phase::Player => "player",
								Phase::Enemy => "enemy",
								Phase::Tower => "tower",
								_ => panic!("should not be here then"),
							}
						),
						(0, 110).into(),
						PinPoint::TOP_LEFT,
					)
					.unwrap();
			} else {
				Font {
					size_factor: 6,
					horizontal_spacing: 4,
					space_width: 15,
					foreground: Color::rgb_u8(255, 0, 0),
					background: None,
					margins: (0, 0).into(),
				}
				.draw_text_line(
					&mut renderer,
					"game over >_<",
					(0, 110).into(),
					PinPoint::TOP_LEFT,
				)
				.unwrap();
			}

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

			if let Some(coords) = hovered_tile_coords {
				let dst = Rect::xywh(
					map_left + 8 * 8 * coords.x,
					map_top + 8 * 8 * coords.y,
					8 * 8,
					8 * 8,
				);
				renderer.draw_rect_edge(dst, Color::rgb_u8(255, 60, 0));
			}
			if let Some(coords) = selected_tile_coords {
				let dst = Rect::xywh(
					map_left + 8 * 8 * coords.x,
					map_top + 8 * 8 * coords.y,
					8 * 8,
					8 * 8,
				);
				renderer.draw_rect_edge(dst, Color::rgb_u8(255, 255, 80));
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
				if anim.tp.is_done() {
					// The current animation is finished.
					match current_animation.take().unwrap().action {
						Action::Move { obj, to, .. } => map.grid.get_mut(to).unwrap().obj = Some(obj),
						Action::CameraMoveX { to, .. } => camera_x = to,
						Action::Appear { obj, to } => map.grid.get_mut(to).unwrap().obj = Some(obj),
						Action::Disappear { .. } => {},
						Action::Shoot { to, .. } => {
							map.inflict_damage_to_obj_at(to, 1);
							audio_player.play_sound_effect(SoundEffect::Hit);
						},
					}
					if end_player_phase_after_animation {
						end_player_phase_after_animation = false;
						end_player_phase_right_now = false;
						phase = Phase::Enemy;
						for coords in map.grid.dims.iter() {
							if let Some(Obj::EnemyBasic { ref mut can_play, .. }) =
								map.grid.get_mut(coords).unwrap().obj
							{
								*can_play = true;
							}
						}
					}
				} else {
					let progress = anim.tp.progress();
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

							let map_top = renderer.dims().h / 2 - 8 * 8 * map.grid.dims.h / 2;
							let map_left = -(camera_x * 8.0 * 8.0) as i32;
							let coords = Coords::from((
								((cursor_position.x as f64 - map_left as f64) / (8.0 * 8.0)).floor() as i32,
								((cursor_position.y as f64 - map_top as f64) / (8.0 * 8.0)).floor() as i32,
							));
							if map.grid.dims.contains(coords) {
								hovered_tile_coords = Some(coords);
							} else {
								hovered_tile_coords = None;
							}
						},
						Action::Appear { obj, to } => {
							let mut dst = Rect::xywh(
								map_left + 8 * 8 * to.x,
								map_top + 8 * 8 * to.y,
								8 * 8,
								8 * 8,
							);
							dst.top_left.x += (((8 * 8) / 2) as f32 * (1.0 - progress)) as i32;
							dst.dims.w = ((8 * 8) as f32 * progress) as i32;
							dst.top_left.y += (((8 * 8) / 2) as f32 * (1.0 - progress)) as i32;
							dst.dims.h = ((8 * 8) as f32 * progress) as i32;
							draw_obj(&mut renderer, obj, dst);
						},
						Action::Disappear { obj, from } => {
							let mut dst = Rect::xywh(
								map_left + 8 * 8 * from.x,
								map_top + 8 * 8 * from.y,
								8 * 8,
								8 * 8,
							);
							dst.top_left.x += (((8 * 8) / 2) as f32 * progress) as i32;
							dst.dims.w = ((8 * 8) as f32 * (1.0 - progress)) as i32;
							dst.top_left.y += (((8 * 8) / 2) as f32 * progress) as i32;
							dst.dims.h = ((8 * 8) as f32 * (1.0 - progress)) as i32;
							draw_obj(&mut renderer, obj, dst);
						},
						Action::Shoot { from, to } => {
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
							draw_shot(&mut renderer, dst);
						},
					}
				}
			} else if end_player_phase_right_now {
				end_player_phase_after_animation = false;
				end_player_phase_right_now = false;
				phase = Phase::Enemy;
				for coords in map.grid.dims.iter() {
					if let Some(Obj::EnemyBasic { ref mut can_play, .. }) =
						map.grid.get_mut(coords).unwrap().obj
					{
						*can_play = true;
					}
				}
			} else {
				// There might be something to do now.
				if phase == Phase::Enemy {
					// The enemies shall play now.
					// We make the enemies closer to the caravan play first so that they don't
					// bump into each other too much.
					// Closer to the caravan here means on a path tile with the smallest distance.
					// First we find the coords of the closest enemy (if any).
					let mut min_path_dist_and_coords: Option<(i32, Coords)> = None;
					for coords in map.grid.dims.iter() {
						let tile = map.grid.get(coords).unwrap();
						if let Some(Obj::EnemyBasic { can_play: true, .. }) = tile.obj {
							if let Some(Path { distance, .. }) = tile.path() {
								if min_path_dist_and_coords.is_none()
									|| min_path_dist_and_coords
										.is_some_and(|(dist_min, _)| *distance < dist_min)
								{
									min_path_dist_and_coords = Some((*distance, coords));
								}
							}
						}
					}
					if let Some((_, coords)) = min_path_dist_and_coords {
						// Found the closest enemy that hasn't played yet. This enemy plays now.
						let tile = map.grid.get_mut(coords).unwrap();
						if let Some(Obj::EnemyBasic { can_play: ref mut can_play @ true, .. }) = tile.obj
						{
							*can_play = false;
							let backward = if let Some(Path { backward, .. }) = tile.path() {
								*backward
							} else {
								panic!("enemy not on a path")
							};
							let dst_coords = coords + backward;
							if map.grid.get(dst_coords).is_some_and(|dst_tile| {
								dst_tile.obj.is_none()
									|| dst_tile.obj.as_ref().is_some_and(|obj| {
										matches!(obj, Obj::Caravan | Obj::TowerBasic { .. })
									})
							}) {
								current_animation = Some(Animation {
									action: Action::Move {
										obj: map.grid.get_mut(coords).unwrap().obj.take().unwrap(),
										from: coords,
										to: dst_coords,
									},
									tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
								});
							}
						}
					} else {
						// No enemies left to play.
						// We finish some enemy buisness and get to next phase.

						// Enemy spawn
						while map.grid.dims.w * 8 * 8
							<= (camera_x + 1.0) as i32 * 8 * 8 + renderer.dims().w + 1
						{
							map.generate_chunk_on_the_right();
						}
						let spawn_coords: Coords = 'spawn_coords: {
							let right = (camera_x + 1.0) as i32 + renderer.dims().w / (8 * 8);
							for y in 0..map.grid.dims.h {
								if map.grid.get((right, y).into()).unwrap().has_path() {
									break 'spawn_coords (right, y).into();
								}
							}
							panic!("no path one some column ?");
						};
						let spawn_tile = map.grid.get_mut(spawn_coords).unwrap();
						if spawn_tile.obj.is_none() && rand_range(0.0..1.0) < 0.4 {
							let rand = rand_range(0.0..1.0);
							let hp = if rand < 0.1 {
								12
							} else if rand < 0.3 {
								10
							} else {
								8
							};
							spawn_tile.obj = Some(Obj::EnemyBasic {
								can_play: false,
								hp,
								alive_animation: None,
								colored_animation: None,
							});
						}

						// Get to next phase
						phase = Phase::Tower;
						for coords in map.grid.dims.iter() {
							if let Some(Obj::TowerBasic { ref mut can_play }) =
								map.grid.get_mut(coords).unwrap().obj
							{
								*can_play = true;
							}
						}
					}
				} else if phase == Phase::Tower {
					// Towers gonna shoot!
					let mut found_an_tower_to_make_play = false;
					for coords in map.grid.dims.iter_left_to_right() {
						let tile = map.grid.get_mut(coords).unwrap();
						if let Some(Obj::TowerBasic { can_play: ref mut can_play @ true }) = tile.obj {
							*can_play = false;

							// Towers will shoot at the enemy that they see that is the closest to
							// the caravan, it seems like a nice default heuristic.
							let mut min_path_dist_and_coords: Option<(i32, Coords)> = None;
							for direction in CoordsDelta::iter_4_directions() {
								let mut view_coords = coords + direction;
								loop {
									let tile = map.grid.get(view_coords);
									if tile.is_none() {
										break;
									}
									let tile = tile.unwrap();
									if tile.has_enemy() {
										if let Some(Path { distance, .. }) = tile.path() {
											if min_path_dist_and_coords.is_none()
												|| min_path_dist_and_coords
													.is_some_and(|(dist_min, _)| *distance < dist_min)
											{
												min_path_dist_and_coords = Some((*distance, view_coords));
												break;
											}
										}
									}
									if tile.obj.is_some() {
										break;
									}
									view_coords += direction;
								}
							}

							if let Some((_, target_coords)) = min_path_dist_and_coords {
								// Shoot!
								let dist_to_target = coords.dist(target_coords);
								current_animation = Some(Animation {
									action: Action::Shoot { from: coords, to: target_coords },
									tp: TimeProgression::new(Duration::from_secs_f32(
										0.05 * dist_to_target as f32,
									)),
								});
								audio_player.play_sound_effect(SoundEffect::Pew);
							}

							found_an_tower_to_make_play = true;
							break;
						}
					}
					if !found_an_tower_to_make_play {
						let the_caravan_is_still_there = 'search_the_caravan: {
							for coords in map.grid.dims.iter() {
								if map.grid.get(coords).unwrap().has_caravan() {
									break 'search_the_caravan true;
								}
							}
							false
						};
						if the_caravan_is_still_there {
							phase = Phase::Player;
							turn_counter += 1;
						} else {
							phase = Phase::GameOver;
						}
					}
				}
			}

			if display_path_dist {
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
					let distance = if let Ground::Path(Path { distance, .. }) =
						map.grid.get(coords).unwrap().ground
					{
						distance
					} else {
						continue;
					};
					let center = dst.top_left + CoordsDelta::from(dst.dims) / 2;
					Font {
						size_factor: 3,
						horizontal_spacing: 2,
						space_width: 7,
						foreground: Color::rgb_u8(80, 255, 255),
						background: Some(Color::BLACK),
						margins: (3, 3).into(),
					}
					.draw_text_line(
						&mut renderer,
						&format!("{distance}"),
						center,
						PinPoint::CENTER_CENTER,
					)
					.unwrap();
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
