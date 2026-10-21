//! Writes the Apple `sbix` table for colour glyphs.
//!
//! `sbix` holds one PNG per glyph and pixel size. Each colour glyph is drawn at
//! every strike size from its paint graph: palette 0 supplies the colours, the
//! text foreground is black, and gradients keep their font-unit geometry. A
//! radial gradient is drawn from its focal point to its outer circle, so an
//! inner radius is not reproduced.

use shift_font::{ColorData, GlyphId, Paint, PaintColor, Rgba};
use skrifa::{
    FontRef as SkrifaFont, MetadataProvider,
    outline::{DrawSettings, OutlinePen},
    prelude::{LocationRef, Size},
};
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint as SkiaPaint, PathBuilder, Pixmap, Point,
    RadialGradient, Shader, SpreadMode, Transform,
};
use write_fonts::read::{FontRef, TableProvider};
use write_fonts::types::GlyphId16;

use crate::colr::{ColorError, CompiledGlyphs, ResolvedLayer, farthest_corner, layers_of};
use crate::svg::linear_end;

/// Pixel sizes of the strikes, smallest first.
const STRIKE_PPEMS: [u16; 3] = [32, 64, 128];

/// Resolution that a strike declares for its pixel size.
const STRIKE_PPI: u16 = 72;

/// Transparent pixels kept around a bitmap so that anti-aliasing is not clipped.
const PADDING: i32 = 1;

/// `sbix` flags: bit 0 must be set.
const FLAGS: u16 = 1;

/// One colour glyph to draw at every strike size.
struct BitmapGlyph<'a> {
    base: GlyphId16,
    layers: Vec<ResolvedLayer<'a>>,
    /// Union of the layer bounds as `[x_min, y_min, x_max, y_max]` in font units.
    bounds: [i16; 4],
}

/// A drawn glyph with the offset of its lower-left corner from the glyph origin.
struct Bitmap {
    png: Vec<u8>,
    origin_x: i16,
    origin_y: i16,
}

/// Returns the `sbix` table for the colour glyphs of `color`.
///
/// Returns `None` when no colour glyph has an outline to draw, so the font keeps
/// no empty table.
///
/// # Errors
///
/// Returns [`ColorError`] when a paint names a glyph missing from the compiled
/// font, the font cannot be read, a bitmap cannot be encoded, or the table
/// overflows its offsets.
pub(crate) fn sbix_table(
    ttf: &[u8],
    color: &ColorData,
    resolve: &impl Fn(&GlyphId) -> Result<GlyphId16, ColorError>,
    compiled: &CompiledGlyphs<'_>,
) -> Result<Option<Vec<u8>>, ColorError> {
    let read_error = |error: &dyn std::fmt::Display| ColorError::ReadFont {
        message: error.to_string(),
    };
    let font = FontRef::new(ttf).map_err(|error| read_error(&error))?;
    let units_per_em = font
        .head()
        .map_err(|error| read_error(&error))?
        .units_per_em();
    let glyph_count = font
        .maxp()
        .map_err(|error| read_error(&error))?
        .num_glyphs();
    let drawn = SkrifaFont::new(ttf).map_err(|error| read_error(&error))?;
    let outlines = drawn.outline_glyphs();

    let mut glyphs = Vec::new();
    for entry in &color.glyphs {
        let layers = layers_of(&entry.paint, resolve)?;
        let bounds = layers
            .iter()
            .filter_map(|layer| compiled.bounds(layer.glyph))
            .reduce(|a, b| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            });
        let Some(bounds) = bounds else {
            continue;
        };
        glyphs.push(BitmapGlyph {
            base: resolve(&entry.glyph)?,
            layers,
            bounds,
        });
    }
    if glyphs.is_empty() {
        return Ok(None);
    }
    glyphs.sort_by_key(|glyph| glyph.base);

    let overflow = || ColorError::TooLarge {
        message: "the sbix bitmaps exceed the table offsets".to_string(),
    };
    let strike_count = STRIKE_PPEMS.len();
    let first_strike = 8 + 4 * strike_count;
    let mut strikes: Vec<Vec<u8>> = Vec::with_capacity(strike_count);
    for ppem in STRIKE_PPEMS {
        let renderer = Renderer {
            scale: f64::from(ppem) / f64::from(units_per_em),
            palette: color
                .palettes
                .first()
                .map_or(&[][..], |p| p.colors.as_slice()),
            compiled,
            outlines: &outlines,
        };
        strikes.push(strike(ppem, glyph_count, &glyphs, &renderer)?);
    }

    let mut bytes = Vec::new();
    bytes.extend(1u16.to_be_bytes());
    bytes.extend(FLAGS.to_be_bytes());
    bytes.extend(
        u32::try_from(strike_count)
            .map_err(|_| overflow())?
            .to_be_bytes(),
    );
    let mut offset = first_strike;
    for strike in &strikes {
        bytes.extend(u32::try_from(offset).map_err(|_| overflow())?.to_be_bytes());
        offset = offset.checked_add(strike.len()).ok_or_else(overflow)?;
    }
    for strike in strikes {
        bytes.extend(strike);
    }
    Ok(Some(bytes))
}

