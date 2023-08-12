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

	/// Represents various sound effects embedded in the binary
	/// that can be played by being passed to `AudioPlayer::play_sound_effect`.
	#[derive(Clone, Copy)]
	pub enum SoundEffect {
		Pew,
		Hit,
		Step,
		Mine,
		Place,
	}

	impl SoundEffect {
		fn bytes(self) -> &'static [u8] {
			match self {
				SoundEffect::Pew => include_bytes!("../assets/sounds/pew01.wav").as_slice(),
				SoundEffect::Hit => include_bytes!("../assets/sounds/hit01.wav").as_slice(),
				SoundEffect::Step => include_bytes!("../assets/sounds/step01.wav").as_slice(),
				SoundEffect::Mine => include_bytes!("../assets/sounds/mine01.wav").as_slice(),
				SoundEffect::Place => include_bytes!("../assets/sounds/place01.wav").as_slice(),
			}
		}

		fn volume(self) -> f32 {
			match self {
				SoundEffect::Pew => 0.4,
				SoundEffect::Hit => 0.4,
				SoundEffect::Step => 0.15,
				SoundEffect::Mine => 0.3,
				SoundEffect::Place => 0.6,
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

/// A path tile info.
/// The path is an oriented non-crossing line of tiles, over which the caravan and enemies move.
#[derive(Clone)]
struct Path {
	/// The direction in which the caravan will move. An other path tile is expected there.
	forward: CoordsDelta,
	/// The direction in which the enemies will move. An other path tile is expected there.
	backward: CoordsDelta,
	/// The distance in tiles, along the path, from the left-most path tile.
	/// Moving `forward` leads to a path tile with an incremented `distance`,
	/// and `backward` leads to a decremented `distance`.
	distance: i32,
}

/// The ground of a tile doesn't move (unlike `Obj`s).
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

#[derive(Clone)]
enum Tower {
	Basic,
	Pink,
	Blue,
}
impl Tower {
	fn initial_hp(&self) -> i32 {
		match self {
			Tower::Basic => 3,
			Tower::Pink => 4,
			Tower::Blue => 3,
		}
	}
	fn shot(&self) -> Shot {
		match self {
			Tower::Basic => Shot {
				damages: 1,
				fire: 0,
				additional_actions: 0,
				cascade: ShotCascade::None,
			},
			Tower::Pink => Shot {
				damages: -1,
				fire: 0,
				additional_actions: 0,
				cascade: ShotCascade::SplitInTwo(Box::new(Shot {
					damages: 3,
					fire: 0,
					additional_actions: 0,
					cascade: ShotCascade::None,
				})),
			},
			Tower::Blue => Shot {
				damages: 0,
				additional_actions: 2,
				fire: 0,
				cascade: ShotCascade::Piercing(Box::new(Shot {
					damages: 1,
					additional_actions: 0,
					fire: 0,
					cascade: ShotCascade::Piercing(Box::new(Shot {
						damages: 0,
						additional_actions: 0,
						fire: 4,
						cascade: ShotCascade::None,
					})),
				})),
			},
		}
	}
}

#[derive(Clone)]
enum Enemy {
	Basic,
}

/// An object that can be on a tile and maybe move or do stuff.
#[derive(Clone)]
enum Obj {
	Caravan,
	Tree,
	Rock {
		visual_variant: u32,
	},
	Crystal,
	Enemy {
		actions: i32,
		hp: i32,
		fire: i32,
		alive_animation: Option<AliveAnimation>,
		colored_animation: Option<ColoredAnimation>,
		#[allow(dead_code)] // It will be used pretty soon!
		variant: Enemy,
	},
	Tower {
		actions: i32,
		hp: i32,
		fire: i32,
		colored_animation: Option<ColoredAnimation>,
		variant: Tower,
	},
}

/// Small object animation: Squishes a little to appear more alive than rocks.
#[derive(Clone)]
struct AliveAnimation {
	tp: TimeProgression,
}

/// Small object animation: Appear a certain color for a short time,
/// for example flashing red when hit.
#[derive(Clone)]
struct ColoredAnimation {
	tp: TimeProgression,
	color: Color,
}

impl Obj {
	fn hp(&self) -> Option<i32> {
		match self {
			Obj::Enemy { hp, .. } => Some(*hp),
			Obj::Tower { hp, .. } => Some(*hp),
			_ => None,
		}
	}
}

/// Tile ^^.
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
			.is_some_and(|obj| matches!(obj, Obj::Enemy { .. }))
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
	/// Draws the ground of the tile designated by the given `coords` to `dst` in the pixel buffer.
	///
	/// The drawing of some types of ground depends on the surrounding tiles, which is why
	/// this is a method of `Map` instead of `Ground`.
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
				/// Checks for one of the 4 possible L-turns.
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
					if let Some(tile_on_the_top) = self.grid.get(coords + (0, -1).into()) {
						tile_on_the_top.has_water()
					} else {
						false
					};
				let there_is_nothing_on_the_top = self.grid.get(coords + (0, -1).into()).is_none();
				let there_is_ground_on_the_top_left_corner =
					if let Some(tile_on_the_top_left_corner) = self.grid.get(coords + (-1, -1).into()) {
						!tile_on_the_top_left_corner.has_water()
					} else {
						false
					};
				let there_is_water_on_the_left =
					if let Some(tile_on_the_left) = self.grid.get(coords + (-1, 0).into()) {
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
				draw_obj(renderer, obj, dst, false);
			},
		}
	}

	fn _draw(&self, _renderer: &mut Renderer, _config: MapDrawingConfig) {
		todo!()
	}

	fn shot_hits_obj_at(&mut self, coords: Coords, shot: &Shot) {
		self.inflict_damage_to_obj_at(coords, shot.damages);
		if shot.fire > 0 {
			match self.grid.get_mut(coords).and_then(|tile| tile.obj.as_mut()) {
				Some(Obj::Enemy { ref mut fire, ref mut colored_animation, .. }) => {
					*fire += shot.fire;
					*colored_animation = Some(ColoredAnimation {
						tp: TimeProgression::new(Duration::from_secs_f32(0.075)),
						color: Color::rgb_u8(255, 180, 0),
					});
				},
				Some(Obj::Tower { ref mut fire, ref mut colored_animation, .. }) => {
					*fire += shot.fire;
					*colored_animation = Some(ColoredAnimation {
						tp: TimeProgression::new(Duration::from_secs_f32(0.075)),
						color: Color::rgb_u8(255, 180, 0),
					});
				},
				_ => {},
			};
		}
		if shot.additional_actions > 0 {
			match self.grid.get_mut(coords).and_then(|tile| tile.obj.as_mut()) {
				Some(Obj::Enemy { ref mut actions, ref mut colored_animation, .. }) => {
					*actions += shot.additional_actions;
					*colored_animation = Some(ColoredAnimation {
						tp: TimeProgression::new(Duration::from_secs_f32(0.075)),
						color: Color::rgb_u8(255, 255, 0),
					});
				},
				Some(Obj::Tower { ref mut actions, ref mut colored_animation, .. }) => {
					*actions += shot.additional_actions;
					*colored_animation = Some(ColoredAnimation {
						tp: TimeProgression::new(Duration::from_secs_f32(0.075)),
						color: Color::rgb_u8(255, 255, 0),
					});
				},
				_ => {},
			};
		}
	}

	fn inflict_damage_to_obj_at(&mut self, coords: Coords, damages: i32) {
		let color = if damages < 0 {
			Color::rgb_u8(255, 150, 150)
		} else {
			Color::rgb_u8(255, 0, 0)
		};
		let destroy = match self.grid.get_mut(coords).and_then(|tile| tile.obj.as_mut()) {
			None => false,
			Some(Obj::Enemy { ref mut hp, ref mut colored_animation, .. }) => {
				*hp -= damages;
				*colored_animation = Some(ColoredAnimation {
					tp: TimeProgression::new(Duration::from_secs_f32(0.075)),
					color,
				});
				*hp <= 0
			},
			Some(Obj::Tower { ref mut hp, ref mut colored_animation, .. }) => {
				*hp -= damages;
				*colored_animation = Some(ColoredAnimation {
					tp: TimeProgression::new(Duration::from_secs_f32(0.075)),
					color,
				});
				*hp <= 0
			},
			Some(_) => false,
		};
		if destroy {
			self.grid.get_mut(coords).unwrap().obj = None;
		}
	}

	fn caravan_coords_and_tile(&self) -> Option<(Coords, &Tile)> {
		for coords in self.grid.dims.iter() {
			let tile = self.grid.get(coords).unwrap();
			if tile.has_caravan() {
				return Some((coords, tile));
			}
		}
		None
	}

	fn caradan_path_dist(&self) -> Option<i32> {
		self
			.caravan_coords_and_tile()
			.map(|(_coords, tile)| tile.path().unwrap().distance)
	}

	fn path_coords(&self) -> Vec<Coords> {
		let left_path_y = 'finding_left_path_y: {
			for y in 0..self.grid.dims.h {
				if self
					.grid
					.get((0, y).into())
					.unwrap()
					.path()
					.is_some_and(|path| path.distance == 0)
				{
					break 'finding_left_path_y y;
				}
			}
			panic!("No path tile with distance 0 on the left side found");
		};
		let mut path_coords = vec![];
		let mut head: Coords = (0, left_path_y).into();
		while let Some(path) = self.grid.get(head).and_then(|tile| tile.path()) {
			path_coords.push(head);
			head += path.forward;
		}
		path_coords
	}

	fn rightmost_path_y_and_dist(&self) -> Option<(i32, i32)> {
		if self.grid.dims.w == 0 {
			return None;
		}
		for y in 0..self.grid.dims.h {
			let coords: Coords = (self.grid.dims.w - 1, y).into();
			if let Ground::Path(Path { forward: CoordsDelta::RIGHT, distance, .. }) =
				self.grid.get(coords).unwrap().ground
			{
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

fn draw_obj(renderer: &mut Renderer, obj: &Obj, mut dst: Rect, disappearing: bool) {
	let mut effects = DrawSpriteEffects::none();
	if disappearing {
		effects.paint = Some(Color::rgb_u8(255, 0, 0));
	}
	match obj {
		Obj::Caravan => {
			let sprite = Rect::tile((7, 2).into(), 16);
			dst.top_left.y -= dst.dims.h * 3 / 16;
			renderer.draw_sprite(dst, sprite, effects);
		},
		Obj::Tree => {
			let mut sprite = Rect::tile((4, 2).into(), 16);
			sprite.top_left.y -= 16;
			sprite.dims.h += 16;
			dst.top_left.y -= dst.dims.h;
			dst.dims.h += dst.dims.h;
			dst.top_left.y -= dst.dims.h / 16;
			renderer.draw_sprite(dst, sprite, effects);
		},
		Obj::Rock { visual_variant } => {
			assert!(*visual_variant < 3);
			let sprite = Rect::tile((*visual_variant as i32, 2).into(), 16);
			dst.top_left.y -= dst.dims.h * 3 / 16;
			renderer.draw_sprite(dst, sprite, effects);
		},
		Obj::Crystal => {
			let mut sprite = Rect::tile((3, 2).into(), 16);
			sprite.top_left.y -= 16;
			sprite.dims.h += 16;
			dst.top_left.y -= dst.dims.h;
			dst.dims.h += dst.dims.h;
			dst.top_left.y -= dst.dims.h / 16;
			renderer.draw_sprite(dst, sprite, effects);
		},
		Obj::Enemy { actions, hp, fire, alive_animation, colored_animation, .. } => {
			let initial_dst = dst;
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
			if let Some(color) = color {
				effects.paint = Some(color);
			}
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

			// Draw fire and action counter in the back.
			if *fire >= 1 {
				let sprite = Rect::xywh(22, 17, 6, 6);
				let fire_dst = Rect {
					top_left: initial_dst.top_left + CoordsDelta::from((-4, 4)),
					dims: sprite.dims * (initial_dst.dims.w / 16),
				};
				renderer.draw_sprite(fire_dst, sprite, DrawSpriteEffects::none());
				if *fire >= 2 {
					Font {
						size_factor: 3,
						horizontal_spacing: 2,
						space_width: 7,
						foreground: Color::rgb_u8(255, 0, 0),
						background: Some(Color::BLACK),
						margins: (3, 3).into(),
					}
					.draw_text_line(
						renderer,
						&format!("{fire}"),
						(fire_dst.left(), fire_dst.top() + fire_dst.dims.h / 2).into(),
						PinPoint::CENTER_RIGHT,
					)
					.unwrap();
				}
			}
			if *actions >= 1 {
				let sprite = Rect::xywh(1, 17, 6, 6);
				let dims = sprite.dims * (initial_dst.dims.w / 16);
				let actions_dst = Rect {
					top_left: initial_dst.top_left
						+ CoordsDelta::from((-4, initial_dst.dims.h - 4 - dims.h)),
					dims: sprite.dims * (initial_dst.dims.w / 16),
				};
				renderer.draw_sprite(actions_dst, sprite, DrawSpriteEffects::none());
				if *actions >= 2 {
					Font {
						size_factor: 3,
						horizontal_spacing: 2,
						space_width: 7,
						foreground: Color::rgb_u8(255, 255, 0),
						background: Some(Color::BLACK),
						margins: (3, 3).into(),
					}
					.draw_text_line(
						renderer,
						&format!("{actions}"),
						(
							actions_dst.left(),
							actions_dst.top() + actions_dst.dims.h / 2,
						)
							.into(),
						PinPoint::CENTER_RIGHT,
					)
					.unwrap();
				}
			}
		},
		Obj::Tower { actions, fire, variant, .. } => {
			let sprite_x = match variant {
				Tower::Basic => 8,
				Tower::Pink => 9,
				Tower::Blue => 10,
			};
			let sprite = Rect::tile((sprite_x, 4).into(), 16);
			dst.top_left.y -= dst.dims.h * 2 / 16;
			renderer.draw_sprite(dst, sprite, effects);

			// Draw fire and action counter in the front.
			if *fire >= 1 {
				let sprite = Rect::xywh(22, 17, 6, 6);
				let fire_dst = Rect {
					top_left: dst.top_left + CoordsDelta::from((-4, 4)),
					dims: sprite.dims * (dst.dims.w / 16),
				};
				renderer.draw_sprite(fire_dst, sprite, DrawSpriteEffects::none());
				if *fire >= 2 {
					Font {
						size_factor: 3,
						horizontal_spacing: 2,
						space_width: 7,
						foreground: Color::rgb_u8(255, 0, 0),
						background: Some(Color::BLACK),
						margins: (3, 3).into(),
					}
					.draw_text_line(
						renderer,
						&format!("{fire}"),
						(fire_dst.left(), fire_dst.top() + fire_dst.dims.h / 2).into(),
						PinPoint::CENTER_RIGHT,
					)
					.unwrap();
				}
			}
			if *actions >= 1 {
				let sprite = Rect::xywh(1, 17, 6, 6);
				let dims = sprite.dims * (dst.dims.w / 16);
				let actions_dst = Rect {
					top_left: dst.top_left + CoordsDelta::from((-4, dst.dims.h - 4 - dims.h)),
					dims: sprite.dims * (dst.dims.w / 16),
				};
				renderer.draw_sprite(actions_dst, sprite, DrawSpriteEffects::none());
				if *actions >= 2 {
					Font {
						size_factor: 3,
						horizontal_spacing: 2,
						space_width: 7,
						foreground: Color::rgb_u8(255, 255, 0),
						background: Some(Color::BLACK),
						margins: (3, 3).into(),
					}
					.draw_text_line(
						renderer,
						&format!("{actions}"),
						(
							actions_dst.left(),
							actions_dst.top() + actions_dst.dims.h / 2,
						)
							.into(),
						PinPoint::CENTER_RIGHT,
					)
					.unwrap();
				}
			}
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
	/// Generates a new random chunk of world.
	/// The path must continue from where it stopped at the right side of the previous chunk,
	/// so we must pass that information via `last_path_y_and_dist`.
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
			let mut last_direction: CoordsDelta = (1, 0).into();
			let mut how_many_times_does_it_go_westward = 0;
			let mut distance_in_chunk = 0;
			let mut it_turned_last_tile = false;
			let mut u_turn_count = 0;
			loop {
				let possible_directions: Vec<_> = CoordsDelta::iter_4_directions()
					.filter(|direction| {
						grid
							.get(cur_head + *direction)
							.is_some_and(|tile| !tile.has_path() && tile.obj.is_none())
							|| (cur_head + *direction).x == grid.dims.w
					})
					.collect();
				if possible_directions.is_empty() {
					continue 'try_new_path;
				} else {
					let direction =
						if possible_directions.contains(&last_direction) && rand_range(0.0..1.0) < 0.05 {
							last_direction
						} else {
							possible_directions[rand_range(0..possible_directions.len())]
						};
					let backward = prev_head - cur_head;
					let forward = direction;
					grid.get_mut(cur_head).unwrap().ground =
						Ground::Path(Path { forward, backward, distance: path_dist });
					let it_turns_now =
						!((backward.dx == 0 && forward.dx == 0) || (backward.dy == 0 && forward.dy == 0));
					if it_turned_last_tile && it_turns_now {
						u_turn_count += 1;
					}
					if it_turned_last_tile {
						// Plant some trees in the corner of turns to prevent boring U-turns.
						for other_direction in CoordsDelta::iter_4_directions() {
							if other_direction != direction && rand_range(0.0..1.0) < 0.95 {
								let other_coords = cur_head + other_direction;
								if let Some(other_tile) = grid.get_mut(other_coords) {
									if other_tile.is_empty_grass() {
										other_tile.obj = Some(Obj::Tree);
									}
								}
							}
						}
					}
					it_turned_last_tile = it_turns_now;
					path_dist += 1;
					distance_in_chunk += 1;
					if direction == CoordsDelta::from((-1, 0)) {
						how_many_times_does_it_go_westward += 1;
					}
					last_direction = direction;
					prev_head = cur_head;
					cur_head += direction;
					let force_turn_probability = if cur_head.y == 0 || cur_head.y == grid.dims.h - 1 {
						0.3
					} else {
						0.1
					};
					if rand_range(0.0..1.0) < force_turn_probability {
						let other_coords = cur_head + direction;
						if let Some(other_tile) = grid.get_mut(other_coords) {
							if other_tile.is_empty_grass() {
								other_tile.obj = Some(Obj::Tree);
							}
						}
					}
					if cur_head.x == grid.dims.w {
						break;
					}
				}
			}
			if how_many_times_does_it_go_westward < 2
				|| !(14..30).contains(&distance_in_chunk)
				|| u_turn_count >= 2
			{
				continue 'try_new_path;
			}
			// Clean up the trees we planted just to help with path generation.
			for coords in grid.dims.iter() {
				grid.get_mut(coords).unwrap().obj = None;
			}
			break grid;
		};

		// Generate some water.
		while rand_range(0.0..1.0) < 0.4 {
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
		let mut crystal_count = 0;
		for _i in 0..30 {
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
						crystal_count += 1;
					}
				}
			}
			if crystal_count >= 1 {
				break;
			}
		}

		// Generate some enemies.
		for coords in grid.dims.iter() {
			let tile = grid.get_mut(coords).unwrap();
			if tile.has_path() {
				let enemy_probability = 0.4;
				if rand_range(0.0..1.0) < enemy_probability {
					tile.obj = Some(Obj::Enemy {
						actions: 0,
						hp: 8,
						fire: 0,
						alive_animation: None,
						colored_animation: None,
						variant: Enemy::Basic,
					});
				}
			}
		}

		Chunk { grid }
	}
}

/// When a shot hits its target, it may (or may not) spawn new shots fro that target
/// (for example to split in two shots that shoot on the sides, or a new shot in the same
/// direction to look like a piercing shot, etc.).
#[derive(Clone)]
enum ShotCascade {
	None,
	Piercing(Box<Shot>),
	SplitInTwo(Box<Shot>),
}

#[derive(Clone)]
struct Shot {
	damages: i32,
	fire: i32,
	additional_actions: i32,
	cascade: ShotCascade,
}

/// An `AnimationAction` is some event that happens over a period (handled by an `Animation`).
enum AnimationAction {
	Move {
		obj: Obj,
		from: Coords,
		to: Coords,
	},
	/// The camera moves on the X axis (normally only to the right).
	CameraMoveX {
		from: f32,
		to: f32,
	},
	Appear {
		obj: Obj,
		to: Coords,
	},
	Disappear {
		obj: Obj,
		from: Coords,
	},
	Shoot {
		from: Coords,
		direction: CoordsDelta,
		shot: Shot,
	},
}

struct Animation {
	action: AnimationAction,
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

struct MapDrawingConfig {
	top_left: Coords,
	/// A square tile will be drawn to a square area of side 16 * zoom.
	zoom: i32,
	/// The x coordinate (in the map's grid coordinate system) of the left side of the screen.
	camera_x: f32,
}

impl MapDrawingConfig {
	fn tile_side(&self) -> i32 {
		self.zoom * 16
	}

	fn tile_coords_to_screen_rect(&self, tile_coords: Coords) -> Rect {
		let dst_side = self.zoom * 16;
		let left = -(self.camera_x * dst_side as f32) as i32;
		Rect::xywh(
			self.top_left.x + left + dst_side * tile_coords.x,
			self.top_left.y + dst_side * tile_coords.y,
			dst_side,
			dst_side,
		)
	}

	fn screen_coords_to_tile_coords(&self, screen_coords: Coords) -> Coords {
		let dst_side = self.zoom * 16;
		let left = -(self.camera_x * dst_side as f32) as i32;
		(
			(screen_coords.x - left - self.top_left.x) / dst_side,
			(screen_coords.y - self.top_left.y) / dst_side,
		)
			.into()
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

	// Center the window.
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

	#[derive(PartialEq, Eq)]
	enum InterfaceMode {
		Normal,
		MovingCaravanChoosingDst,
		MovingCaravanAnimation { remaining_moves: i32 },
	}
	let mut interface_mode = InterfaceMode::Normal;

	let mut turn_counter = 0;
	let mut distance_traveled = 0;
	let mut crystal_amount = 20;

	let mut current_animations: Vec<Animation> = vec![];
	let mut end_player_phase_after_animation = false;
	let mut end_player_phase_right_now = false;

	let mut map_drawing_config =
		MapDrawingConfig { top_left: (0, 180).into(), zoom: 4, camera_x: 0.0 };

	let mut cursor_position = Coords::from((0, 0));
	let mut hovered_tile_coords: Option<Coords> = None;
	let mut selected_tile_coords: Option<Coords> = None;

	let mut selectable_tile_coords: Vec<Coords> = vec![];

	let mut tower_type_to_place = Tower::Basic;

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

				while map.grid.dims.w * 8 * 8
					<= (map_drawing_config.camera_x + 1.0) as i32 * 8 * 8 + renderer.dims().w + 1
				{
					map.generate_chunk_on_the_right();
				}
			},

			WindowEvent::CursorMoved { position, .. } => {
				cursor_position = (position.x.floor() as i32, position.y.floor() as i32).into();
				let coords = map_drawing_config.screen_coords_to_tile_coords(cursor_position);
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
					&& current_animations.is_empty()
					&& phase == Phase::Player
				{
					let tile = map.grid.get(selected_tile_coords.unwrap()).unwrap().clone();
					let tower_price = 10;
					if tile.obj.is_none()
						&& !tile.has_water()
						&& crystal_amount >= tower_price
						&& interface_mode == InterfaceMode::Normal
					{
						// Place a tower on empty ground.
						current_animations.push(Animation {
							action: AnimationAction::Appear {
								obj: Obj::Tower {
									actions: 0,
									hp: tower_type_to_place.initial_hp(),
									fire: 0,
									colored_animation: None,
									variant: tower_type_to_place.clone(),
								},
								to: selected_tile_coords.unwrap(),
							},
							tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
						});
						audio_player.play_sound_effect(SoundEffect::Place);
						crystal_amount -= tower_price;
						end_player_phase_after_animation = true;
					} else if matches!(tile.obj, Some(Obj::Crystal))
						&& current_animations.is_empty()
						&& interface_mode == InterfaceMode::Normal
					{
						// Mine the crystal.
						current_animations.push(Animation {
							action: AnimationAction::Disappear {
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
						audio_player.play_sound_effect(SoundEffect::Mine);
						crystal_amount += 30;
						end_player_phase_after_animation = true;
					} else if matches!(tile.obj, Some(Obj::Caravan))
						&& current_animations.is_empty()
						&& interface_mode == InterfaceMode::Normal
					{
						interface_mode = InterfaceMode::MovingCaravanChoosingDst;
						// Make selectable the tiles on which the caravan can move.
						let path_coords = map.path_coords();
						let caravan_path_dist = map.caradan_path_dist().unwrap();
						for coords in path_coords {
							let path = map.grid.get(coords).unwrap().path().unwrap();
							if path.distance <= caravan_path_dist {
								continue;
							}
							if map
								.grid
								.get(coords)
								.unwrap()
								.obj
								.as_ref()
								.is_some_and(|obj| !matches!(obj, Obj::Caravan))
							{
								break;
							}
							selectable_tile_coords.push(coords);
						}
					}
				} else if interface_mode == InterfaceMode::MovingCaravanChoosingDst
					&& hovered_tile_coords.is_some_and(|coords| selectable_tile_coords.contains(&coords))
				{
					let dst_tile = map.grid.get(hovered_tile_coords.unwrap()).unwrap().clone();
					let dst_dist = dst_tile.path().unwrap().distance;
					let src_dist = map
						.caravan_coords_and_tile()
						.unwrap()
						.1
						.path()
						.unwrap()
						.distance;
					let move_dist = dst_dist - src_dist;
					interface_mode =
						InterfaceMode::MovingCaravanAnimation { remaining_moves: move_dist };
					selectable_tile_coords.clear();
				} else if interface_mode == InterfaceMode::Normal {
					selected_tile_coords = hovered_tile_coords;
					selectable_tile_coords.clear();
				} else {
					interface_mode = InterfaceMode::Normal;
					selectable_tile_coords.clear();
				}
			},

			WindowEvent::MouseInput {
				state: ElementState::Pressed,
				button: MouseButton::Right,
				..
			} => {
				selected_tile_coords = None;
				selectable_tile_coords.clear();
			},

			WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::Space),
						..
					},
				..
			} if current_animations.is_empty() && phase == Phase::Player => {
				for coords in map.grid.dims.iter() {
					if map.grid.get(coords).is_some_and(|tile| tile.has_caravan()) {
						if let Some(Path { forward, distance, .. }) =
							map.grid.get(coords).unwrap().path().cloned()
						{
							let dst_coords = coords + forward;
							if map.grid.get(dst_coords).unwrap().obj.is_none() {
								current_animations.push(Animation {
									action: AnimationAction::Move {
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
			} if current_animations.is_empty() && phase == Phase::Player => {
				current_animations.push(Animation {
					action: AnimationAction::CameraMoveX {
						from: map_drawing_config.camera_x,
						to: map_drawing_config.camera_x + 1.0,
					},
					tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
				});
				let side = map_drawing_config.tile_side();
				while map.grid.dims.w * side
					<= (map_drawing_config.camera_x + 1.0) as i32 * side + renderer.dims().w + 1
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
			} if current_animations.is_empty() && phase == Phase::Player => {
				end_player_phase_right_now = true;
			},

			WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::T),
						..
					},
				..
			} => {
				tower_type_to_place = match tower_type_to_place {
					Tower::Basic => Tower::Pink,
					Tower::Pink => Tower::Blue,
					Tower::Blue => Tower::Basic,
				};
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

			// Trigger some enemy alive animations at random.
			for coords in map.grid.dims.iter() {
				if let Some(Obj::Enemy { alive_animation, .. }) =
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

			// Here comes the rendering of the map and interface.
			renderer.clear();

			// Drawing the ground of the tiles first so that objects can't ever appear behind ground.
			for coords in map.grid.dims.iter() {
				let dst = map_drawing_config.tile_coords_to_screen_rect(coords);
				if dst.right_excluded() < 0 || renderer.dims().w < dst.left() {
					continue;
				}
				map.draw_tile_ground_at(&mut renderer, coords, dst);
			}

			// Draw the selection/hover/selectable rectangles and related stuff.
			if let Some(coords) = hovered_tile_coords {
				if selectable_tile_coords.contains(&coords) {
					let dst = map_drawing_config
						.tile_coords_to_screen_rect(coords)
						.add_margin(1);
					renderer.draw_rect_edge(dst, Color::rgb_u8(0, 100, 255));
				} else {
					let dst = map_drawing_config.tile_coords_to_screen_rect(coords);
					renderer.draw_rect_edge(dst, Color::rgb_u8(255, 60, 0));
				}
			}
			if let Some(coords) = selected_tile_coords {
				let dst = map_drawing_config
					.tile_coords_to_screen_rect(coords)
					.add_margin(2);
				renderer.draw_rect_edge(dst, Color::rgb_u8(255, 255, 80));
			}
			for coords in selectable_tile_coords.iter() {
				if matches!(hovered_tile_coords, Some(hovered) if hovered == *coords) {
					continue;
				}
				let dst = map_drawing_config
					.tile_coords_to_screen_rect(*coords)
					.add_margin(-1);
				renderer.draw_rect_edge(dst, Color::rgb_u8(0, 100, 255));
			}

			// Now the objects that are not in animations.
			for coords in map.grid.dims.iter() {
				let dst = map_drawing_config.tile_coords_to_screen_rect(coords);
				if dst.right_excluded() < 0 || renderer.dims().w < dst.left() {
					continue;
				}
				map.draw_tile_obj_at(&mut renderer, coords, dst);
			}

			if let InterfaceMode::MovingCaravanAnimation { remaining_moves } = interface_mode {
				if current_animations.is_empty() {
					if remaining_moves <= 0 {
						interface_mode = InterfaceMode::Normal;
					} else {
						let (caravan_coords, caravan_tile) = map.caravan_coords_and_tile().unwrap();
						let distance = caravan_tile.path().unwrap().distance;
						let forward = map
							.grid
							.get(caravan_coords)
							.unwrap()
							.path()
							.unwrap()
							.forward;
						current_animations.push(Animation {
							action: AnimationAction::Move {
								obj: map
									.grid
									.get_mut(caravan_coords)
									.unwrap()
									.obj
									.take()
									.unwrap(),
								from: caravan_coords,
								to: caravan_coords + forward,
							},
							tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
						});
						distance_traveled = distance + 1;
						interface_mode =
							InterfaceMode::MovingCaravanAnimation { remaining_moves: remaining_moves - 1 };
						if remaining_moves == 1 {
							end_player_phase_after_animation = true;
						}
					}
				}
			}

			if !current_animations.is_empty() {
				let mut anim_indices_to_remove: Vec<usize> = vec![];
				let mut new_anims: Vec<Animation> = vec![];
				for (anim_index, anim) in current_animations.iter().enumerate() {
					if anim.tp.is_done() {
						let duration = anim.tp.duration;

						// The current animation is finished.
						anim_indices_to_remove.push(anim_index);
						match &anim.action {
							AnimationAction::Move { obj, to, .. } => {
								map.grid.get_mut(*to).unwrap().obj = Some(obj.clone())
							},
							AnimationAction::CameraMoveX { to, .. } => map_drawing_config.camera_x = *to,
							AnimationAction::Appear { obj, to } => {
								map.grid.get_mut(*to).unwrap().obj = Some(obj.clone())
							},
							AnimationAction::Disappear { .. } => {},
							AnimationAction::Shoot { from, direction, shot } => {
								let to = *from + *direction;
								if map.grid.dims.contains(to) {
									if map.grid.get(to).unwrap().obj.is_some() {
										map.shot_hits_obj_at(to, shot);
										audio_player.play_sound_effect(SoundEffect::Hit);
										match &shot.cascade {
											ShotCascade::None => {},
											ShotCascade::Piercing(piercing_shot) => {
												new_anims.push(Animation {
													action: AnimationAction::Shoot {
														from: to,
														direction: *direction,
														shot: *(*piercing_shot).clone(),
													},
													tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
												});
												audio_player.play_sound_effect(SoundEffect::Pew);
											},
											ShotCascade::SplitInTwo(side_shots) => {
												let one_side = CoordsDelta::from((direction.dy, direction.dx));
												new_anims.push(Animation {
													action: AnimationAction::Shoot {
														from: to,
														direction: one_side,
														shot: *(*side_shots).clone(),
													},
													tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
												});
												new_anims.push(Animation {
													action: AnimationAction::Shoot {
														from: to,
														direction: -one_side,
														shot: *(*side_shots).clone(),
													},
													tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
												});
												audio_player.play_sound_effect(SoundEffect::Pew);
											},
										}
									} else {
										new_anims.push(Animation {
											action: AnimationAction::Shoot {
												from: to,
												direction: *direction,
												shot: shot.clone(),
											},
											tp: TimeProgression { start: Instant::now(), duration },
										});
									}
								}
							},
						}
						if end_player_phase_after_animation {
							end_player_phase_after_animation = false;
							end_player_phase_right_now = false;
							selectable_tile_coords.clear();
							phase = Phase::Enemy;
							for coords in map.grid.dims.iter() {
								if let Some(Obj::Enemy { ref mut actions, .. }) =
									map.grid.get_mut(coords).unwrap().obj
								{
									*actions += 1;
								}
							}
						}
					} else {
						let progress = anim.tp.progress();
						match &anim.action {
							AnimationAction::Move { obj, from, to } => {
								let dst_from = map_drawing_config.tile_coords_to_screen_rect(*from);
								let dst_to = map_drawing_config.tile_coords_to_screen_rect(*to);
								let dst = linear_interpolation_rect(progress, dst_from, dst_to);
								draw_obj(&mut renderer, obj, dst, false);
							},
							AnimationAction::CameraMoveX { from, to } => {
								map_drawing_config.camera_x = linear_interpolation(progress, *from, *to);
								let coords =
									map_drawing_config.screen_coords_to_tile_coords(cursor_position);
								if map.grid.dims.contains(coords) {
									hovered_tile_coords = Some(coords);
								} else {
									hovered_tile_coords = None;
								}
							},
							AnimationAction::Appear { obj, to } => {
								let mut dst = map_drawing_config.tile_coords_to_screen_rect(*to);
								let side = map_drawing_config.tile_side();
								dst.top_left.x += ((side / 2) as f32 * (1.0 - progress)) as i32;
								dst.dims.w = (side as f32 * progress) as i32;
								dst.top_left.y += ((side / 2) as f32 * (1.0 - progress)) as i32;
								dst.dims.h = (side as f32 * progress) as i32;
								draw_obj(&mut renderer, obj, dst, false);
							},
							AnimationAction::Disappear { obj, from } => {
								let dst = map_drawing_config.tile_coords_to_screen_rect(*from);
								draw_obj(&mut renderer, obj, dst, true);
							},
							AnimationAction::Shoot { from, direction, .. } => {
								let to = *from + *direction;
								let dst_from = map_drawing_config.tile_coords_to_screen_rect(*from);
								let dst_to = map_drawing_config.tile_coords_to_screen_rect(to);
								let dst = linear_interpolation_rect(progress, dst_from, dst_to);
								draw_shot(&mut renderer, dst);
							},
						}
					}
				}
				for anim_index_to_remove in anim_indices_to_remove.into_iter().rev() {
					current_animations.remove(anim_index_to_remove);
				}
				current_animations.extend(new_anims);
			} else if end_player_phase_right_now {
				end_player_phase_after_animation = false;
				end_player_phase_right_now = false;
				selectable_tile_coords.clear();
				phase = Phase::Enemy;
				for coords in map.grid.dims.iter() {
					if let Some(Obj::Enemy { ref mut actions, .. }) =
						map.grid.get_mut(coords).unwrap().obj
					{
						*actions += 1;
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
						if let Some(Obj::Enemy { actions, .. }) = tile.obj {
							if actions >= 1 {
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
					}
					if let Some((_, coords)) = min_path_dist_and_coords {
						// Found the closest enemy that hasn't played yet. This enemy plays now.
						// But before playing, we handle fire effect (if any).
						if let Obj::Enemy { actions, ref mut fire, .. } =
							map.grid.get_mut(coords).unwrap().obj.as_mut().unwrap()
						{
							if *actions >= 1 && *fire >= 1 {
								*fire -= 1;
								map.inflict_damage_to_obj_at(coords, 1);
								audio_player.play_sound_effect(SoundEffect::Hit);
							}
						}
						let tile = map.grid.get_mut(coords).unwrap();
						if let Some(Obj::Enemy { ref mut actions, .. }) = tile.obj {
							if *actions >= 1 {
								// Now the enemy really plays.
								*actions -= 1;
								let backward = if let Some(Path { backward, .. }) = tile.path() {
									*backward
								} else {
									panic!("enemy not on a path")
								};
								let dst_coords = coords + backward;
								if map.grid.get(dst_coords).is_some_and(|dst_tile| {
									dst_tile.obj.is_none()
										|| dst_tile
											.obj
											.as_ref()
											.is_some_and(|obj| matches!(obj, Obj::Caravan | Obj::Tower { .. }))
								}) {
									current_animations.push(Animation {
										action: AnimationAction::Move {
											obj: map.grid.get_mut(coords).unwrap().obj.take().unwrap(),
											from: coords,
											to: dst_coords,
										},
										tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
									});
									audio_player.play_sound_effect(SoundEffect::Step);
								}
							}
						}
					} else {
						// No enemies left to play.
						// We finish some enemy buisness and get to next phase.

						// Enemy spawn
						let tile_side = map_drawing_config.tile_side();
						while map.grid.dims.w * tile_side
							<= (map_drawing_config.camera_x + 1.0) as i32 * tile_side
								+ renderer.dims().w + 1
						{
							map.generate_chunk_on_the_right();
						}
						let spawn_coords: Coords = 'spawn_coords: {
							let right =
								(map_drawing_config.camera_x + 1.0) as i32 + renderer.dims().w / tile_side;
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
							spawn_tile.obj = Some(Obj::Enemy {
								actions: 0,
								hp,
								fire: 0,
								alive_animation: None,
								colored_animation: None,
								variant: Enemy::Basic,
							});
						}

						// Get to next phase
						phase = Phase::Tower;
						for coords in map.grid.dims.iter() {
							if let Some(Obj::Tower { ref mut actions, .. }) =
								map.grid.get_mut(coords).unwrap().obj
							{
								*actions += 1;
							}
						}
					}
				} else if phase == Phase::Tower {
					// Towers gonna shoot!
					let mut found_an_tower_to_make_play = false;
					for coords in map.grid.dims.iter_left_to_right() {
						// Before playing, we handle fire effect (if any).
						if let Some(Obj::Tower { actions, ref mut fire, .. }) =
							map.grid.get_mut(coords).unwrap().obj.as_mut()
						{
							if *actions >= 1 && *fire >= 1 {
								*fire -= 1;
								map.inflict_damage_to_obj_at(coords, 1);
								audio_player.play_sound_effect(SoundEffect::Hit);
							}
						}
						let tile = map.grid.get_mut(coords).unwrap();
						if let Some(Obj::Tower { ref mut actions, ref variant, .. }) = tile.obj {
							if *actions >= 1 {
								*actions -= 1;
								let shot = variant.shot();

								// Towers will shoot at the enemy that they see that is the closest to
								// the caravan, it seems like a nice default heuristic.
								let mut min_path_dist_and_dir: Option<(i32, CoordsDelta)> = None;
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
												if min_path_dist_and_dir.is_none()
													|| min_path_dist_and_dir
														.is_some_and(|(dist_min, _)| *distance < dist_min)
												{
													min_path_dist_and_dir = Some((*distance, direction));
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

								if let Some((_, direction)) = min_path_dist_and_dir {
									// Shoot!
									// The shot here is a test for now,
									// the basic tower isn't supposed to shoot shots like these.
									current_animations.push(Animation {
										action: AnimationAction::Shoot { from: coords, direction, shot },
										tp: TimeProgression::new(Duration::from_secs_f32(0.05)),
									});
									audio_player.play_sound_effect(SoundEffect::Pew);
								}

								found_an_tower_to_make_play = true;
								break;
							}
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
					let dst = map_drawing_config.tile_coords_to_screen_rect(coords);
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
						(10, 30).into(),
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
					Rect::xywh(1, 24, 6, 6),
					DrawSpriteEffects::none(),
				);
			}

			font_white_3
				.draw_text_line(
					&mut renderer,
					&format!("turn {turn_counter}"),
					(10, 60).into(),
					PinPoint::TOP_LEFT,
				)
				.unwrap();
			font_white_3
				.draw_text_line(
					&mut renderer,
					&format!("traveled {distance_traveled} tiles"),
					(10, 80).into(),
					PinPoint::TOP_LEFT,
				)
				.unwrap();

			if phase != Phase::GameOver {
				font_white_3
					.draw_text_line(
						&mut renderer,
						match phase {
							Phase::Player => match interface_mode {
								InterfaceMode::MovingCaravanChoosingDst => {
									"player phase: moving the caravan"
								},
								_ => "player phase",
							},
							Phase::Enemy => "enemy phase",
							Phase::Tower => "tower phase",
							_ => panic!("should not be here then"),
						},
						(10, 110).into(),
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
					(10, 110).into(),
					PinPoint::TOP_LEFT,
				)
				.unwrap();
			}

			let map_bottom =
				map_drawing_config.top_left.y + map_drawing_config.tile_side() * map.grid.dims.h;

			let coords_to_display = hovered_tile_coords.or(selected_tile_coords);
			if let Some(coords) = coords_to_display {
				let tile = map.grid.get(coords).unwrap();
				let dst = Rect::xywh(10, map_bottom + 10, 8 * 8 * 2, 8 * 8 * 2);
				map.draw_tile_ground_at(&mut renderer, coords, dst);
				map.draw_tile_obj_at(&mut renderer, coords, dst);
				let obj_name = tile.obj.as_ref().map(|obj| match obj {
					Obj::Caravan => "caravan",
					Obj::Enemy { variant, .. } => match variant {
						Enemy::Basic => "basic enemy",
					},
					Obj::Rock { .. } => "rock",
					Obj::Tower { variant, .. } => match variant {
						Tower::Basic => "basic tower",
						Tower::Pink => "pink tower",
						Tower::Blue => "blue tower",
					},
					Obj::Tree => "tree",
					Obj::Crystal => "crystal",
				});
				let obj_hp = tile.obj.as_ref().and_then(|obj| obj.hp());
				let ground_name = match tile.ground {
					Ground::Grass { .. } => "grass",
					Ground::Path(_) => "path",
					Ground::Water => "water",
				};
				font_white_3
					.draw_text_line(
						&mut renderer,
						ground_name,
						(10 + 8 * 8 * 2 + 10, map_bottom + 10).into(),
						PinPoint::TOP_LEFT,
					)
					.unwrap();
				if let Some(obj_name) = obj_name {
					font_white_3
						.draw_text_line(
							&mut renderer,
							obj_name,
							(10 + 8 * 8 * 2 + 10, map_bottom + 10 + 20).into(),
							PinPoint::TOP_LEFT,
						)
						.unwrap();
				}
				if let Some(obj_hp) = obj_hp {
					font_white_3
						.draw_text_line(
							&mut renderer,
							&format!("hp: {obj_hp}"),
							(10 + 8 * 8 * 2 + 10, map_bottom + 10 + 20 * 2).into(),
							PinPoint::TOP_LEFT,
						)
						.unwrap();
				}
			}

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

			window.request_redraw();
		},

		Event::RedrawRequested(_) => {
			renderer.render();
		},

		_ => {},
	});
}
