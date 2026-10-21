//! Adds `COLR` and `CPAL` colour tables to a compiled TrueType binary.
//!
//! fontc models colour as paints that carry literal colours, looks each colour
//! up again in the first palette, and writes every stop fully opaque. That
//! loses palette identity, cannot express opacity, and confuses entries that
//! share a colour in one palette but differ in another. Shift therefore keeps
//! colour out of fontc and writes both tables itself, from palette indices.
//!
//! Colour glyphs are also written as an OpenType `SVG ` table (see `svg`), for
//! renderers that read SVG instead of `COLR`.
//!
//! A colour glyph whose layers are all opaque solid fills becomes a `COLR`
//! version 0 record, which the oldest colour renderers read. Every other
//! colour glyph becomes a version 1 paint graph with a clip box.

use std::collections::HashMap;

use crate::color_tables::{ColorTable, ColorTables};
use crate::sbix::sbix_table;
use crate::svg::svg_table;
use shift_font::{ColorData, ColorGlyph, GlyphId, Paint, PaintColor};
use skrifa::{FontRef as SkrifaFont, MetadataProvider};
use write_fonts::FontBuilder;
use write_fonts::OffsetMarker;
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::read::tables::glyf::Glyph as GlyfGlyph;
use write_fonts::read::{FontRef, TableProvider};
use write_fonts::tables::colr::{
    BaseGlyph, BaseGlyphList, BaseGlyphPaint, Clip, ClipBox, ClipList, ColorLine, ColorStop, Colr,
    Extend, Layer, LayerList, Paint as ColrPaint, PaintColrLayers, PaintGlyph, PaintLinearGradient,
    PaintRadialGradient, PaintSolid,
};
use write_fonts::tables::name::{Name, NameRecord};
use write_fonts::types::{F2Dot14, FWord, GlyphId16, NameId, Tag};

/// Palette index that selects the text foreground colour.
pub(crate) const FOREGROUND: u16 = 0xFFFF;

/// Label that `CPAL` uses for a palette without a name.
const NO_LABEL: u16 = 0xFFFF;

/// First name ID that a font may use for its own strings.
const FIRST_FONT_NAME_ID: u16 = 256;

/// Last name ID that a font may use for its own strings.
const LAST_FONT_NAME_ID: u16 = 32767;

/// Windows platform, Unicode BMP encoding and US English language of a `name` record.
const WINDOWS_PLATFORM: u16 = 3;
const UNICODE_BMP_ENCODING: u16 = 1;
const US_ENGLISH: u16 = 0x0409;

/// Describes a failure to add colour tables to a compiled font.
#[derive(Debug, thiserror::Error)]
pub enum ColorError {
    #[error("cannot read the compiled font: {message}")]
    ReadFont { message: String },

    #[error("colour glyph refers to glyph {name:?}, which the compiled font does not contain")]
    MissingGlyph { name: String },

    #[error("colour data is too large for the colour tables: {message}")]
    TooLarge { message: String },

    #[error("cannot rebuild the font tables: {message}")]
    Rebuild { message: String },
}

fn read_error(error: &impl std::fmt::Display) -> ColorError {
    ColorError::ReadFont {
        message: error.to_string(),
    }
}

