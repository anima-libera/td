mod coords;
mod renderer;
use crate::coords::*;
use crate::renderer::*;

#[derive(Clone)]
enum AllyType {
	Mage,
	Pyro,
	Fighter,
	Pink,
	Green,
	Chemist,
	Medic,
}
#[derive(Clone)]
enum EnemyType {
	Blob,
	Snake,
	Monster,
	Elec,
	Ears,
	Worm,
	Orb,
}
#[derive(Clone)]
enum NatureType {
	Tree,
	Grass,
	Rock,
}

#[derive(Clone)]
enum EntType {
	Ally(AllyType),
	Enemy(EnemyType),
	Nature(NatureType),
}

impl EntType {
	fn sprite_coords(&self) -> (i32, i32) {
		match self {
			EntType::Ally(AllyType::Mage) => (0, 0),
			EntType::Ally(AllyType::Pyro) => (1, 0),
			EntType::Ally(AllyType::Fighter) => (2, 0),
			EntType::Ally(AllyType::Pink) => (3, 0),
			EntType::Ally(AllyType::Green) => (4, 0),
			EntType::Ally(AllyType::Chemist) => (5, 0),
			EntType::Ally(AllyType::Medic) => (6, 0),
			EntType::Enemy(EnemyType::Blob) => (0, 1),
			EntType::Enemy(EnemyType::Snake) => (1, 1),
			EntType::Enemy(EnemyType::Monster) => (2, 1),
			EntType::Enemy(EnemyType::Elec) => (3, 1),
			EntType::Enemy(EnemyType::Ears) => (4, 1),
			EntType::Enemy(EnemyType::Worm) => (5, 1),
			EntType::Enemy(EnemyType::Orb) => (6, 1),
			EntType::Nature(NatureType::Tree) => (0, 3),
			EntType::Nature(NatureType::Grass) => (1, 3),
			EntType::Nature(NatureType::Rock) => (2, 3),
		}
	}
}

struct Life {
	max: u32,
	current: u32,
}

struct Ent {
	type_: EntType,
	life: Option<Life>,
}

fn main() {
	env_logger::init();
	let event_loop = winit::event_loop::EventLoop::new();
	let window = winit::window::WindowBuilder::new()
		.with_title("()Oo0°")
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

	let mut line: Vec<Option<Ent>> = vec![];
	for _i in 0..10 {
		line.push(None);
	}
	line[0] = Some(Ent { life: None, type_: EntType::Ally(AllyType::Pink) });
	line[9] = Some(Ent { life: None, type_: EntType::Enemy(EnemyType::Blob) });

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

			for (i, ent) in line.iter().enumerate() {
				if let Some(ent) = ent {
					let sprite_coords = ent.type_.sprite_coords();
					renderer.draw_sprite(
						Rect::xywh(8 * 8 * i as i32, 100, 8 * 8, 8 * 8),
						Rect::tile(sprite_coords.into(), 8),
						false,
						None,
					);
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
