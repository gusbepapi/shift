use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult, GlyphId};

/// Largest palette entry count that `CPAL` can address, because index
/// `0xFFFF` is reserved for the text foreground colour.
pub const MAX_PALETTE_ENTRIES: usize = 0xFFFE;

/// Deepest paint nesting accepted by validation.
const MAX_PAINT_DEPTH: usize = 8;

/// An eight-bit RGBA colour in the sRGB colour space.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    /// Creates a colour from its four channels.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// One named colour palette, mirroring a `CPAL` palette.
///
/// Every palette in a font has the same number of entries, and entry `n` of
/// every palette plays the same role, so paints refer to colours by index and
/// switching palette recolours the whole font.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorPalette {
    pub name: Option<String>,
    pub colors: Vec<Rgba>,
}

/// A paint colour that is either a palette entry or the text foreground.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PaintColor {
    /// The colour of the surrounding text, `0xFFFF` in `COLR`.
    Foreground,
    /// An entry of the palettes, resolved against the active palette.
    Palette { index: u16 },
}

/// A gradient coordinate in font units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorPoint {
    pub x: f64,
    pub y: f64,
}

impl ColorPoint {
    /// Creates a coordinate.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// One stop of a gradient colour line.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorStop {
    /// Position along the colour line, from 0 to 1.
    pub offset: f64,
    pub color: PaintColor,
    /// Opacity multiplied into the colour, from 0 to 1.
    pub alpha: f64,
}

/// A fill or a clipped layer of a colour glyph.
///
/// Coordinates of gradient geometry are font units in the base glyph's space.
/// A colour glyph is a [`Paint::Glyph`] or a [`Paint::Layers`] of them, and a
/// glyph paint fills with [`Paint::Solid`], [`Paint::LinearGradient`], or
/// [`Paint::RadialGradient`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Paint {
    /// Fills the outline of another glyph with the nested paint.
    Glyph {
        glyph: GlyphId,
        fill: Box<Paint>,
    },
    /// Paints the layers in order, first at the bottom.
    Layers {
        layers: Vec<Paint>,
    },
    Solid {
        color: PaintColor,
        alpha: f64,
    },
    /// A linear gradient along `p0` to `p1`, rotated by `p2`.
    LinearGradient {
        stops: Vec<ColorStop>,
        p0: ColorPoint,
        p1: ColorPoint,
        /// Rotation reference; `None` derives a perpendicular to `p0`-`p1`.
        p2: Option<ColorPoint>,
    },
    /// A radial gradient between two circles.
    RadialGradient {
        stops: Vec<ColorStop>,
        c0: ColorPoint,
        /// Inner radius; `None` is 0.
        r0: Option<f64>,
        c1: ColorPoint,
        /// Outer radius; `None` is derived from the bounding box.
        r1: Option<f64>,
    },
}

/// The paint graph of one colour base glyph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorGlyph {
    /// The base glyph that text maps to; its own outline is the fallback.
    pub glyph: GlyphId,
    pub paint: Paint,
}

/// Font-wide colour data: palettes and colour glyph paint graphs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorData {
    #[serde(default)]
    pub palettes: Vec<ColorPalette>,
    #[serde(default)]
    pub glyphs: Vec<ColorGlyph>,
}

impl ColorData {
    /// Returns whether the font carries no colour data.
    pub fn is_empty(&self) -> bool {
        self.palettes.is_empty() && self.glyphs.is_empty()
    }

    /// Returns the colour glyph for a base glyph.
    pub fn glyph(&self, glyph: &GlyphId) -> Option<&ColorGlyph> {
        self.glyphs.iter().find(|entry| &entry.glyph == glyph)
    }

    /// Returns the number of entries every palette has.
    pub fn palette_len(&self) -> usize {
        self.palettes
            .first()
            .map_or(0, |palette| palette.colors.len())
    }

