use image::GenericImageView;

use crate::coords::*;

#[derive(Debug)]
enum CharSpriteError {
	Whitespace(char),
	Unsupported(char),
}

fn char_sprite(ch: char) -> Result<Rect, CharSpriteError> {
	const PUNCT_1: &str = "|.:!";
	const PUNCT_2: &str = ",;'[]()`";
	const PUNCT_3: &str = "_/\\%#\"^{}?*+-=@<>¨~°";
	let row_height = 5 + 1;
	if ch.is_ascii_alphabetic() {
		// First row from the bottom in the spritesheet, case insensitive, a few letters are wider.
		let ch = ch.to_ascii_lowercase();
		let mut x = (ch as i32 - 'a' as i32) * 4;
		let mut w = 3;
		for (wider_ch, how_much_wider) in [('m', 2), ('n', 1), ('q', 1), ('w', 2)] {
			use std::cmp::Ordering;
			match Ord::cmp(&ch, &wider_ch) {
				Ordering::Less => {},
				Ordering::Equal => w += how_much_wider,
				Ordering::Greater => x += how_much_wider,
			}
		}
		Ok(Rect::xywh(x, 256 - row_height, w, 5))
	} else if ch.is_ascii_digit() {
		// Second row from the bottom.
		let x = (ch as i32 - '0' as i32) * 4;
		Ok(Rect::xywh(x, 256 - row_height * 2, 3, 5))
	} else if PUNCT_3.contains(ch) {
		// Third row from the bottom, reserved for 3-pixel-wide special characters.
		let index = PUNCT_3.chars().position(|c| c == ch).unwrap() as i32;
		Ok(Rect::xywh(index * 4, 256 - row_height * 3, 3, 5))
	} else if PUNCT_1.contains(ch) {
		// Beginning of the forth row from the bottom, for 1-pixel-wide special characters.
		let index = PUNCT_1.chars().position(|c| c == ch).unwrap() as i32;
		Ok(Rect::xywh(index * 2, 256 - row_height * 4, 1, 5))
	} else if PUNCT_2.contains(ch) {
		// End of the forth row from the bottom, for 2-pixel-wide special characters.
		let index = PUNCT_2.chars().position(|c| c == ch).unwrap() as i32;
		let x = PUNCT_1.len() as i32 * 2 + index * 3;
		Ok(Rect::xywh(x, 256 - row_height * 4, 2, 5))
	} else if ch == ' ' || ch == '\n' {
		Err(CharSpriteError::Whitespace(ch))
	} else {
		Err(CharSpriteError::Unsupported(ch))
	}
}

pub struct Font {
	/// By how many times do we make the sprites bigger?
	pub size_factor: i32,
	/// The number of pixels between each character that has a sprite
	/// (note that the space character does not have a sprite).
	pub horizontal_spacing: i32,
	/// The width of space characters in pixels.
	pub space_width: i32,
	/// The color of the character sprites.
	pub foreground: Color,
	/// The background can be filled with the given color, if any.
	pub background: Option<Color>,
	/// How many margin pixels on the edges? Each axis has two margins, one on each side.
	pub margins: Dimensions,
}

#[derive(Debug)]
pub enum CharError {
	Unsupported(char),
}

impl Font {
	fn char_width(&self, ch: char) -> Result<i32, CharError> {
		match char_sprite(ch) {
			Ok(sprite) => Ok(sprite.dims.w * self.size_factor),
			Err(CharSpriteError::Whitespace(whitespace)) => {
				if whitespace == ' ' {
					Ok(self.space_width)
				} else if whitespace == '\n' {
					Ok(0)
				} else {
					unreachable!()
				}
			},
			Err(CharSpriteError::Unsupported(unsupported)) => Err(CharError::Unsupported(unsupported)),
		}
	}

	fn char_can_have_spacing_around_it(&self, ch: char) -> bool {
		ch != ' ' && ch != '\n'
	}

	fn text_line_width(&self, text: &str) -> Result<i32, CharError> {
		let mut width = 0;
		let mut last_can_have_spacing = false;
		for ch in text.chars() {
			width += self.char_width(ch)?;
			let current_can_have_spacing = self.char_can_have_spacing_around_it(ch);
			if last_can_have_spacing && current_can_have_spacing {
				width += self.horizontal_spacing;
			}
			last_can_have_spacing = current_can_have_spacing;
		}
		Ok(width)
	}

