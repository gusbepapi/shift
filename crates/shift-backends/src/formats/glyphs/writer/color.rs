//! Writes colour palettes and colour glyphs into a Glyphs file.
//!
//! Glyphs has two colour models, and each colour glyph uses the one that holds
//! it without loss:
//!
//! - A glyph whose layers are all opaque solid fills keeps its master layers and
//!   gains one palette layer per paint layer, with `colorPalette` set to the
//!   palette entry (`"*"` for the text foreground). Palette identity survives.
//! - Any other glyph, with a gradient or a translucent fill, is written as
//!   `color = 1` master layers whose shapes carry literal fill colours or
//!   gradients: components for solid fills, and the outlines of the layer glyph
//!   for gradients. Those layers replace the master layers' own shapes, and the
//!   colours come from palette 0 because the model has no palette entries there.
//!
//! Glyphs cannot express some of what Shift holds. Palette names are not
//! written. A radial gradient keeps its outer centre only, because Glyphs
//! derives the radius itself. A linear gradient keeps its axis but not the
//! rotation reference. A text foreground stop or layer in a literal glyph is
//! written black, and every gradient spreads by padding.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use shift_font::{
    ColorData, ColorPoint, ColorStop, Glyph, GlyphId, GlyphLayer, Paint, PaintColor, Rgba,
    SourceId,
    composite::{resolved_contours_bbox, resolved_contours_from_layers},
};

use super::{Dict, Master, Value, format_number, parameter, path_dict};
use crate::numeric::saturating_u32;
use crate::svg::linear_end;
use crate::traits::FontView;

/// Palette index that Glyphs spells `"*"` for the text foreground colour.
const FOREGROUND_INDEX: u16 = 0xFFFF;

/// Bounds of a glyph's outline as `(x_min, y_min, x_max, y_max)` in font units.
type Bounds = (f64, f64, f64, f64);

/// The `Color Palettes` custom parameter, or `None` when the font has no palette.
///
/// Palette names have no place in this parameter and are not written.
pub(super) fn palettes_parameter(color: &ColorData) -> Option<Value> {
    if color.palettes.is_empty() {
        return None;
    }
    let palettes = color
        .palettes
        .iter()
        .map(|palette| {
            Value::Array(
                palette
                    .colors
                    .iter()
                    .map(|color| Value::Raw(rgba_text(*color)))
                    .collect(),
            )
        })
        .collect();
    Some(parameter("Color Palettes", Value::Array(palettes)))
}

/// How one colour glyph is written to every master.
pub(super) enum GlyphColor {
    /// Palette layers beside the master layer, one per paint layer in order.
    Palette(Vec<PaletteLayer>),
    /// Shapes that replace the master layer's own and mark it as a colour layer.
    Literal(Vec<LiteralLayer>),
}

/// One palette layer: the glyph that it draws and the palette entry that fills it.
pub(super) struct PaletteLayer {
    glyph_name: String,
    palette_index: u16,
}

/// One layer with a literal colour or gradient.
pub(super) struct LiteralLayer {
    glyph: GlyphId,
    glyph_name: String,
    /// The `attr` of every shape of the layer: `fillColor` or `gradient`.
    attributes: Dict,
    /// Whether the layer is drawn as outlines instead of a component.
    ///
    /// fontc measures a gradient against the bounds of the glyph it paints, and
    /// that glyph has none while it holds only components, so a gradient layer
    /// carries the outlines of its glyph.
    outlined: bool,
}

impl GlyphColor {
    /// Returns the layers that go beside a master layer, with `width` as their width.
    pub(super) fn extra_layers(&self, master: &Master<'_>, width: f64) -> Vec<Value> {
        let Self::Palette(layers) = self else {
            return Vec::new();
        };
        layers
            .iter()
            .enumerate()
            .map(|(index, layer)| {
                let mut attributes = Dict::new();
                attributes.put(
                    "colorPalette",
                    if layer.palette_index == FOREGROUND_INDEX {
                        Value::string("*")
                    } else {
                        Value::number(f64::from(layer.palette_index))
                    },
                );
                let mut component = Dict::new();
                component.put("ref", Value::string(&layer.glyph_name));

                let mut value = Dict::new();
                value.put("associatedMasterId", Value::string(&master.id));
                value.put("attr", Value::Dict(attributes));
                value.put("layerId", Value::string(&format!("{}-C{index}", master.id)));
                value.put("name", Value::string(&format!("Color {}", index + 1)));
                value.put("shapes", Value::Array(vec![Value::Dict(component)]));
                value.put("width", Value::number(width));
                Value::Dict(value)
            })
            .collect()
    }
}

