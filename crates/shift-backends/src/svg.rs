//! Writes the OpenType `SVG ` table for colour glyphs.
//!
//! Each colour glyph becomes one uncompressed SVG document that holds a
//! `<g id="glyphN">` element. Palette entries are written as `var(--colorN)`
//! with the first palette's colour as the fallback, which is how a renderer
//! substitutes the palette that the application selects. The text foreground is
//! `context-fill`. Outlines are the compiled glyphs at the default location,
//! and a nested group flips them from the font's upward y axis to SVG's
//! downward one, so gradient geometry keeps its font coordinates.

use std::fmt::Write as _;

use shift_font::{ColorData, ColorPoint, ColorStop, GlyphId, Paint, PaintColor, Rgba};
use skrifa::{
    FontRef as SkrifaFont, MetadataProvider,
    outline::{DrawSettings, OutlinePen},
    prelude::{LocationRef, Size},
};
use write_fonts::types::GlyphId16;

use crate::colr::{ColorError, CompiledGlyphs, ResolvedLayer, farthest_corner, layers_of};

/// Size of the `SVG ` header: version, document list offset and a reserved word.
const HEADER_LEN: usize = 10;

/// Size of one document record: first glyph, last glyph, offset and length.
const RECORD_LEN: usize = 12;

/// Returns the `SVG ` table for the colour glyphs of `color`.
///
/// `ttf` is the compiled font that supplies outlines. Returns `None` when no
/// colour glyph has an outline to draw, so the font keeps no empty table.
///
/// # Errors
///
/// Returns [`ColorError`] when a paint names a glyph missing from the compiled
/// font, the font cannot be read, or the table overflows its offsets.
pub(crate) fn svg_table(
    ttf: &[u8],
    color: &ColorData,
    resolve: &impl Fn(&GlyphId) -> Result<GlyphId16, ColorError>,
    compiled: &CompiledGlyphs<'_>,
) -> Result<Option<Vec<u8>>, ColorError> {
    let font = SkrifaFont::new(ttf).map_err(|error| ColorError::ReadFont {
        message: error.to_string(),
    })?;
    let outlines = font.outline_glyphs();

    let mut documents: Vec<(GlyphId16, String)> = Vec::new();
    for entry in &color.glyphs {
        let base = resolve(&entry.glyph)?;
        let layers = layers_of(&entry.paint, resolve)?;
        let document = Document {
            base,
            color,
            compiled,
        };
        let Some(text) = document.render(&layers, |glyph| path_data(&outlines, glyph))? else {
            continue;
        };
        documents.push((base, text));
    }
    if documents.is_empty() {
        return Ok(None);
    }
    documents.sort_by_key(|(glyph, _)| *glyph);

    let overflow = || ColorError::TooLarge {
        message: "the SVG documents exceed the table offsets".to_string(),
    };
    let list_len = 2 + RECORD_LEN * documents.len();
    let mut offset = list_len;
    let mut list = Vec::with_capacity(list_len);
    list.extend(
        u16::try_from(documents.len())
            .map_err(|_| overflow())?
            .to_be_bytes(),
    );
    for (glyph, text) in &documents {
        list.extend(glyph.to_u16().to_be_bytes());
        list.extend(glyph.to_u16().to_be_bytes());
        list.extend(u32::try_from(offset).map_err(|_| overflow())?.to_be_bytes());
        list.extend(
            u32::try_from(text.len())
                .map_err(|_| overflow())?
                .to_be_bytes(),
        );
        offset = offset.checked_add(text.len()).ok_or_else(overflow)?;
    }

    let mut bytes = Vec::with_capacity(HEADER_LEN + offset);
    bytes.extend(0u16.to_be_bytes());
    bytes.extend(
        u32::try_from(HEADER_LEN)
            .map_err(|_| overflow())?
            .to_be_bytes(),
    );
    bytes.extend(0u32.to_be_bytes());
    bytes.extend(list);
    for (_, text) in &documents {
        bytes.extend(text.as_bytes());
    }
    Ok(Some(bytes))
}

