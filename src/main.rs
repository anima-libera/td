mod coords;
mod renderer;
use rand::Rng;

use crate::coords::*;
use crate::renderer::*;

#[derive(Clone)]
enum Ground {
	Grass { visual_variant: u8 },
	Path,
	Water,
}

struct Tile {
	ground: Ground,
}

struct Map {
	grid: Grid<Tile>,
}

impl Map {
	fn draw_tile_ground_at(&self, renderer: &mut Renderer, coords: Coords, dst: Rect) {
		let ground = self.grid.get(coords).unwrap().ground.clone();
		match ground {
			Ground::Grass { visual_variant } => {
				let sprite = Rect::tile((visual_variant as i32, 0).into(), 16);
				renderer.draw_sprite(dst, sprite, false, None);
			},
			Ground::Path => {
				let sprite = Rect::tile((4, 0).into(), 16);
				renderer.draw_sprite(dst, sprite, false, None);
			},
			Ground::Water => {
				let sprite = Rect::tile((6, 0).into(), 16);
				renderer.draw_sprite(dst, sprite, false, None);
			},
		}
	}
}

fn main() {
	env_logger::init();
	let event_loop = winit::event_loop::EventLoop::new();
	let window = winit::window::WindowBuilder::new()
		.with_title("Defend the caravan")
		.with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
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

	let map = Map {
		grid: Grid::new((20, 10).into(), |_coords: Coords| Tile {
			ground: Ground::Grass { visual_variant: rand::thread_rng().gen_range(0..4) },
		}),
	};

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