/// Inserts `CPAL` and `COLR` tables for `color` into the compiled `ttf`.
///
/// `names` maps each authored glyph identity to the name fontc compiled it
/// under, and `tables` chooses the tables that carry the colour glyphs.
/// Palettes are written when `COLR` or `SVG ` is chosen. Returns the input
/// unchanged when the font has no colour data or no table is chosen.
///
/// # Errors
///
/// Returns [`ColorError`] when a paint names a glyph missing from the compiled
/// font, a table overflows its format, or the font cannot be read or rebuilt.
pub fn add_color_tables(
    ttf: &[u8],
    color: &ColorData,
    names: &HashMap<GlyphId, String>,
    tables: &ColorTables,
) -> Result<Vec<u8>, ColorError> {
    if color.is_empty() || tables.is_empty() {
        return Ok(ttf.to_vec());
    }

    let font = FontRef::new(ttf).map_err(|error| read_error(&error))?;
    let compiled = CompiledGlyphs::read(ttf, &font)?;
    let resolve = |glyph: &GlyphId| -> Result<GlyphId16, ColorError> {
        let name = names
            .get(glyph)
            .map_or_else(|| glyph.to_string(), Clone::clone);
        compiled
            .ids
            .get(&name)
            .copied()
            .ok_or(ColorError::MissingGlyph { name })
    };

    let mut builder = FontBuilder::new();
    let mut rebuilt = false;

    let wants_palettes = tables.contains(ColorTable::Colr) || tables.contains(ColorTable::Svg);
    if color.palette_len() > 0 && wants_palettes {
        let labels = PaletteLabels::allocate(color, &font)?;
        builder.add_raw(Tag::new(b"CPAL"), cpal(color, &labels.ids)?);
        if let Some(name) = labels.name {
            builder
                .add_table(&name)
                .map_err(|error| ColorError::Rebuild {
                    message: error.to_string(),
                })?;
        }
        rebuilt = true;
    }

    if !color.glyphs.is_empty() {
        if tables.contains(ColorTable::Colr) {
            builder
                .add_table(&colr(color, &resolve, &compiled)?)
                .map_err(|error| ColorError::Rebuild {
                    message: error.to_string(),
                })?;
            rebuilt = true;
        }
        if tables.contains(ColorTable::Svg)
            && let Some(svg) = svg_table(ttf, color, &resolve, &compiled)?
        {
            builder.add_raw(Tag::new(b"SVG "), svg);
            rebuilt = true;
        }
        if tables.contains(ColorTable::Sbix)
            && let Some(sbix) = sbix_table(ttf, color, &resolve, &compiled)?
        {
            builder.add_raw(Tag::new(b"sbix"), sbix);
            rebuilt = true;
        }
    }

    if !rebuilt {
        return Ok(ttf.to_vec());
    }
    Ok(builder.copy_missing_tables(font).build())
}

/// Glyph ids and bounds of the compiled font.
pub(crate) struct CompiledGlyphs<'a> {
    ids: HashMap<String, GlyphId16>,
    font: FontRef<'a>,
}

impl<'a> CompiledGlyphs<'a> {
    fn read(bytes: &'a [u8], font: &FontRef<'a>) -> Result<Self, ColorError> {
        let skrifa = SkrifaFont::new(bytes).map_err(|error| read_error(&error))?;
        let mut ids = HashMap::new();
        for (glyph_id, name) in skrifa.glyph_names().iter() {
            let id = u16::try_from(glyph_id.to_u32())
                .map(GlyphId16::new)
                .map_err(|_| ColorError::TooLarge {
                    message: "more than 65 535 glyphs".to_string(),
                })?;
            ids.insert(name.as_str().to_string(), id);
        }
        Ok(Self {
            ids,
            font: font.clone(),
        })
    }

    /// Returns `(x_min, y_min, x_max, y_max)` of a glyph's outline, if it has one.
    pub(crate) fn bounds(&self, glyph: GlyphId16) -> Option<[i16; 4]> {
        let loca = self.font.loca(None).ok()?;
        let glyf = self.font.glyf().ok()?;
        let outline = loca.get_glyf(glyph.into(), &glyf).ok()??;
        Some(match outline {
            GlyfGlyph::Simple(simple) => [
                simple.x_min(),
                simple.y_min(),
                simple.x_max(),
                simple.y_max(),
            ],
            GlyfGlyph::Composite(composite) => [
                composite.x_min(),
                composite.y_min(),
                composite.x_max(),
                composite.y_max(),
            ],
        })
    }
}

/// Name IDs of the palette labels, and the `name` table that defines them.
struct PaletteLabels {
    /// One name ID per palette, [`NO_LABEL`] for a palette without a name.
    ids: Vec<u16>,
    /// The font's `name` table with the labels added, or `None` when no
    /// palette has a name and the table stays as compiled.
    name: Option<Name>,
}