    /// Validates palettes and paint graphs against the glyphs that exist.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidColor`] when palettes differ in length,
    /// when a base glyph repeats or is missing, when a paint refers to a
    /// missing glyph or palette entry, or when a gradient or alpha value is
    /// not finite and in range.
    pub fn validate(&self, glyph_exists: impl Fn(&GlyphId) -> bool) -> CoreResult<()> {
        let expected = self.palette_len();
        if self.palettes.len() > usize::from(u16::MAX) {
            return Err(invalid("too many palettes"));
        }
        if expected > MAX_PALETTE_ENTRIES {
            return Err(invalid("palette has too many entries"));
        }
        for (index, palette) in self.palettes.iter().enumerate() {
            if palette.colors.len() != expected {
                return Err(invalid(format!(
                    "palette {index} has {} entries but palette 0 has {expected}",
                    palette.colors.len()
                )));
            }
        }

        let base_glyphs: HashSet<&GlyphId> = self.glyphs.iter().map(|entry| &entry.glyph).collect();
        if base_glyphs.len() != self.glyphs.len() {
            return Err(invalid("a base glyph has more than one paint graph"));
        }
        for entry in &self.glyphs {
            if !glyph_exists(&entry.glyph) {
                return Err(invalid(format!(
                    "base glyph {} does not exist",
                    entry.glyph
                )));
            }
            let context = PaintContext {
                base: &entry.glyph,
                base_glyphs: &base_glyphs,
                palette_len: expected,
                glyph_exists: &glyph_exists,
            };
            match &entry.paint {
                Paint::Glyph { .. } => context.check(&entry.paint, Position::Layer, 0)?,
                Paint::Layers { layers } => {
                    if layers.is_empty() {
                        return Err(invalid("a layer stack must contain a layer"));
                    }
                    for layer in layers {
                        if !matches!(layer, Paint::Glyph { .. }) {
                            return Err(invalid("a layer stack may contain only glyph paints"));
                        }
                        context.check(layer, Position::Layer, 1)?;
                    }
                }
                _ => return Err(invalid("a colour glyph must be a glyph or a layer stack")),
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Position {
    Layer,
    Fill,
}

struct PaintContext<'a, F: Fn(&GlyphId) -> bool> {
    base: &'a GlyphId,
    base_glyphs: &'a HashSet<&'a GlyphId>,
    palette_len: usize,
    glyph_exists: &'a F,
}

impl<F: Fn(&GlyphId) -> bool> PaintContext<'_, F> {
    fn check(&self, paint: &Paint, position: Position, depth: usize) -> CoreResult<()> {
        if depth > MAX_PAINT_DEPTH {
            return Err(invalid("paint nesting is too deep"));
        }
        match paint {
            Paint::Glyph { glyph, fill } => {
                if position == Position::Fill {
                    return Err(invalid("a fill must not be a glyph paint"));
                }
                if glyph == self.base {
                    return Err(invalid("a layer must not use its own base glyph"));
                }
                if self.base_glyphs.contains(glyph) {
                    return Err(invalid(format!(
                        "layer glyph {glyph} is itself a colour glyph"
                    )));
                }
                if !(self.glyph_exists)(glyph) {
                    return Err(invalid(format!("layer glyph {glyph} does not exist")));
                }
                self.check(fill, Position::Fill, depth + 1)
            }
            Paint::Layers { .. } => Err(invalid("a layer stack cannot be nested")),
            Paint::Solid { color, alpha } => {
                self.check_color(*color)?;
                check_unit("alpha", *alpha)
            }
            Paint::LinearGradient { stops, p0, p1, p2 } => {
                for point in [Some(p0), Some(p1), p2.as_ref()].into_iter().flatten() {
                    check_point(point)?;
                }
                self.check_stops(stops)
            }
            Paint::RadialGradient {
                stops,
                c0,
                r0,
                c1,
                r1,
            } => {
                check_point(c0)?;
                check_point(c1)?;
                for radius in [r0, r1].into_iter().flatten() {
                    if !radius.is_finite() || *radius < 0.0 {
                        return Err(invalid("a radius must be finite and not negative"));
                    }
                }
                self.check_stops(stops)
            }
        }
    }

    fn check_color(&self, color: PaintColor) -> CoreResult<()> {
        if let PaintColor::Palette { index } = color
            && usize::from(index) >= self.palette_len
        {
            return Err(invalid(format!(
                "palette entry {index} does not exist; palettes have {} entries",
                self.palette_len
            )));
        }
        Ok(())
    }

    fn check_stops(&self, stops: &[ColorStop]) -> CoreResult<()> {
        if stops.is_empty() {
            return Err(invalid("a gradient needs at least one colour stop"));
        }
        let mut previous = 0.0;
        for stop in stops {
            check_unit("stop offset", stop.offset)?;
            check_unit("stop alpha", stop.alpha)?;
            if stop.offset < previous {
                return Err(invalid("gradient stops must be ordered by offset"));
            }
            previous = stop.offset;
            self.check_color(stop.color)?;
        }
        Ok(())
    }
}

fn check_unit(label: &str, value: f64) -> CoreResult<()> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(invalid(format!("{label} must be between 0 and 1")))
    }
}