	pub fn draw_text_line(
		&self,
		renderer: &mut Renderer,
		text: &str,
		top_left: Coords,
	) -> Result<(), CharError> {
		let width = self.text_line_width(text)? + self.margins.w * 2;
		let height = 5 * self.size_factor + self.margins.h * 2;
		let dims = (width, height).into();
		if let Some(background) = self.background {
			renderer.draw_rect(Rect { top_left, dims }, background);
		}
		let mut head = top_left + self.margins.into();
		let mut last_can_have_spacing = false;
		for ch in text.chars() {
			let current_can_have_spacing = self.char_can_have_spacing_around_it(ch);
			if last_can_have_spacing && current_can_have_spacing {
				head.x += self.horizontal_spacing;
			}
			head.x += match char_sprite(ch) {
				Ok(sprite) => {
					let dst = Rect { top_left: head, dims: sprite.dims * self.size_factor };
					renderer.draw_sprite(
						dst,
						sprite,
						DrawSpriteEffects {
							flip_horizontally: false,
							flip_vertically: false,
							flip_diagonally_id: false,
							paint: Some(self.foreground),
						},
					);
					dst.dims.w
				},
				Err(CharSpriteError::Whitespace(' ')) => self.space_width,
				Err(CharSpriteError::Whitespace(_)) => todo!(),
				Err(CharSpriteError::Unsupported(unsupported)) => {
					return Err(CharError::Unsupported(unsupported));
				},
			};
			last_can_have_spacing = current_can_have_spacing;
		}
		Ok(())
	}
}

pub struct Renderer {
	pix_buf: pixels::Pixels,
	pix_buf_dims: Dimensions,
	spritesheet: image::DynamicImage,
	clear_color: Color,
}

impl Renderer {
	pub fn new(window: &winit::window::Window, clear_color: Color) -> Renderer {
		let clear_color_wgpu = {
			fn conv_srgb_to_linear(x: f64) -> f64 {
				// See https://github.com/gfx-rs/wgpu/issues/2326
				// Stolen from https://github.com/three-rs/three/blob/07e47da5e0673aa9a16526719e16debd59040eec/src/color.rs#L42
				// (licensed MIT, not a substancial portion so not concerned by license obligations)
				// Basically the brightness is adjusted somewhere by wgpu or something due to sRGB stuff,
				// color is hard.
				if x > 0.04045 {
					((x + 0.055) / 1.055).powf(2.4)
				} else {
					x / 12.92
				}
			}
			pixels::wgpu::Color {
				r: conv_srgb_to_linear(clear_color.r() as f64 / 255.0),
				g: conv_srgb_to_linear(clear_color.g() as f64 / 255.0),
				b: conv_srgb_to_linear(clear_color.b() as f64 / 255.0),
				a: conv_srgb_to_linear(clear_color.a() as f64 / 255.0),
			}
		};

		let pix_buf_dims: Dimensions = window.inner_size().into();
		let pix_buf = {
			let size = pix_buf_dims;
			let surface_texture = pixels::SurfaceTexture::new(size.w as u32, size.h as u32, &window);
			pixels::PixelsBuilder::new(size.w as u32, size.h as u32, surface_texture)
				.clear_color(clear_color_wgpu)
				.build()
				.unwrap()
		};

		let spritesheet =
			image::load_from_memory(include_bytes!("../assets/spritesheet.png")).unwrap();

		Renderer { pix_buf, pix_buf_dims, spritesheet, clear_color }
	}

	pub fn clear(&mut self) {
		self
			.pix_buf
			.frame_mut()
			.chunks_exact_mut(4)
			.for_each(|pixel| pixel.copy_from_slice(&self.clear_color.raw()));
	}

	pub fn render(&mut self) {
		self.pix_buf.render().unwrap();
	}

	pub fn resized(&mut self, new_dims: Dimensions) {
		self
			.pix_buf
			.resize_surface(new_dims.w as u32, new_dims.h as u32)
			.unwrap();
		self
			.pix_buf
			.resize_buffer(new_dims.w as u32, new_dims.h as u32)
			.unwrap();
		self.pix_buf_dims = new_dims;
	}

	pub fn dims(&self) -> Dimensions {
		self.pix_buf_dims
	}