impl PaletteLabels {
    /// Adds a `name` record for each distinct palette name, from the first
    /// font-specific name ID that the compiled `name` table leaves free.
    ///
    /// # Errors
    ///
    /// Returns [`ColorError`] when the `name` table cannot be read or the
    /// font-specific name IDs run out.
    fn allocate(color: &ColorData, font: &FontRef<'_>) -> Result<Self, ColorError> {
        let named = color
            .palettes
            .iter()
            .any(|palette| palette.name.as_deref().is_some_and(|name| !name.is_empty()));
        if !named {
            return Ok(Self {
                ids: vec![NO_LABEL; color.palettes.len()],
                name: None,
            });
        }

        let mut name: Name = match font.name() {
            Ok(table) => table.to_owned_table(),
            Err(_) => Name::default(),
        };
        let mut next = name
            .name_record
            .iter()
            .map(|record| record.name_id.to_u16().saturating_add(1))
            .fold(FIRST_FONT_NAME_ID, u16::max);
        let mut assigned: HashMap<&str, u16> = HashMap::new();
        let mut ids = Vec::with_capacity(color.palettes.len());

        for palette in &color.palettes {
            let Some(label) = palette.name.as_deref().filter(|label| !label.is_empty()) else {
                ids.push(NO_LABEL);
                continue;
            };
            if let Some(id) = assigned.get(label) {
                ids.push(*id);
                continue;
            }
            if next > LAST_FONT_NAME_ID {
                return Err(ColorError::TooLarge {
                    message: "no font-specific name IDs left for the palette names".to_string(),
                });
            }
            name.name_record.push(NameRecord::new(
                WINDOWS_PLATFORM,
                UNICODE_BMP_ENCODING,
                US_ENGLISH,
                NameId::new(next),
                OffsetMarker::new(label.to_string()),
            ));
            assigned.insert(label, next);
            ids.push(next);
            next += 1;
        }

        name.name_record.sort_by_key(|record| {
            (
                record.platform_id,
                record.encoding_id,
                record.language_id,
                record.name_id,
            )
        });
        Ok(Self {
            ids,
            name: Some(name),
        })
    }
}

/// Serialises the `CPAL` table.
///
/// `write-fonts` always writes version 0 and drops the label arrays, so the
/// table is written here. It is version 1 when any palette has a label, and
/// version 0 otherwise.
fn cpal(color: &ColorData, labels: &[u16]) -> Result<Vec<u8>, ColorError> {
    let too_large = |what: &str| ColorError::TooLarge {
        message: format!("too many {what}"),
    };
    let per_palette = color.palette_len();
    let palettes = color.palettes.len();
    let records: Vec<_> = color
        .palettes
        .iter()
        .flat_map(|palette| palette.colors.iter())
        .collect();
    let labelled = labels.iter().any(|label| *label != NO_LABEL);

    let entries = u16::try_from(per_palette).map_err(|_| too_large("palette entries"))?;
    let palette_count = u16::try_from(palettes).map_err(|_| too_large("palettes"))?;
    let record_count = u16::try_from(records.len()).map_err(|_| too_large("colour records"))?;

    // Version 1 adds three 32-bit offsets after the palette indices
    let header_len = 12 + 2 * palettes + if labelled { 12 } else { 0 };
    let records_offset = u32::try_from(header_len).map_err(|_| too_large("palettes"))?;
    let labels_offset = records_offset
        .checked_add(u32::from(record_count) * 4)
        .ok_or_else(|| too_large("colour records"))?;

    let mut bytes = Vec::with_capacity(header_len + records.len() * 4 + palettes * 2);
    bytes.extend(u16::from(labelled).to_be_bytes());
    bytes.extend(entries.to_be_bytes());
    bytes.extend(palette_count.to_be_bytes());
    bytes.extend(record_count.to_be_bytes());
    bytes.extend(records_offset.to_be_bytes());
    for index in 0..palettes {
        let first = u16::try_from(index * per_palette).map_err(|_| too_large("colour records"))?;
        bytes.extend(first.to_be_bytes());
    }
    if labelled {
        // Palette types and entry labels are absent
        bytes.extend(0u32.to_be_bytes());
        bytes.extend(labels_offset.to_be_bytes());
        bytes.extend(0u32.to_be_bytes());
    }
    for c in records {
        bytes.extend([c.b, c.g, c.r, c.a]);
    }
    if labelled {
        for label in labels {
            bytes.extend(label.to_be_bytes());
        }
    }
    Ok(bytes)
}