/// Plans how the colour glyphs of a font are written.
pub(super) struct ColorWriter<'a, F: FontView> {
    font: &'a F,
    names: &'a HashMap<GlyphId, String>,
    /// Layers of each source by glyph, read once when a gradient first needs them.
    layers: RefCell<HashMap<SourceId, Rc<HashMap<GlyphId, GlyphLayer>>>>,
}

impl<'a, F: FontView> ColorWriter<'a, F> {
    pub(super) fn new(font: &'a F, names: &'a HashMap<GlyphId, String>) -> Self {
        Self {
            font,
            names,
            layers: RefCell::new(HashMap::new()),
        }
    }

    /// Returns how `glyph` is written, or `None` when it is not a colour glyph.
    pub(super) fn plan(&self, glyph: &Glyph) -> Option<GlyphColor> {
        let entry = self.font.color().glyph(&glyph.id())?;
        let layers = paint_layers(&entry.paint);
        if layers.is_empty() {
            return None;
        }

        let mut palette = Vec::with_capacity(layers.len());
        for (layer_glyph, fill) in &layers {
            let Paint::Solid { color, alpha } = fill else {
                return Some(self.literal(&layers));
            };
            if *alpha < 1.0 {
                return Some(self.literal(&layers));
            }
            palette.push(PaletteLayer {
                glyph_name: self.name(layer_glyph),
                palette_index: match color {
                    PaintColor::Foreground => FOREGROUND_INDEX,
                    PaintColor::Palette { index } => *index,
                },
            });
        }
        Some(GlyphColor::Palette(palette))
    }

    /// Returns the shapes that replace a master layer's, for a literal colour glyph.
    pub(super) fn literal_shapes(
        &self,
        colour: &GlyphColor,
        master: &Master<'_>,
    ) -> Option<Vec<Value>> {
        let GlyphColor::Literal(layers) = colour else {
            return None;
        };
        let source_layers = self.layers_of(&master.source.id());
        let mut shapes = Vec::with_capacity(layers.len());
        for layer in layers {
            if !layer.outlined {
                let mut component = Dict::new();
                component.put("attr", Value::Dict(layer.attributes.clone()));
                component.put("ref", Value::string(&layer.glyph_name));
                shapes.push(Value::Dict(component));
                continue;
            }
            let contours =
                resolved_contours_from_layers(&layer.glyph, &source_layers).unwrap_or_default();
            for contour in contours.iter().filter(|c| c.points.len() > 1) {
                let mut path = path_dict(&contour.points, contour.closed);
                path.entries.insert(
                    0,
                    ("attr".to_string(), Value::Dict(layer.attributes.clone())),
                );
                shapes.push(Value::Dict(path));
            }
        }
        Some(shapes)
    }

    fn name(&self, glyph: &GlyphId) -> String {
        self.names
            .get(glyph)
            .map_or_else(|| glyph.to_string(), Clone::clone)
    }

    /// Plans layers with literal colours, grouping equal neighbouring paints into
    /// runs because Glyphs measures a gradient against the bounds of its run.
    fn literal(&self, layers: &[(&GlyphId, &Paint)]) -> GlyphColor {
        let mut planned = Vec::with_capacity(layers.len());
        let mut start = 0;
        while start < layers.len() {
            let fill = layers[start].1;
            let end = layers[start..]
                .iter()
                .position(|(_, other)| *other != fill)
                .map_or(layers.len(), |offset| start + offset);
            let bounds = self.run_bounds(&layers[start..end]);
            for (glyph, _) in &layers[start..end] {
                planned.push(self.literal_layer(glyph, fill, bounds));
            }
            start = end;
        }
        GlyphColor::Literal(planned)
    }

    fn literal_layer(&self, glyph: &GlyphId, fill: &Paint, bounds: Option<Bounds>) -> LiteralLayer {
        let mut attributes = Dict::new();
        let mut outlined = false;
        match fill {
            Paint::Solid { color, alpha } => {
                attributes.put(
                    "fillColor",
                    Value::Raw(rgba_text(self.literal_colour(*color, *alpha))),
                );
            }
            Paint::LinearGradient { stops, p0, p1, p2 } => {
                let end = linear_end(*p0, *p1, *p2);
                attributes.put(
                    "gradient",
                    self.gradient(stops, *p0, Some(end), bounds, false),
                );
                outlined = true;
            }
            Paint::RadialGradient { stops, c1, .. } => {
                attributes.put("gradient", self.gradient(stops, *c1, None, bounds, true));
                outlined = true;
            }
            // Validation allows only fills in a layer, so nothing is drawn
            Paint::Glyph { .. } | Paint::Layers { .. } => {}
        }
        LiteralLayer {
            glyph: glyph.clone(),
            glyph_name: self.name(glyph),
            attributes,
            outlined,
        }
    }

