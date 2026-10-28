//! Reads colour palettes and colour glyphs from a Glyphs source.
//!
//! This is the inverse of the colour export in `writer/color.rs`. Glyphs has
//! two colour models, and a glyph uses the first that it has:
//!
//! - **Palette layers** are non-master layers whose `colorPalette` names a
//!   palette entry (`"*"` is the text foreground). Each becomes one paint
//!   layer, filled with that entry, in the order the layers appear.
//! - A **colour layer** is a layer with `color = 1`. Each of its shapes becomes
//!   one paint layer, filled with the shape's `fillColor` or `gradient`.
//!
//! A paint layer needs a glyph to fill. A layer that is a single unmoved
//! component keeps the glyph it names, which is what the export writes.
//! Otherwise the layer's own outlines become a new glyph named
//! `<glyph>.color<N>`, with one layer in every master that has the matching
//! Glyphs layer.
//!
//! Paint comes from the default master, because Shift keeps one paint graph per
//! glyph; the other masters supply only the geometry of new layer glyphs.
//! Literal colours are appended to every palette, equal colours sharing an
//! entry. A Glyphs gradient measures its points as fractions of the bounds of
//! the run of shapes that share it, and those fractions become font units here.

use std::collections::{HashMap, HashSet};

use glyphs_reader::{
    Color, Component, Font as GlyphsFont, Glyph as GlyphsGlyph, Layer as GlyphsLayer, Shape,
    ShapeAttributes,
};
use ordered_float::OrderedFloat;
use shift_font::{
    ColorData, ColorGlyph, ColorPalette, ColorPoint, ColorStop, GlyphId, GlyphLayer, LayerId,
    Paint, PaintColor, Rgba, SourceId,
    composite::{resolved_contours_bbox, resolved_contours_from_layers},
};

use super::conversion::{GlyphsLayerSources, add_shape};
use crate::{FormatBackendError, FormatBackendResult, ImportLoss, ImportLossKind, SourceGlyphIds};

/// Palette index that Glyphs spells `"*"` for the text foreground colour.
const FOREGROUND_INDEX: i64 = 0xFFFF;

/// Most entries a palette can hold, since `0xFFFF` means the foreground.
const MAX_PALETTE_ENTRIES: usize = 0xFFFE;

/// Bounds as `(x_min, y_min, x_max, y_max)` in font units.
type Bounds = (f64, f64, f64, f64);

/// Where the outlines of a new layer glyph come from in a Glyphs glyph.
#[derive(Clone, Copy, Debug)]
enum Origin {
    /// The nth palette layer of the glyph.
    PaletteLayer(usize),
    /// The nth shape of the glyph's colour layer.
    ColorShape(usize),
}

/// A layer glyph that Glyphs does not name, built from layer outlines.
pub(super) struct SynthesizedGlyph {
    pub(super) id: GlyphId,
    pub(super) name: String,
    base: String,
    origin: Origin,
    /// The Glyphs master id and source of every master that has the layer.
    layers: Vec<(String, SourceId)>,
}

impl SynthesizedGlyph {
    /// Returns the number of layers the glyph has.
    pub(super) fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Builds the glyph from the source glyph it was taken from.
    pub(super) fn convert(
        &self,
        source: &GlyphsFont,
        glyph_ids: &HashMap<String, GlyphId>,
    ) -> FormatBackendResult<shift_font::Glyph> {
        let mut glyph = shift_font::Glyph::with_id(self.id, self.name.clone());
        let Some(base) = source.glyphs.get(self.base.as_str()) else {
            return Ok(glyph);
        };
        for (master_id, source_id) in &self.layers {
            let Some((layer, shapes)) = origin_layer(base, self.origin, master_id) else {
                continue;
            };
            let mut result =
                GlyphLayer::with_width(LayerId::new(), *source_id, layer.width.into_inner());
            for shape in shapes {
                add_shape(&mut result, shape, glyph_ids)?;
            }
            glyph.set_layer(result);
        }
        Ok(glyph)
    }
}

/// The palettes, paint graphs and new glyphs that a Glyphs source contributes.
pub(super) struct ColorImport {
    pub(super) data: ColorData,
    pub(super) glyphs: Vec<SynthesizedGlyph>,
}