/// One layer of a colour glyph with its compiled glyph.
pub(crate) struct ResolvedLayer<'a> {
    pub(crate) glyph: GlyphId16,
    pub(crate) fill: &'a Paint,
}

pub(crate) fn layers_of<'a>(
    paint: &'a Paint,
    resolve: &impl Fn(&GlyphId) -> Result<GlyphId16, ColorError>,
) -> Result<Vec<ResolvedLayer<'a>>, ColorError> {
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
        .map(|(glyph, fill)| {
            Ok(ResolvedLayer {
                glyph: resolve(glyph)?,
                fill,
            })
        })
        .collect()
}

fn is_opaque_solid(fill: &Paint) -> bool {
    matches!(fill, Paint::Solid { alpha, .. } if *alpha >= 1.0)
}

fn palette_index(color: PaintColor) -> u16 {
    match color {
        PaintColor::Foreground => FOREGROUND,
        PaintColor::Palette { index } => index,
    }
}

fn colr(
    color: &ColorData,
    resolve: &impl Fn(&GlyphId) -> Result<GlyphId16, ColorError>,
    compiled: &CompiledGlyphs<'_>,
) -> Result<Colr, ColorError> {
    let mut entries: Vec<(GlyphId16, &ColorGlyph)> = color
        .glyphs
        .iter()
        .map(|entry| Ok((resolve(&entry.glyph)?, entry)))
        .collect::<Result<_, ColorError>>()?;
    entries.sort_by_key(|(glyph, _)| *glyph);

    let mut v0_glyphs = Vec::new();
    let mut v0_layers: Vec<Layer> = Vec::new();
    let mut v1_glyphs = Vec::new();
    let mut v1_layers = LayerList::default();
    let mut clips: Vec<Clip> = Vec::new();

    for (base, entry) in entries {
        let layers = layers_of(&entry.paint, resolve)?;
        if layers.iter().all(|layer| is_opaque_solid(layer.fill)) {
            let first = u16::try_from(v0_layers.len()).map_err(|_| ColorError::TooLarge {
                message: "more than 65 535 version 0 layers".to_string(),
            })?;
            for layer in &layers {
                let Paint::Solid { color, .. } = layer.fill else {
                    continue;
                };
                v0_layers.push(Layer::new(layer.glyph, palette_index(*color)));
            }
            v0_glyphs.push(BaseGlyph::new(base, first, layers.len() as u16));
            continue;
        }

        let first = v1_layers.paints.len();
        let mut bounds: Option<[i16; 4]> = None;
        for layer in &layers {
            let layer_bounds = compiled.bounds(layer.glyph);
            bounds = union(bounds, layer_bounds);
            let fill = to_colr_fill(layer.fill, layer_bounds)?;
            v1_layers.paints.push(
                ColrPaint::Glyph(PaintGlyph {
                    paint: fill.into(),
                    glyph_id: layer.glyph,
                })
                .into(),
            );
        }
        let layer_count = u8::try_from(layers.len()).map_err(|_| ColorError::TooLarge {
            message: "more than 255 layers in one colour glyph".to_string(),
        })?;
        v1_glyphs.push(BaseGlyphPaint::new(
            base,
            ColrPaint::ColrLayers(PaintColrLayers::new(
                layer_count,
                u32::try_from(first).map_err(|_| ColorError::TooLarge {
                    message: "too many colour layers".to_string(),
                })?,
            )),
        ));
        if let Some(bounds) = bounds {
            push_clip(&mut clips, base, bounds);
        }
    }

    let v0_count = u16::try_from(v0_glyphs.len()).map_err(|_| ColorError::TooLarge {
        message: "more than 65 535 version 0 colour glyphs".to_string(),
    })?;
    let v0_layer_count = u16::try_from(v0_layers.len()).map_err(|_| ColorError::TooLarge {
        message: "more than 65 535 version 0 layers".to_string(),
    })?;
    let mut table = Colr::new(
        v0_count,
        (!v0_glyphs.is_empty()).then_some(v0_glyphs),
        (!v0_layers.is_empty()).then_some(v0_layers),
        v0_layer_count,
    );
    if !v1_glyphs.is_empty() {
        table.base_glyph_list = BaseGlyphList::new(v1_glyphs.len() as u32, v1_glyphs).into();
        v1_layers.num_layers = v1_layers.paints.len() as u32;
        table.layer_list = v1_layers.into();
    }
    if !clips.is_empty() {
        table.clip_list = ClipList::new(1, clips.len() as u32, clips).into();
    }
    Ok(table)
}