/// One colour glyph's SVG document under construction.
struct Document<'a, 'f> {
    base: GlyphId16,
    color: &'a ColorData,
    compiled: &'a CompiledGlyphs<'f>,
}

impl Document<'_, '_> {
    /// Renders the document, or `None` when no layer has an outline.
    fn render(
        &self,
        layers: &[ResolvedLayer<'_>],
        path_of: impl Fn(GlyphId16) -> Result<Option<String>, ColorError>,
    ) -> Result<Option<String>, ColorError> {
        let mut defs = String::new();
        let mut body = String::new();
        for (index, layer) in layers.iter().enumerate() {
            let Some(path) = path_of(layer.glyph)? else {
                continue;
            };
            let gradient = format!("g{}_{index}", self.base.to_u16());
            let bounds = self.compiled.bounds(layer.glyph);
            let fill = self.fill(layer.fill, &gradient, bounds, &mut defs);
            let _ = write!(body, r#"<path d="{path}" {fill}/>"#);
        }
        if body.is_empty() {
            return Ok(None);
        }

        let mut text = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg">"#);
        if !defs.is_empty() {
            let _ = write!(text, "<defs>{defs}</defs>");
        }
        let _ = write!(
            text,
            r#"<g id="glyph{}"><g transform="scale(1,-1)">{body}</g></g></svg>"#,
            self.base.to_u16()
        );
        Ok(Some(text))
    }

    /// Returns the fill attributes of a layer, adding any gradient to `defs`.
    fn fill(
        &self,
        paint: &Paint,
        gradient: &str,
        bounds: Option<[i16; 4]>,
        defs: &mut String,
    ) -> String {
        match paint {
            Paint::Solid { color, alpha } => {
                format!(
                    r#"style="fill:{};fill-opacity:{}""#,
                    self.colour(*color),
                    number(*alpha)
                )
            }
            Paint::LinearGradient { stops, p0, p1, p2 } => {
                let end = linear_end(*p0, *p1, *p2);
                let _ = write!(
                    defs,
                    r#"<linearGradient id="{gradient}" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}">{}</linearGradient>"#,
                    number(p0.x),
                    number(p0.y),
                    number(end.x),
                    number(end.y),
                    self.stops(stops),
                );
                format!(r#"fill="url(#{gradient})""#)
            }
            Paint::RadialGradient {
                stops,
                c0,
                r0,
                c1,
                r1,
            } => {
                let outer = r1.unwrap_or_else(|| farthest_corner(*c1, bounds));
                let _ = write!(
                    defs,
                    r#"<radialGradient id="{gradient}" gradientUnits="userSpaceOnUse" cx="{}" cy="{}" r="{}" fx="{}" fy="{}" fr="{}">{}</radialGradient>"#,
                    number(c1.x),
                    number(c1.y),
                    number(outer),
                    number(c0.x),
                    number(c0.y),
                    number(r0.unwrap_or(0.0)),
                    self.stops(stops),
                );
                format!(r#"fill="url(#{gradient})""#)
            }
            // Validation allows only fills here, so these draw nothing
            Paint::Glyph { .. } | Paint::Layers { .. } => r#"fill="none""#.to_string(),
        }
    }

    fn stops(&self, stops: &[ColorStop]) -> String {
        stops
            .iter()
            .map(|stop| {
                format!(
                    r#"<stop offset="{}" style="stop-color:{};stop-opacity:{}"/>"#,
                    number(stop.offset),
                    self.colour(stop.color),
                    number(stop.alpha),
                )
            })
            .collect()
    }

    /// Returns a CSS colour for a paint colour.
    fn colour(&self, color: PaintColor) -> String {
        match color {
            PaintColor::Foreground => "context-fill".to_string(),
            PaintColor::Palette { index } => {
                let fallback = self
                    .color
                    .palettes
                    .first()
                    .and_then(|palette| palette.colors.get(usize::from(index)))
                    .copied()
                    .unwrap_or(Rgba::new(0, 0, 0, 255));
                format!("var(--color{index},{})", hex(fallback))
            }
        }
    }
}

/// Returns the end of the gradient line that a COLR-style `p0`, `p1`, `p2` give.
///
/// The line starts at `p0` and runs along the perpendicular of `p0`-`p2`, as far
/// as `p1` projects onto it. Without `p2` that perpendicular is `p0`-`p1`'s own
/// direction, so the line ends at `p1`.
pub(crate) fn linear_end(p0: ColorPoint, p1: ColorPoint, p2: Option<ColorPoint>) -> ColorPoint {
    let Some(p2) = p2 else {
        return p1;
    };
    let (dx, dy) = (p2.x - p0.x, p2.y - p0.y);
    let length = dx.hypot(dy);
    if length == 0.0 {
        return p1;
    }
    // Unit vector perpendicular to p0-p2
    let (nx, ny) = (-dy / length, dx / length);
    let along = (p1.x - p0.x) * nx + (p1.y - p0.y) * ny;
    ColorPoint::new(p0.x + along * nx, p0.y + along * ny)
}

fn hex(color: Rgba) -> String {
    if color.a == 255 {
        return format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b);
    }
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        color.r, color.g, color.b, color.a
    )
}

/// Formats a number with at most three decimals and no trailing zeros.
fn number(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded == 0.0 {
        return "0".to_string();
    }
    rounded.to_string()
}

/// Returns the SVG path data of a compiled glyph, or `None` for an empty outline.
fn path_data(
    outlines: &skrifa::outline::OutlineGlyphCollection<'_>,
    glyph: GlyphId16,
) -> Result<Option<String>, ColorError> {
    let Some(outline) = outlines.get(skrifa::GlyphId::new(u32::from(glyph.to_u16()))) else {
        return Ok(None);
    };
    let mut pen = PathPen::default();
    outline
        .draw(
            DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
            &mut pen,
        )
        .map_err(|error| ColorError::ReadFont {
            message: error.to_string(),
        })?;
    Ok((!pen.0.is_empty()).then_some(pen.0))
}

/// Collects outline commands as SVG path data.
#[derive(Default)]
struct PathPen(String);

impl OutlinePen for PathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.0, "M{} {}", number(x.into()), number(y.into()));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.0, "L{} {}", number(x.into()), number(y.into()));
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let _ = write!(
            self.0,
            "Q{} {} {} {}",
            number(cx.into()),
            number(cy.into()),
            number(x.into()),
            number(y.into())
        );
    }

    fn curve_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        let _ = write!(
            self.0,
            "C{} {} {} {} {} {}",
            number(c1x.into()),
            number(c1y.into()),
            number(c2x.into()),
            number(c2y.into()),
            number(x.into()),
            number(y.into())
        );
    }

    fn close(&mut self) {
        self.0.push('Z');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_end_without_p2_is_p1() {
        let end = linear_end(
            ColorPoint::new(0.0, 0.0),
            ColorPoint::new(100.0, 40.0),
            None,
        );
        assert_eq!(end, ColorPoint::new(100.0, 40.0));
    }

    #[test]
    fn linear_end_projects_p1_onto_the_perpendicular_of_p0_p2() {
        // p0-p2 is horizontal, so the line runs vertically through p0
        let end = linear_end(
            ColorPoint::new(0.0, 0.0),
            ColorPoint::new(30.0, 80.0),
            Some(ColorPoint::new(50.0, 0.0)),
        );
        assert!(end.x.abs() < 1e-9);
        assert!((end.y - 80.0).abs() < 1e-9);
    }

    #[test]
    fn numbers_drop_trailing_zeros_and_negative_zero() {
        assert_eq!(number(12.0), "12");
        assert_eq!(number(0.5), "0.5");
        assert_eq!(number(-0.0001), "0");
        assert_eq!(number(1.23456), "1.235");
    }

    #[test]
    fn hex_includes_alpha_only_when_translucent() {
        assert_eq!(hex(Rgba::new(10, 20, 30, 255)), "#0a141e");
        assert_eq!(hex(Rgba::new(10, 20, 30, 128)), "#0a141e80");
    }
}