/// Plans the colour of `source`, publishing the identities of its new glyphs.
///
/// New layer glyphs are added to `glyph_ids` so the directory can declare them
/// before any batch is read.
///
/// # Errors
///
/// Returns an error when the planned colour data fails Shift's validation,
/// which means a layer refers to a glyph or palette entry that does not exist.
pub(super) fn plan_color(
    source: &GlyphsFont,
    ids: &SourceGlyphIds,
    sources: &GlyphsLayerSources,
    glyph_ids: &mut HashMap<String, GlyphId>,
    losses: &mut Vec<ImportLoss>,
) -> FormatBackendResult<ColorImport> {
    let Some(default_master) = source.masters.get(source.default_master_idx) else {
        return Ok(ColorImport::empty());
    };
    let default_master = default_master.id.as_str();

    let mut planner = Planner::new(source, ids, sources, glyph_ids, default_master);
    let mut glyphs = Vec::new();
    for glyph in source.glyphs.values() {
        let paints = if planner.colour_bases.contains(glyph.name.as_str()) {
            planner.paints(glyph)
        } else {
            Vec::new()
        };
        if paints.is_empty() {
            continue;
        }
        let base = planner.glyph_ids[glyph.name.as_str()];
        glyphs.push(ColorGlyph {
            glyph: base,
            paint: Paint::Layers { layers: paints },
        });
    }

    planner.report(&glyphs, losses);
    let data = ColorData {
        palettes: planner.palettes,
        glyphs,
    };
    let synthesized = planner.synthesized;
    let known: HashSet<&GlyphId> = glyph_ids.values().collect();
    data.validate(|id| known.contains(id))
        .map_err(|error| FormatBackendError::Glyphs(format!("invalid colour data: {error}")))?;
    Ok(ColorImport {
        data,
        glyphs: synthesized,
    })
}

impl ColorImport {
    fn empty() -> Self {
        Self {
            data: ColorData::default(),
            glyphs: Vec::new(),
        }
    }
}

struct Planner<'a> {
    source: &'a GlyphsFont,
    ids: &'a SourceGlyphIds,
    sources: &'a GlyphsLayerSources,
    glyph_ids: &'a mut HashMap<String, GlyphId>,
    default_master: &'a str,
    palettes: Vec<ColorPalette>,
    /// Entries the file declares; palette layers may address only these.
    declared_len: usize,
    literals: HashMap<Rgba, u16>,
    /// Names of the glyphs that have colour layers.
    colour_bases: HashSet<&'a str>,
    synthesized: Vec<SynthesizedGlyph>,
    omitted_layers: usize,
    unsupported_gradients: usize,
    overflowed_colours: usize,
    trimmed_palettes: bool,
}

impl<'a> Planner<'a> {
    fn new(
        source: &'a GlyphsFont,
        ids: &'a SourceGlyphIds,
        sources: &'a GlyphsLayerSources,
        glyph_ids: &'a mut HashMap<String, GlyphId>,
        default_master: &'a str,
    ) -> Self {
        let (palettes, trimmed_palettes) = declared_palettes(source);
        let declared_len = palettes.first().map_or(0, |palette| palette.colors.len());
        let colour_bases = source
            .glyphs
            .values()
            .filter(|glyph| {
                palette_layers(glyph, default_master)
                    .iter()
                    .any(|layer| !layer.shapes.is_empty())
                    || colour_layer(glyph, default_master).is_some_and(|l| !l.shapes.is_empty())
            })
            .map(|glyph| glyph.name.as_str())
            .collect();
        Self {
            source,
            ids,
            sources,
            glyph_ids,
            default_master,
            palettes,
            declared_len,
            literals: HashMap::new(),
            colour_bases,
            synthesized: Vec::new(),
            omitted_layers: 0,
            unsupported_gradients: 0,
            overflowed_colours: 0,
            trimmed_palettes,
        }
    }

    /// Returns the paint layers of `glyph`: palette layers when it has any
    /// usable, otherwise the shapes of its colour layer.
    fn paints(&mut self, glyph: &GlyphsGlyph) -> Vec<Paint> {
        let from_palette = self.palette_paints(glyph);
        if !from_palette.is_empty() {
            return from_palette;
        }
        self.literal_paints(glyph)
    }