	/// Draw a rect from the spritesheet onto a rect in the pixel buffer.
	/// The `paint` argument, if some, will paint all the non-transparent pixels to the given color.
	pub fn draw_sprite(&mut self, dst: Rect, sprite: Rect, effects: DrawSpriteEffects) {
		// `coords_dst_dims` is a pixel in the dst rect but with (0, 0) being the top left corner.
		for coords_dst_dims in dst.dims.iter() {
			// `(sx, sy)` is the pixel to read from the spritesheet.
			let (cddx, cddy) = if effects.flip_diagonally_id {
				(coords_dst_dims.y, coords_dst_dims.x)
			} else {
				(coords_dst_dims.x, coords_dst_dims.y)
			};
			let sx = if effects.flip_horizontally {
				(sprite.top_left.x + sprite.dims.w - 1 - cddx * sprite.dims.w / dst.dims.w) as u32
			} else {
				(sprite.top_left.x + cddx * sprite.dims.w / dst.dims.w) as u32
			};
			let sy = if effects.flip_vertically {
				(sprite.top_left.y + sprite.dims.h - 1 - cddy * sprite.dims.h / dst.dims.h) as u32
			} else {
				(sprite.top_left.y + cddy * sprite.dims.h / dst.dims.h) as u32
			};

			let color = self.spritesheet.get_pixel(sx, sy).0;
			// Skip transparent pixels.
			if color[3] == 0 {
				continue;
			}
			let color = effects.paint.map(Color::raw).unwrap_or(color);

			// `coords_pixel_buffer` is the pixel to write to in the pixel buffer,
			// each of which is visited once.
			let coords_pixel_buffer = coords_dst_dims + dst.top_left.into();
			if let Some(pixel_index) = self.pix_buf_dims.index_of_coords(coords_pixel_buffer) {
				let pixel_byte_index = pixel_index * 4;
				let pixel_bytes = pixel_byte_index..(pixel_byte_index + 4);
				self.pix_buf.frame_mut()[pixel_bytes].copy_from_slice(&color);
			}
		}
	}

	pub fn draw_rect(&mut self, dst: Rect, color: Color) {
		for coords in dst.iter() {
			if let Some(pixel_index) = self.pix_buf_dims.index_of_coords(coords) {
				let pixel_byte_index = pixel_index * 4;
				let pixel_bytes = pixel_byte_index..(pixel_byte_index + 4);
				self.pix_buf.frame_mut()[pixel_bytes].copy_from_slice(&color.raw());
			}
		}
	}

	pub fn draw_rect_edge(&mut self, dst: Rect, color: Color) {
		let dst_inside = Rect {
			top_left: dst.top_left + (2, 2).into(),
			dims: dst.dims - (4, 4).into(),
		};
		for coords in dst.iter() {
			if !dst_inside.contains(coords) {
				if let Some(pixel_index) = self.pix_buf_dims.index_of_coords(coords) {
					let pixel_byte_index = pixel_index * 4;
					let pixel_bytes = pixel_byte_index..(pixel_byte_index + 4);
					self.pix_buf.frame_mut()[pixel_bytes].copy_from_slice(&color.raw());
				}
			}
		}
	}
}

pub struct DrawSpriteEffects {
	pub flip_horizontally: bool,
	pub flip_vertically: bool,
	pub flip_diagonally_id: bool,
	pub paint: Option<Color>,
}
impl DrawSpriteEffects {
	pub fn none() -> DrawSpriteEffects {
		DrawSpriteEffects {
			flip_horizontally: false,
			flip_vertically: false,
			flip_diagonally_id: false,
			paint: None,
		}
	}
}

#[derive(Clone, Copy)]
pub struct Color {
	rgba: [u8; 4],
}

impl Color {
	pub const BLACK: Color = Color { rgba: [0, 0, 0, 255] };
	pub const WHITE: Color = Color { rgba: [255, 255, 255, 255] };

	pub fn rgba_u8(r: u8, g: u8, b: u8, a: u8) -> Color {
		Color { rgba: [r, g, b, a] }
	}
	pub fn rgb_u8(r: u8, g: u8, b: u8) -> Color {
		Color::rgba_u8(r, g, b, 255)
	}

	pub fn raw(self) -> [u8; 4] {
		self.rgba
	}
}
// Code factorization be like
macro_rules! channel {
	( $c:ident, $i:expr ) => {
		pub fn $c(self) -> u8 {
			self.rgba[$i]
		}
	};
}
impl Color {
	channel!(r, 0);
	channel!(g, 1);
	channel!(b, 2);
	channel!(a, 3);
}
