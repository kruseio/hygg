use image::imageops::{self, FilterType};
use image::{DynamicImage, GenericImageView, Pixel, RgbaImage};

const RESET: &str = "\x1b[0m";
const UPPER_HALF_BLOCK: char = '▀';

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderConfig {
  pub width: Option<u32>,
  pub height: Option<u32>,
}

impl RenderConfig {
  pub const fn new(width: Option<u32>, height: Option<u32>) -> Self {
    Self { width, height }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Color {
  r: u8,
  g: u8,
  b: u8,
}

pub fn render_half_block(
  image: &DynamicImage,
  config: RenderConfig,
) -> Vec<String> {
  let Some((target_width, target_pixel_height)) =
    target_dimensions(image.dimensions(), config)
  else {
    return Vec::new();
  };

  let rgba = resize_rgba(image, target_width, target_pixel_height);
  let line_count = target_pixel_height.div_ceil(2);
  let mut lines = Vec::with_capacity(line_count as usize);

  for row in 0..line_count {
    let top_y = row * 2;
    let bottom_y = top_y + 1;
    let mut line = String::new();

    for x in 0..target_width {
      let top = composite_over_white(rgba.get_pixel(x, top_y));
      let bottom = if bottom_y < target_pixel_height {
        composite_over_white(rgba.get_pixel(x, bottom_y))
      } else {
        Color { r: 255, g: 255, b: 255 }
      };

      line.push_str(&format!(
        "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m{}",
        top.r, top.g, top.b, bottom.r, bottom.g, bottom.b, UPPER_HALF_BLOCK
      ));
    }

    line.push_str(RESET);
    lines.push(line);
  }

  lines
}

fn resize_rgba(
  image: &DynamicImage,
  target_width: u32,
  target_pixel_height: u32,
) -> RgbaImage {
  let rgba = flatten_over_white(image);
  if rgba.dimensions() == (target_width, target_pixel_height) {
    return rgba;
  }

  imageops::resize(
    &rgba,
    target_width,
    target_pixel_height,
    FilterType::Lanczos3,
  )
}

fn flatten_over_white(image: &DynamicImage) -> RgbaImage {
  let mut rgba = image.to_rgba8();
  for pixel in rgba.pixels_mut() {
    let color = composite_over_white(pixel);
    *pixel = image::Rgba([color.r, color.g, color.b, 255]);
  }
  rgba
}

fn target_dimensions(
  source_dimensions: (u32, u32),
  config: RenderConfig,
) -> Option<(u32, u32)> {
  let (source_width, source_height) = source_dimensions;
  if source_width == 0 || source_height == 0 {
    return None;
  }

  let target_width = config.width.unwrap_or_else(|| {
    config
      .height
      .map(|height| {
        scale_dimension(source_width, height.saturating_mul(2), source_height)
      })
      .unwrap_or(source_width)
  });

  let target_pixel_height =
    config.height.map(|height| height.saturating_mul(2)).unwrap_or_else(|| {
      config
        .width
        .map(|width| scale_dimension(source_height, width, source_width))
        .unwrap_or(source_height)
    });

  if target_width == 0 || target_pixel_height == 0 {
    return None;
  }

  Some((target_width, target_pixel_height))
}

fn scale_dimension(
  source_dimension: u32,
  target_axis: u32,
  source_axis: u32,
) -> u32 {
  if target_axis == 0 || source_axis == 0 {
    return 0;
  }

  let scaled = (u64::from(source_dimension) * u64::from(target_axis)
    + u64::from(source_axis / 2))
    / u64::from(source_axis);

  scaled.max(1).min(u64::from(u32::MAX)) as u32
}

fn composite_over_white(pixel: &image::Rgba<u8>) -> Color {
  let channels = pixel.channels();
  let alpha = u32::from(channels[3]);
  if alpha == 255 {
    return Color { r: channels[0], g: channels[1], b: channels[2] };
  }
  if alpha == 0 {
    return Color { r: 255, g: 255, b: 255 };
  }

  let inv_alpha = 255 - alpha;
  let blend = |channel: u8| -> u8 {
    ((u32::from(channel) * alpha + 255 * inv_alpha + 127) / 255) as u8
  };

  Color { r: blend(channels[0]), g: blend(channels[1]), b: blend(channels[2]) }
}

#[cfg(test)]
mod tests {
  use super::{RenderConfig, UPPER_HALF_BLOCK, render_half_block};
  use image::{DynamicImage, Rgba, RgbaImage};

  fn rgba_image(width: u32, height: u32, pixels: &[[u8; 4]]) -> DynamicImage {
    let mut image = RgbaImage::new(width, height);
    for (index, pixel) in pixels.iter().enumerate() {
      let x = index as u32 % width;
      let y = index as u32 / width;
      image.put_pixel(x, y, Rgba(*pixel));
    }
    DynamicImage::ImageRgba8(image)
  }

  fn block_count(line: &str) -> usize {
    line.chars().filter(|&ch| ch == UPPER_HALF_BLOCK).count()
  }

  #[test]
  fn renders_one_half_block_with_truecolor_foreground_and_background() {
    let image = rgba_image(1, 2, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
    let lines = render_half_block(&image, RenderConfig::default());

    assert_eq!(lines.len(), 1);
    assert_eq!(block_count(&lines[0]), 1);
    assert_eq!(lines[0], "\x1b[38;2;255;0;0m\x1b[48;2;0;0;255m▀\x1b[0m");
  }

  #[test]
  fn alpha_composites_pixels_over_white() {
    let image = rgba_image(1, 2, &[[255, 0, 0, 128], [0, 0, 0, 0]]);
    let lines = render_half_block(&image, RenderConfig::default());

    assert_eq!(
      lines[0],
      "\x1b[38;2;255;127;127m\x1b[48;2;255;255;255m▀\x1b[0m"
    );
  }

  #[test]
  fn pads_odd_height_images_with_white_lower_half() {
    let image = rgba_image(
      1,
      3,
      &[[10, 20, 30, 255], [40, 50, 60, 255], [70, 80, 90, 255]],
    );
    let lines = render_half_block(&image, RenderConfig::default());

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1], "\x1b[38;2;70;80;90m\x1b[48;2;255;255;255m▀\x1b[0m");
  }

  #[test]
  fn respects_explicit_terminal_width_and_height() {
    let image = rgba_image(
      2,
      4,
      &[
        [0, 0, 0, 255],
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [0, 255, 255, 255],
        [255, 0, 255, 255],
        [255, 255, 255, 255],
      ],
    );

    let lines = render_half_block(&image, RenderConfig::new(Some(1), Some(1)));

    assert_eq!(lines.len(), 1);
    assert_eq!(block_count(&lines[0]), 1);
    assert!(lines[0].ends_with("\x1b[0m"));
  }

  #[test]
  fn preserves_aspect_ratio_when_only_width_is_configured() {
    let image = rgba_image(
      2,
      4,
      &[
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
      ],
    );

    let lines = render_half_block(&image, RenderConfig::new(Some(1), None));

    assert_eq!(lines.len(), 1);
    assert_eq!(block_count(&lines[0]), 1);
  }

  #[test]
  fn zero_sized_config_returns_no_lines() {
    let image = rgba_image(1, 1, &[[0, 0, 0, 255]]);

    assert!(
      render_half_block(&image, RenderConfig::new(Some(0), None)).is_empty()
    );
    assert!(
      render_half_block(&image, RenderConfig::new(None, Some(0))).is_empty()
    );
  }
}