/// Serialises one strike, whose glyphs without a bitmap have empty records.
fn strike(
    ppem: u16,
    glyph_count: u16,
    glyphs: &[BitmapGlyph<'_>],
    renderer: &Renderer<'_, '_>,
) -> Result<Vec<u8>, ColorError> {
    let overflow = || ColorError::TooLarge {
        message: "the sbix bitmaps exceed the table offsets".to_string(),
    };
    let header_len = 4 + 4 * (usize::from(glyph_count) + 1);
    let mut offsets: Vec<u32> = Vec::with_capacity(usize::from(glyph_count) + 1);
    let mut records: Vec<u8> = Vec::new();
    let mut pending = glyphs.iter().peekable();

    for glyph in 0..glyph_count {
        offsets.push(u32::try_from(header_len + records.len()).map_err(|_| overflow())?);
        let Some(next) = pending.next_if(|next| next.base.to_u16() == glyph) else {
            continue;
        };
        let Some(bitmap) = renderer.render(next, ppem)? else {
            continue;
        };
        records.extend(bitmap.origin_x.to_be_bytes());
        records.extend(bitmap.origin_y.to_be_bytes());
        records.extend(b"png ");
        records.extend(bitmap.png);
    }
    offsets.push(u32::try_from(header_len + records.len()).map_err(|_| overflow())?);

    let mut bytes = Vec::with_capacity(header_len + records.len());
    bytes.extend(ppem.to_be_bytes());
    bytes.extend(STRIKE_PPI.to_be_bytes());
    for offset in offsets {
        bytes.extend(offset.to_be_bytes());
    }
    bytes.extend(records);
    Ok(bytes)
}

/// Draws colour glyphs at one pixel size.
struct Renderer<'a, 'f> {
    /// Pixels per font unit.
    scale: f64,
    /// Colours of palette 0.
    palette: &'a [Rgba],
    compiled: &'a CompiledGlyphs<'f>,
    outlines: &'a skrifa::outline::OutlineGlyphCollection<'a>,
}

impl Renderer<'_, '_> {
    /// Draws the layers of a glyph, or returns `None` when it has no pixels.
    fn render(&self, glyph: &BitmapGlyph<'_>, ppem: u16) -> Result<Option<Bitmap>, ColorError> {
        let [x_min, y_min, x_max, y_max] = glyph.bounds.map(|value| f64::from(value) * self.scale);
        let left = x_min.floor() as i32 - PADDING;
        let bottom = y_min.floor() as i32 - PADDING;
        let right = x_max.ceil() as i32 + PADDING;
        let top = y_max.ceil() as i32 + PADDING;
        let (Ok(width), Ok(height)) = (u32::try_from(right - left), u32::try_from(top - bottom))
        else {
            return Ok(None);
        };
        let Some(mut pixmap) = Pixmap::new(width, height) else {
            return Ok(None);
        };
        let (Ok(origin_x), Ok(origin_y)) = (i16::try_from(left), i16::try_from(bottom)) else {
            return Err(ColorError::TooLarge {
                message: format!("a bitmap at {ppem} pixels per em lies outside the glyph origin"),
            });
        };

        // Font units to pixels, with the y axis flipped
        let scale = self.scale as f32;
        let transform = Transform::from_row(scale, 0.0, 0.0, -scale, -left as f32, top as f32);
        for layer in &glyph.layers {
            self.draw_layer(&mut pixmap, layer, transform)?;
        }
        let png = pixmap.encode_png().map_err(|error| ColorError::Rebuild {
            message: format!("cannot encode a bitmap: {error}"),
        })?;
        Ok(Some(Bitmap {
            png,
            origin_x,
            origin_y,
        }))
    }

    fn draw_layer(
        &self,
        pixmap: &mut Pixmap,
        layer: &ResolvedLayer<'_>,
        transform: Transform,
    ) -> Result<(), ColorError> {
        let mut pen = SkiaPen::default();
        let Some(outline) = self
            .outlines
            .get(skrifa::GlyphId::new(u32::from(layer.glyph.to_u16())))
        else {
            return Ok(());
        };
        outline
            .draw(
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                &mut pen,
            )
            .map_err(|error| ColorError::ReadFont {
                message: error.to_string(),
            })?;
        let Some(path) = pen.0.finish() else {
            return Ok(());
        };
        let Some(shader) = self.shader(layer.fill, self.compiled.bounds(layer.glyph)) else {
            return Ok(());
        };
        let paint = SkiaPaint {
            shader,
            anti_alias: true,
            ..SkiaPaint::default()
        };
        pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
        Ok(())
    }

    /// Builds the shader of a fill, in font units, or `None` when it draws nothing.
    fn shader(&self, fill: &Paint, bounds: Option<[i16; 4]>) -> Option<Shader<'static>> {
        match fill {
            Paint::Solid { color, alpha } => Some(Shader::SolidColor(self.colour(*color, *alpha)?)),
            Paint::LinearGradient { stops, p0, p1, p2 } => {
                let end = linear_end(*p0, *p1, *p2);
                LinearGradient::new(
                    point(p0.x, p0.y),
                    point(end.x, end.y),
                    self.stops(stops)?,
                    SpreadMode::Pad,
                    Transform::identity(),
                )
            }
            Paint::RadialGradient {
                stops, c0, c1, r1, ..
            } => {
                let outer = r1.unwrap_or_else(|| farthest_corner(*c1, bounds));
                RadialGradient::new(
                    point(c0.x, c0.y),
                    point(c1.x, c1.y),
                    outer as f32,
                    self.stops(stops)?,
                    SpreadMode::Pad,
                    Transform::identity(),
                )
            }
            Paint::Glyph { .. } | Paint::Layers { .. } => None,
        }
    }

    fn stops(&self, stops: &[shift_font::ColorStop]) -> Option<Vec<GradientStop>> {
        stops
            .iter()
            .map(|stop| {
                Some(GradientStop::new(
                    stop.offset as f32,
                    self.colour(stop.color, stop.alpha)?,
                ))
            })
            .collect()
    }

    /// Returns a paint colour from palette 0, with `alpha` multiplied into it.
    fn colour(&self, color: PaintColor, alpha: f64) -> Option<Color> {
        let rgba = match color {
            PaintColor::Foreground => Rgba::new(0, 0, 0, 255),
            PaintColor::Palette { index } => self
                .palette
                .get(usize::from(index))
                .copied()
                .unwrap_or(Rgba::new(0, 0, 0, 255)),
        };
        let opacity = f64::from(rgba.a) / 255.0 * alpha.clamp(0.0, 1.0);
        Color::from_rgba(
            f32::from(rgba.r) / 255.0,
            f32::from(rgba.g) / 255.0,
            f32::from(rgba.b) / 255.0,
            opacity as f32,
        )
    }
}

fn point(x: f64, y: f64) -> Point {
    Point::from_xy(x as f32, y as f32)
}

/// Collects outline commands as a `tiny-skia` path.
#[derive(Default)]
struct SkiaPen(PathBuilder);

impl OutlinePen for SkiaPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.0.quad_to(cx, cy, x, y);
    }

    fn curve_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.0.cubic_to(c1x, c1y, c2x, c2y, x, y);
    }

    fn close(&mut self) {
        self.0.close();
    }
}