    fn palette_paints(&mut self, glyph: &GlyphsGlyph) -> Vec<Paint> {
        let mut paints = Vec::new();
        for (index, layer) in palette_layers(glyph, self.default_master)
            .into_iter()
            .enumerate()
        {
            if layer.shapes.is_empty() {
                continue;
            }
            let Some(color) = self.palette_color(layer.attributes.color_palette) else {
                self.omitted_layers += 1;
                continue;
            };
            let shapes = layer.shapes.iter().collect::<Vec<_>>();
            let layer_glyph = self.layer_glyph(glyph, &shapes, Origin::PaletteLayer(index));
            paints.push(Paint::Glyph {
                glyph: layer_glyph,
                fill: Box::new(Paint::Solid { color, alpha: 1.0 }),
            });
        }
        paints
    }

    fn literal_paints(&mut self, glyph: &GlyphsGlyph) -> Vec<Paint> {
        let Some(layer) = colour_layer(glyph, self.default_master) else {
            return Vec::new();
        };
        let shapes = &layer.shapes;
        let mut paints = Vec::with_capacity(shapes.len());
        let mut start = 0;
        while start < shapes.len() {
            let attributes = shapes[start].attributes();
            let end = shapes[start..]
                .iter()
                .position(|shape| shape.attributes() != attributes)
                .map_or(shapes.len(), |offset| start + offset);
            let bounds = if attributes.gradient.is_some() {
                self.run_bounds(&shapes[start..end])
            } else {
                None
            };
            for (index, shape) in shapes.iter().enumerate().take(end).skip(start) {
                let fill = self.fill(attributes, bounds);
                let layer_glyph = self.layer_glyph(glyph, &[shape], Origin::ColorShape(index));
                paints.push(Paint::Glyph {
                    glyph: layer_glyph,
                    fill: Box::new(fill),
                });
            }
            start = end;
        }
        paints
    }

    /// Returns the palette entry that a palette layer names, if the palettes have it.
    fn palette_color(&self, index: Option<i64>) -> Option<PaintColor> {
        let index = index?;
        if index == FOREGROUND_INDEX {
            return Some(PaintColor::Foreground);
        }
        let index = u16::try_from(index).ok()?;
        (usize::from(index) < self.declared_len).then_some(PaintColor::Palette { index })
    }

    /// Returns the glyph that a paint layer fills, creating one when no glyph
    /// of the font already has exactly the layer's outline.
    fn layer_glyph(&mut self, base: &GlyphsGlyph, shapes: &[&Shape], origin: Origin) -> GlyphId {
        if let [Shape::Component(component)] = shapes
            && self.can_reuse(base, component)
        {
            return self.glyph_ids[component.name.as_str()];
        }
        self.synthesize(base, origin)
    }