fn check_point(point: &ColorPoint) -> CoreResult<()> {
    if point.x.is_finite() && point.y.is_finite() {
        Ok(())
    } else {
        Err(invalid("gradient coordinates must be finite"))
    }
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::InvalidColor {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette(count: usize) -> ColorPalette {
        ColorPalette {
            name: None,
            colors: (0..count)
                .map(|index| Rgba::new(index as u8, 0, 0, 255))
                .collect(),
        }
    }

    fn solid(index: u16) -> Paint {
        Paint::Solid {
            color: PaintColor::Palette { index },
            alpha: 1.0,
        }
    }

    fn layer(glyph: &GlyphId, fill: Paint) -> Paint {
        Paint::Glyph {
            glyph: glyph.clone(),
            fill: Box::new(fill),
        }
    }

    fn fixture() -> (GlyphId, GlyphId, GlyphId) {
        (GlyphId::new(), GlyphId::new(), GlyphId::new())
    }

    #[test]
    fn empty_data_is_valid() {
        assert!(ColorData::default().validate(|_| false).is_ok());
        assert!(ColorData::default().is_empty());
    }

    #[test]
    fn accepts_layered_glyph_with_gradient() {
        let (base, a, b) = fixture();
        let data = ColorData {
            palettes: vec![palette(2), palette(2)],
            glyphs: vec![ColorGlyph {
                glyph: base.clone(),
                paint: Paint::Layers {
                    layers: vec![
                        layer(&a, solid(0)),
                        layer(
                            &b,
                            Paint::LinearGradient {
                                stops: vec![
                                    ColorStop {
                                        offset: 0.0,
                                        color: PaintColor::Palette { index: 0 },
                                        alpha: 1.0,
                                    },
                                    ColorStop {
                                        offset: 1.0,
                                        color: PaintColor::Foreground,
                                        alpha: 0.5,
                                    },
                                ],
                                p0: ColorPoint::new(0.0, 0.0),
                                p1: ColorPoint::new(0.0, 700.0),
                                p2: None,
                            },
                        ),
                    ],
                },
            }],
        };
        assert!(data.validate(|_| true).is_ok());
        assert_eq!(data.palette_len(), 2);
        assert!(data.glyph(&base).is_some());
    }

    #[test]
    fn rejects_palettes_of_unequal_length() {
        let data = ColorData {
            palettes: vec![palette(2), palette(3)],
            glyphs: Vec::new(),
        };
        assert!(matches!(
            data.validate(|_| true),
            Err(CoreError::InvalidColor { .. })
        ));
    }

    #[test]
    fn rejects_out_of_range_palette_entry() {
        let (base, a, _) = fixture();
        let data = ColorData {
            palettes: vec![palette(1)],
            glyphs: vec![ColorGlyph {
                glyph: base,
                paint: layer(&a, solid(1)),
            }],
        };
        assert!(data.validate(|_| true).is_err());
    }

    #[test]
    fn rejects_missing_and_self_referencing_glyphs() {
        let (base, a, _) = fixture();
        let data = ColorData {
            palettes: vec![palette(1)],
            glyphs: vec![ColorGlyph {
                glyph: base.clone(),
                paint: layer(&base, solid(0)),
            }],
        };
        assert!(data.validate(|_| true).is_err());

        let data = ColorData {
            palettes: vec![palette(1)],
            glyphs: vec![ColorGlyph {
                glyph: base,
                paint: layer(&a, solid(0)),
            }],
        };
        assert!(data.validate(|id| id != &a).is_err());
    }

    #[test]
    fn rejects_duplicate_base_glyph_and_nested_colour_glyph() {
        let (base, a, b) = fixture();
        let entry = |glyph: &GlyphId, layer_glyph: &GlyphId| ColorGlyph {
            glyph: glyph.clone(),
            paint: layer(layer_glyph, solid(0)),
        };
        let duplicate = ColorData {
            palettes: vec![palette(1)],
            glyphs: vec![entry(&base, &a), entry(&base, &b)],
        };
        assert!(duplicate.validate(|_| true).is_err());

        let nested = ColorData {
            palettes: vec![palette(1)],
            glyphs: vec![entry(&base, &a), entry(&a, &b)],
        };
        assert!(nested.validate(|_| true).is_err());
    }

    #[test]
    fn rejects_bad_alpha_stops_and_structure() {
        let (base, a, _) = fixture();
        let with = |paint: Paint| ColorData {
            palettes: vec![palette(1)],
            glyphs: vec![ColorGlyph {
                glyph: base.clone(),
                paint,
            }],
        };
        let bad_alpha = layer(
            &a,
            Paint::Solid {
                color: PaintColor::Foreground,
                alpha: 1.5,
            },
        );
        assert!(with(bad_alpha).validate(|_| true).is_err());

        let unordered = layer(
            &a,
            Paint::RadialGradient {
                stops: vec![
                    ColorStop {
                        offset: 0.8,
                        color: PaintColor::Foreground,
                        alpha: 1.0,
                    },
                    ColorStop {
                        offset: 0.2,
                        color: PaintColor::Foreground,
                        alpha: 1.0,
                    },
                ],
                c0: ColorPoint::new(0.0, 0.0),
                r0: None,
                c1: ColorPoint::new(0.0, 0.0),
                r1: Some(100.0),
            },
        );
        assert!(with(unordered).validate(|_| true).is_err());

        assert!(with(solid(0)).validate(|_| true).is_err());
        assert!(
            with(Paint::Layers { layers: Vec::new() })
                .validate(|_| true)
                .is_err()
        );
    }

    #[test]
    fn serialises_as_tagged_camel_case_json() {
        let (base, a, _) = fixture();
        let data = ColorData {
            palettes: vec![palette(1)],
            glyphs: vec![ColorGlyph {
                glyph: base,
                paint: layer(&a, solid(0)),
            }],
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"type\":\"glyph\""));
        assert_eq!(serde_json::from_str::<ColorData>(&json).unwrap(), data);
    }
}