fn union(current: Option<[i16; 4]>, next: Option<[i16; 4]>) -> Option<[i16; 4]> {
    match (current, next) {
        (Some(a), Some(b)) => Some([
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]),
        (a, b) => a.or(b),
    }
}

/// Appends a clip box, extending the previous clip when it is the next glyph
/// with the same box.
fn push_clip(clips: &mut Vec<Clip>, glyph: GlyphId16, bounds: [i16; 4]) {
    let clip_box = ClipBox::format_1(
        FWord::new(bounds[0]),
        FWord::new(bounds[1]),
        FWord::new(bounds[2]),
        FWord::new(bounds[3]),
    );
    if let Some(last) = clips.last_mut()
        && last.end_glyph_id.to_u32() + 1 == glyph.to_u32()
        && *last.clip_box == clip_box
    {
        last.end_glyph_id = glyph;
        return;
    }
    clips.push(Clip::new(glyph, glyph, clip_box));
}

fn f2dot14(value: f64) -> F2Dot14 {
    F2Dot14::from_f32(value.clamp(0.0, 1.0) as f32)
}

fn fword(value: f64) -> FWord {
    FWord::new(
        value
            .round()
            .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16,
    )
}

fn color_line(stops: &[shift_font::ColorStop]) -> ColorLine {
    ColorLine::new(
        Extend::Pad,
        stops.len() as u16,
        stops
            .iter()
            .map(|stop| {
                ColorStop::new(
                    f2dot14(stop.offset),
                    palette_index(stop.color),
                    f2dot14(stop.alpha),
                )
            })
            .collect(),
    )
}

fn to_colr_fill(fill: &Paint, layer_bounds: Option<[i16; 4]>) -> Result<ColrPaint, ColorError> {
    Ok(match fill {
        Paint::Solid { color, alpha } => ColrPaint::Solid(PaintSolid {
            palette_index: palette_index(*color),
            alpha: f2dot14(*alpha),
        }),
        Paint::LinearGradient { stops, p0, p1, p2 } => {
            // Without a rotation point, the gradient runs perpendicular to p0-p1
            let (x2, y2) = p2.map_or((p0.x + (p1.y - p0.y), p0.y - (p1.x - p0.x)), |p| (p.x, p.y));
            ColrPaint::LinearGradient(PaintLinearGradient::new(
                color_line(stops),
                fword(p0.x),
                fword(p0.y),
                fword(p1.x),
                fword(p1.y),
                fword(x2),
                fword(y2),
            ))
        }
        Paint::RadialGradient {
            stops,
            c0,
            r0,
            c1,
            r1,
        } => {
            let outer = r1.unwrap_or_else(|| farthest_corner(*c1, layer_bounds));
            ColrPaint::RadialGradient(PaintRadialGradient::new(
                color_line(stops),
                fword(c0.x),
                fword(c0.y),
                radius(r0.unwrap_or(0.0)),
                fword(c1.x),
                fword(c1.y),
                radius(outer),
            ))
        }
        Paint::Glyph { .. } | Paint::Layers { .. } => {
            return Err(ColorError::Rebuild {
                message: "a fill cannot be a glyph or layer paint".to_string(),
            });
        }
    })
}

fn radius(value: f64) -> write_fonts::types::UfWord {
    write_fonts::types::UfWord::new(value.round().clamp(0.0, f64::from(u16::MAX)) as u16)
}

/// Distance from `center` to the farthest corner of the layer's bounds.
pub(crate) fn farthest_corner(center: shift_font::ColorPoint, bounds: Option<[i16; 4]>) -> f64 {
    let Some([x0, y0, x1, y1]) = bounds else {
        return 0.0;
    };
    [(x0, y0), (x0, y1), (x1, y0), (x1, y1)]
        .into_iter()
        .map(|(x, y)| (f64::from(x) - center.x).hypot(f64::from(y) - center.y))
        .fold(0.0, f64::max)
}