    /// Whether a component is the whole of its layer, so its glyph can be the layer.
    ///
    /// A paint layer must not be its own base glyph or another colour glyph.
    fn can_reuse(&self, base: &GlyphsGlyph, component: &Component) -> bool {
        let name = component.name.as_str();
        let unmoved = component.transform.as_coeffs() == [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        unmoved
            && component.smart_component_values.is_empty()
            && name != base.name.as_str()
            && self.glyph_ids.contains_key(name)
            && !self.colour_bases.contains(name)
    }

    fn synthesize(&mut self, base: &GlyphsGlyph, origin: Origin) -> GlyphId {
        let number = match origin {
            Origin::PaletteLayer(index) | Origin::ColorShape(index) => index + 1,
        };
        let stem = format!("{}.color{number}", base.name);
        let mut name = stem.clone();
        let mut attempt = 1;
        while self.glyph_ids.contains_key(&name) {
            name = format!("{stem}.{attempt}");
            attempt += 1;
        }
        let id = self.ids.glyph_id(&name);
        self.glyph_ids.insert(name.clone(), id);

        let layers = self
            .source
            .masters
            .iter()
            .filter(|master| origin_layer(base, origin, &master.id).is_some())
            .filter_map(|master| Some((master.id.clone(), self.sources.master_source(&master.id)?)))
            .collect();
        self.synthesized.push(SynthesizedGlyph {
            id,
            name,
            base: base.name.to_string(),
            origin,
            layers,
        });
        id
    }

    /// Returns the paint of a shape with literal colour attributes.
    fn fill(&mut self, attributes: &ShapeAttributes, bounds: Option<Bounds>) -> Paint {
        if let Some(paint) = self.gradient(attributes, bounds) {
            return paint;
        }
        let color = match attributes.fill_color {
            Some(color) => self.literal(color),
            None => PaintColor::Foreground,
        };
        Paint::Solid { color, alpha: 1.0 }
    }

    /// Converts a Glyphs gradient, whose points are fractions of the run bounds.
    ///
    /// A circle gradient keeps its centre; Shift derives its radius from the
    /// bounds, as Glyphs does.
    fn gradient(&mut self, attributes: &ShapeAttributes, bounds: Option<Bounds>) -> Option<Paint> {
        let gradient = attributes.gradient.as_ref()?;
        let mut stops = gradient
            .colors
            .iter()
            .map(|stop| ColorStop {
                offset: stop.stop_offset.into_inner().clamp(0.0, 1.0),
                color: self.literal(stop.color),
                alpha: 1.0,
            })
            .collect::<Vec<_>>();
        if stops.is_empty() {
            return None;
        }
        stops.sort_by(|left, right| left.offset.total_cmp(&right.offset));

        let (min_x, min_y, max_x, max_y) = bounds.unwrap_or((0.0, 0.0, 1.0, 1.0));
        let point = |fractions: &[OrderedFloat<f64>]| {
            let fraction = |index: usize| fractions.get(index).map_or(0.0, |v| v.into_inner());
            ColorPoint::new(
                min_x + fraction(0) * (max_x - min_x),
                min_y + fraction(1) * (max_y - min_y),
            )
        };
        let start = point(&gradient.start);
        match gradient.style.as_str() {
            "circle" => Some(Paint::RadialGradient {
                stops,
                c0: start,
                r0: None,
                c1: start,
                r1: None,
            }),
            style => {
                if !matches!(style, "" | "linear") {
                    self.unsupported_gradients += 1;
                }
                Some(Paint::LinearGradient {
                    stops,
                    p0: start,
                    p1: point(&gradient.end),
                    p2: None,
                })
            }
        }
    }

    /// Returns the palette entry of a literal colour, appending it when it is new.
    fn literal(&mut self, color: Color) -> PaintColor {
        let rgba = rgba(color);
        if let Some(index) = self.literals.get(&rgba) {
            return PaintColor::Palette { index: *index };
        }
        if self.palettes.is_empty() {
            self.palettes.push(ColorPalette {
                name: None,
                colors: Vec::new(),
            });
        }
        let length = self.palettes[0].colors.len();
        let Some(index) = u16::try_from(length)
            .ok()
            .filter(|_| length < MAX_PALETTE_ENTRIES)
        else {
            self.overflowed_colours += 1;
            return PaintColor::Foreground;
        };
        for palette in &mut self.palettes {
            palette.colors.push(rgba);
        }
        self.literals.insert(rgba, index);
        PaintColor::Palette { index }
    }

    /// Measures a run of shapes in the default master, as Glyphs does for a gradient.
    fn run_bounds(&self, run: &[Shape]) -> Option<Bounds> {
        let default_source = self.sources.master_source(self.default_master)?;
        let mut root = GlyphLayer::with_width(LayerId::new(), default_source, 0.0);
        let mut pending = Vec::new();
        for shape in run {
            add_shape(&mut root, shape, self.glyph_ids).ok()?;
            if let Shape::Component(component) = shape {
                pending.push(component.name.to_string());
            }
        }

        let mut layers: HashMap<GlyphId, GlyphLayer> = HashMap::new();
        while let Some(name) = pending.pop() {
            let id = *self.glyph_ids.get(&name)?;
            if layers.contains_key(&id) {
                continue;
            }
            let glyph = self.source.glyphs.get(name.as_str())?;
            let layer = glyph
                .layers
                .iter()
                .find(|layer| layer.is_master() && layer.layer_id == self.default_master)?;
            let mut converted =
                GlyphLayer::with_width(LayerId::new(), default_source, layer.width.into_inner());
            for shape in &layer.shapes {
                add_shape(&mut converted, shape, self.glyph_ids).ok()?;
                if let Shape::Component(component) = shape {
                    pending.push(component.name.to_string());
                }
            }
            layers.insert(id, converted);
        }

        let root_id = GlyphId::new();
        layers.insert(root_id, root);
        let contours = resolved_contours_from_layers(root_id, &layers).ok()?;
        resolved_contours_bbox(&contours)
    }

    fn report(&self, glyphs: &[ColorGlyph], losses: &mut Vec<ImportLoss>) {
        let mut lose = |kind, message: String| losses.push(ImportLoss { kind, message });
        if self.trimmed_palettes {
            lose(
                ImportLossKind::Approximated,
                "Glyphs palettes differ in length; every palette was cut to the shortest."
                    .to_string(),
            );
        }
        if self.omitted_layers > 0 {
            lose(
                ImportLossKind::Omitted,
                format!(
                    "{} Glyphs palette layers name a palette entry that the font does not have and were omitted.",
                    self.omitted_layers
                ),
            );
        }
        if self.unsupported_gradients > 0 {
            lose(
                ImportLossKind::Approximated,
                format!(
                    "{} Glyphs gradients have a type that Shift does not represent and were imported as linear gradients.",
                    self.unsupported_gradients
                ),
            );
        }
        if self.overflowed_colours > 0 {
            lose(
                ImportLossKind::Approximated,
                format!(
                    "{} Glyphs colours did not fit in a palette and were imported as the text colour.",
                    self.overflowed_colours
                ),
            );
        }
        if !glyphs.is_empty() && self.source.masters.len() > 1 {
            lose(
                ImportLossKind::Approximated,
                "Shift keeps one paint graph per glyph; Glyphs colour layers were read from the default master only."
                    .to_string(),
            );
        }
    }
}

/// Reads the `Color Palettes` parameter, cutting palettes to a common length.
///
/// Returns the palettes and whether any had to be cut.
fn declared_palettes(source: &GlyphsFont) -> (Vec<ColorPalette>, bool) {
    let Some(declared) = source.color_palettes() else {
        return (Vec::new(), false);
    };
    let shortest = declared.iter().map(Vec::len).min().unwrap_or(0);
    let trimmed = declared.iter().any(|palette| palette.len() != shortest);
    let palettes = declared
        .iter()
        .map(|palette| ColorPalette {
            name: None,
            colors: palette.iter().take(shortest).copied().map(rgba).collect(),
        })
        .collect();
    (palettes, trimmed)
}

fn rgba(color: Color) -> Rgba {
    let channel = |value: i64| u8::try_from(value.clamp(0, 255)).unwrap_or(u8::MAX);
    Rgba::new(
        channel(color.r),
        channel(color.g),
        channel(color.b),
        channel(color.a),
    )
}

/// Lists the palette layers of a master in the order the glyph stores them.
fn palette_layers<'a>(glyph: &'a GlyphsGlyph, master: &str) -> Vec<&'a GlyphsLayer> {
    glyph
        .layers
        .iter()
        .filter(|layer| {
            layer.attributes.color_palette.is_some()
                && layer.attributes.coordinates.is_empty()
                && layer.master_id() == master
        })
        .collect()
}

/// Returns the layer of a master whose shapes carry literal colours.
fn colour_layer<'a>(glyph: &'a GlyphsGlyph, master: &str) -> Option<&'a GlyphsLayer> {
    glyph.layers.iter().find(|layer| {
        layer.attributes.color
            && layer.attributes.color_palette.is_none()
            && layer.attributes.coordinates.is_empty()
            && layer.master_id() == master
    })
}

/// Returns the layer and shapes that a new layer glyph takes from one master.
fn origin_layer<'a>(
    base: &'a GlyphsGlyph,
    origin: Origin,
    master: &str,
) -> Option<(&'a GlyphsLayer, Vec<&'a Shape>)> {
    match origin {
        Origin::PaletteLayer(index) => {
            let layer = palette_layers(base, master).into_iter().nth(index)?;
            (!layer.shapes.is_empty()).then(|| (layer, layer.shapes.iter().collect()))
        }
        Origin::ColorShape(index) => {
            let layer = colour_layer(base, master)?;
            let shape = layer.shapes.get(index)?;
            Some((layer, vec![shape]))
        }
    }
}