    /// Builds a Glyphs gradient, whose points are fractions of the run's bounds.
    fn gradient(
        &self,
        stops: &[ColorStop],
        start: ColorPoint,
        end: Option<ColorPoint>,
        bounds: Option<Bounds>,
        circle: bool,
    ) -> Value {
        let mut gradient = Dict::new();
        gradient.put(
            "colors",
            Value::Array(
                stops
                    .iter()
                    .map(|stop| {
                        Value::Raw(format!(
                            "({},{})",
                            rgba_text(self.literal_colour(stop.color, stop.alpha)),
                            format_number(stop.offset)
                        ))
                    })
                    .collect(),
            ),
        );
        // A circle gradient has one centre, and Glyphs takes its radius from the bounds
        let end = end.unwrap_or(start);
        gradient.put("end", Value::Raw(fraction_text(end, bounds)));
        gradient.put("start", Value::Raw(fraction_text(start, bounds)));
        if circle {
            gradient.put("type", Value::string("circle"));
        }
        Value::Dict(gradient)
    }

    /// Returns the colour of palette 0 with `alpha` multiplied in.
    fn literal_colour(&self, color: PaintColor, alpha: f64) -> Rgba {
        let base = match color {
            PaintColor::Foreground => Rgba::new(0, 0, 0, 255),
            PaintColor::Palette { index } => self
                .font
                .color()
                .palettes
                .first()
                .and_then(|palette| palette.colors.get(usize::from(index)))
                .copied()
                .unwrap_or(Rgba::new(0, 0, 0, 255)),
        };
        let opacity = f64::from(base.a) * alpha.clamp(0.0, 1.0);
        Rgba::new(
            base.r,
            base.g,
            base.b,
            u8::try_from(saturating_u32(opacity.round())).unwrap_or(u8::MAX),
        )
    }

    /// Returns the layers of `source` by glyph, reading them on first use.
    fn layers_of(&self, source: &SourceId) -> Rc<HashMap<GlyphId, GlyphLayer>> {
        let mut cache = self.layers.borrow_mut();
        Rc::clone(cache.entry(source.clone()).or_insert_with(|| {
            Rc::new(
                self.font
                    .glyphs()
                    .into_iter()
                    .filter_map(|glyph| Some((glyph.id(), glyph.layer_for_source(source)?.clone())))
                    .collect(),
            )
        }))
    }

    /// Returns the union of the default-source outlines of a run's layer glyphs.
    fn run_bounds(&self, run: &[(&GlyphId, &Paint)]) -> Option<Bounds> {
        let layers = self.layers_of(&self.font.default_source_id()?);
        run.iter()
            .filter_map(|(glyph, _)| {
                let contours = resolved_contours_from_layers(glyph, &layers).ok()?;
                resolved_contours_bbox(&contours)
            })
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
    }
}

/// Lists the glyph and fill of each paint layer of a colour glyph.
fn paint_layers(paint: &Paint) -> Vec<(&GlyphId, &Paint)> {
    let single = std::slice::from_ref(paint);
    let layers = match paint {
        Paint::Layers { layers } => layers.as_slice(),
        _ => single,
    };
    layers
        .iter()
        .filter_map(|layer| match layer {
            Paint::Glyph { glyph, fill } => Some((glyph, fill.as_ref())),
            _ => None,
        })
        .collect()
}

fn rgba_text(color: Rgba) -> String {
    format!("({},{},{},{})", color.r, color.g, color.b, color.a)
}

/// Writes a point as fractions of `bounds`; a span of zero maps to 0.
fn fraction_text(point: ColorPoint, bounds: Option<Bounds>) -> String {
    let fraction = |value: f64, min: f64, max: f64| {
        if max > min {
            (value - min) / (max - min)
        } else {
            0.0
        }
    };
    let (min_x, min_y, max_x, max_y) = bounds.unwrap_or((0.0, 0.0, 0.0, 0.0));
    format!(
        "({},{})",
        format_number(fraction(point.x, min_x, max_x)),
        format_number(fraction(point.y, min_y, max_y))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_are_measured_from_the_bounds() {
        let bounds = Some((100.0, 0.0, 300.0, 800.0));

        assert_eq!(
            fraction_text(ColorPoint::new(200.0, 800.0), bounds),
            "(0.5,1)"
        );
        assert_eq!(fraction_text(ColorPoint::new(100.0, 0.0), bounds), "(0,0)");
    }

    #[test]
    fn a_zero_span_or_missing_bounds_maps_to_zero() {
        assert_eq!(
            fraction_text(ColorPoint::new(5.0, 5.0), Some((5.0, 5.0, 5.0, 5.0))),
            "(0,0)"
        );
        assert_eq!(fraction_text(ColorPoint::new(5.0, 5.0), None), "(0,0)");
    }

    #[test]
    fn colours_are_written_as_integer_tuples() {
        assert_eq!(rgba_text(Rgba::new(255, 0, 10, 128)), "(255,0,10,128)");
    }
}
