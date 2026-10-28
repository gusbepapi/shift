//! Writes a Shift font as a Glyphs 3 source file.
//!
//! The output keeps what Shift models: masters with their axis values and metrics, glyphs with
//! Unicode values, outlines, components and anchors for every master, kerning with its groups,
//! feature code, named instances, the glyphs that are not exported, and colour palettes with
//! colour glyphs (see the `color` module for what Glyphs can hold). Layers of sources that
//! are not masters, guidelines, hints and the remaining font information are not written.

use std::collections::HashMap;
use std::fmt::Write;

use shift_font::{
    Axis, AxisMapping, Component, Contour, Glyph, GlyphId, Kerning, KerningPosition, KerningSide,
    MetricKind, Point, PointType, Source,
};

mod color;

use crate::errors::{FormatBackendError, FormatBackendResult};
use crate::traits::FontView;

/// Version of the Glyphs application that the file claims to come from.
const APP_VERSION: &str = "3343";

const FIRST_GROUP_PREFIX: &str = "@MMK_L_";
const SECOND_GROUP_PREFIX: &str = "@MMK_R_";

// A matrix is read as scale and rotation when the rest is below this
const MATRIX_TOLERANCE: f64 = 1e-6;

/// Writes Glyphs 3 source files.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlyphsWriter;

impl GlyphsWriter {
    pub fn new() -> Self {
        Self
    }

    /// Renders `font` as the text of a `.glyphs` file.
    ///
    /// # Errors
    ///
    /// Returns [`FormatBackendError::Glyphs`] when the font has no default master or a named
    /// instance cannot be placed in the design space.
    #[allow(
        clippy::unused_self,
        reason = "a writer method; the writer is the entry point callers hold"
    )]
    pub fn render(self, font: &impl FontView) -> FormatBackendResult<String> {
        let masters = Masters::of(font)?;
        let mut root = Dict::new();
        root.put(".appVersion", Value::string(APP_VERSION));
        root.put(".formatVersion", Value::number(3.0));

        if !font.axes().is_empty() {
            root.put(
                "axes",
                Value::Array(font.axes().iter().map(axis_value).collect()),
            );
        }

        let mut parameters = Vec::new();
        let default_id = masters.default().id.clone();
        parameters.push(parameter(
            "Variable Font Origin",
            Value::string(&default_id),
        ));
        if let Some(mappings) = axis_mappings_value(font.axes(), font.axis_mappings()) {
            parameters.push(parameter("Axis Mappings", mappings));
        }
        if let Some(palettes) = color::palettes_parameter(font.color()) {
            parameters.push(palettes);
        }
        root.put("customParameters", Value::Array(parameters));

        let metadata = font.metadata();
        put_text(&mut root, "copyright", metadata.copyright.as_deref());
        put_text(&mut root, "designer", metadata.designer.as_deref());
        put_text(&mut root, "designerURL", metadata.designer_url.as_deref());
        root.put(
            "familyName",
            Value::string(metadata.family_name.as_deref().unwrap_or("Untitled")),
        );

        let metric_kinds = metric_kinds(&masters);
        root.put(
            "fontMaster",
            Value::Array(
                masters
                    .all
                    .iter()
                    .map(|master| master_value(font, master, &metric_kinds))
                    .collect(),
            ),
        );

        let glyph_names: HashMap<GlyphId, String> = font
            .glyphs()
            .map(|glyph| (glyph.id(), glyph.name().to_string()))
            .collect();
        let kerning = font.kerning();
        let skipped = font.lib().skipped_export_glyphs();
        let colours = color::ColorWriter::new(font, &glyph_names);
        root.put(
            "glyphs",
            Value::Array(
                font.glyphs()
                    .map(|glyph| {
                        glyph_value(
                            glyph,
                            &masters,
                            kerning,
                            skipped.contains(glyph.name()),
                            &colours,
                        )
                    })
                    .collect(),
            ),
        );

        let instances = font
            .named_instances()
            .iter()
            .map(|instance| {
                let design = shift_font::variation::map_location(
                    instance.location(),
                    font.axes(),
                    font.axis_mappings(),
                )
                .map_err(|error| {
                    FormatBackendError::Glyphs(format!(
                        "cannot place instance {:?} in the design space: {error}",
                        instance.name()
                    ))
                })?;
                let mut dict = Dict::new();
                dict.put("name", Value::string(instance.name()));
                if !font.axes().is_empty() {
                    dict.put(
                        "axesValues",
                        axes_values(font.axes(), |axis| design.value_on(axis)),
                    );
                }
                Ok(Value::Dict(dict))
            })
            .collect::<FormatBackendResult<Vec<_>>>()?;
        if !instances.is_empty() {
            root.put("instances", Value::Array(instances));
        }

        if let Some(pairs) = kerning_value(kerning, &masters, &glyph_names) {
            root.put("kerningLTR", pairs);
        }
        if let Some(code) = font
            .features()
            .fea_source()
            .filter(|code| !code.trim().is_empty())
        {
            let mut prefix = Dict::new();
            prefix.put("code", Value::string(code));
            prefix.put("name", Value::string("Shift"));
            root.put("featurePrefixes", Value::Array(vec![Value::Dict(prefix)]));
        }

        let mut metrics = Vec::new();
        for kind in &metric_kinds {
            let mut dict = Dict::new();
            dict.put("type", Value::string(kind.glyphs_name()));
            metrics.push(Value::Dict(dict));
        }
        root.put("metrics", Value::Array(metrics));

        root.put("unitsPerEm", Value::number(font.metrics().units_per_em));
        if let Some(major) = metadata.version_major {
            root.put("versionMajor", Value::number(f64::from(major)));
        }
        if let Some(minor) = metadata.version_minor {
            root.put("versionMinor", Value::number(f64::from(minor)));
        }

        let mut out = String::new();
        Value::Dict(root).render(&mut out);
        out.push('\n');
        Ok(out)
    }
}

/// A master as the file names it.
struct Master<'a> {
    id: String,
    source: &'a Source,
}

struct Masters<'a> {
    all: Vec<Master<'a>>,
    default: usize,
}

impl<'a> Masters<'a> {
    fn of(font: &'a impl FontView) -> FormatBackendResult<Self> {
        let default_source = font
            .default_source_id()
            .ok_or_else(|| FormatBackendError::Glyphs("the font has no default source".into()))?;
        let all: Vec<Master<'a>> = font
            .sources()
            .iter()
            .filter(|source| source.is_master())
            .enumerate()
            .map(|(index, source)| Master {
                id: format!("m{:02}", index + 1),
                source,
            })
            .collect();
        let default = all
            .iter()
            .position(|master| master.source.id() == default_source)
            .ok_or_else(|| {
                FormatBackendError::Glyphs("the default source is not a master".into())
            })?;
        Ok(Self { all, default })
    }

    fn default(&self) -> &Master<'a> {
        &self.all[self.default]
    }
}

/// Metric rows of the file, in the order every master lists their values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Kind(MetricKind),
    ItalicAngle,
}

impl Row {
    fn glyphs_name(self) -> &'static str {
        match self {
            Self::Kind(MetricKind::Ascender) => "ascender",
            Self::Kind(MetricKind::CapHeight) => "cap height",
            Self::Kind(MetricKind::XHeight) => "x-height",
            Self::Kind(MetricKind::Descender) => "descender",
            Self::Kind(_) => "baseline",
            Self::ItalicAngle => "italic angle",
        }
    }
}

fn metric_kinds(masters: &Masters<'_>) -> Vec<Row> {
    let mut rows = vec![
        Row::Kind(MetricKind::Ascender),
        Row::Kind(MetricKind::CapHeight),
        Row::Kind(MetricKind::XHeight),
        Row::Kind(MetricKind::Baseline),
        Row::Kind(MetricKind::Descender),
    ];
    if masters.all.iter().any(|master| {
        master
            .source
            .italic_angle()
            .is_some_and(|angle| angle != 0.0)
    }) {
        rows.push(Row::ItalicAngle);
    }
    rows
}

fn master_value(font: &impl FontView, master: &Master<'_>, rows: &[Row]) -> Value {
    let mut dict = Dict::new();
    if !font.axes().is_empty() {
        dict.put(
            "axesValues",
            axes_values(font.axes(), |axis| master.source.location().value_on(axis)),
        );
    }
    dict.put("id", Value::string(&master.id));
    let values = rows
        .iter()
        .map(|row| {
            let mut value = Dict::new();
            match row {
                Row::ItalicAngle => {
                    if let Some(angle) = master.source.italic_angle() {
                        value.put("pos", Value::number(angle));
                    }
                }
                Row::Kind(kind) => {
                    let metric = font
                        .metric_definitions()
                        .iter()
                        .find(|definition| definition.kind() == *kind)
                        .and_then(|definition| master.source.metric_value(definition.id()));
                    if let Some(metric) = metric {
                        let (position, overshoot) = (metric.position, metric.overshoot);
                        if overshoot != 0.0 {
                            value.put("over", Value::number(overshoot));
                        }
                        if position != 0.0 {
                            value.put("pos", Value::number(position));
                        }
                    }
                }
            }
            Value::Dict(value)
        })
        .collect();
    dict.put("metricValues", Value::Array(values));
    dict.put("name", Value::string(master.source.name()));
    Value::Dict(dict)
}

fn axis_value(axis: &Axis) -> Value {
    let mut dict = Dict::new();
    if axis.is_hidden() {
        dict.put("hidden", Value::number(1.0));
    }
    dict.put("name", Value::string(axis.name()));
    dict.put("tag", Value::string(axis.tag()));
    Value::Dict(dict)
}

fn axes_values(axes: &[Axis], value: impl Fn(&Axis) -> f64) -> Value {
    Value::Raw(format!(
        "({})",
        axes.iter()
            .map(|axis| format_number(value(axis)))
            .collect::<Vec<_>>()
            .join(",")
    ))
}

fn parameter(name: &str, value: Value) -> Value {
    let mut dict = Dict::new();
    dict.put("name", Value::string(name));
    dict.put("value", value);
    Value::Dict(dict)
}

/// Writes each axis's user-to-design mapping, so no reader has to guess one.
///
/// An axis without a mapping of its own maps every value to itself.
fn axis_mappings_value(axes: &[Axis], mappings: &[AxisMapping]) -> Option<Value> {
    let mut by_tag = Dict::new();
    for axis in axes {
        let id = axis.id();
        let mut points = mappings
            .iter()
            .filter(|mapping| mapping.inputs() == [id] && mapping.outputs() == [id])
            .flat_map(AxisMapping::points)
            .filter_map(|point| Some((point.input.get(id)?, point.output.get(id)?)))
            .collect::<Vec<_>>();
        if points.is_empty() {
            points = [axis.minimum(), axis.default(), axis.maximum()]
                .into_iter()
                .map(|value| (value, value))
                .collect();
        }
        points.sort_by(|left, right| left.0.total_cmp(&right.0));
        points.dedup_by(|left, right| left.0 == right.0);
        let mut dict = Dict::new();
        for (user, design) in points {
            dict.put(&format_number(user), Value::number(design));
        }
        by_tag.put(axis.tag(), Value::Dict(dict));
    }
    (!by_tag.is_empty()).then_some(Value::Dict(by_tag))
}

fn glyph_value(
    glyph: &Glyph,
    masters: &Masters<'_>,
    kerning: &Kerning,
    skipped: bool,
    colours: &color::ColorWriter<'_, impl FontView>,
) -> Value {
    let colour = colours.plan(glyph);
    let mut dict = Dict::new();
    if skipped {
        dict.put("export", Value::number(0.0));
    }
    dict.put("glyphname", Value::string(glyph.name()));
    if let Some(group) = kerning_group(kerning, KerningPosition::Second, glyph.id()) {
        dict.put("kernLeft", Value::string(&group));
    }
    if let Some(group) = kerning_group(kerning, KerningPosition::First, glyph.id()) {
        dict.put("kernRight", Value::string(&group));
    }

    let layers: Vec<Value> = masters
        .all
        .iter()
        .filter_map(|master| {
            let layer = glyph.layer_for_source(master.source.id())?;
            let mut value = Dict::new();
            let anchors: Vec<Value> = layer
                .anchors_iter()
                .map(|anchor| {
                    let mut anchor_value = Dict::new();
                    anchor_value.put("name", Value::string(anchor.name().unwrap_or("")));
                    anchor_value.put(
                        "pos",
                        Value::Raw(format!(
                            "({},{})",
                            format_number(anchor.x()),
                            format_number(anchor.y())
                        )),
                    );
                    Value::Dict(anchor_value)
                })
                .collect();
            if !anchors.is_empty() {
                value.put("anchors", Value::Array(anchors));
            }
            let literal = colour
                .as_ref()
                .and_then(|colour| colours.literal_shapes(colour, master));
            if literal.is_some() {
                let mut attributes = Dict::new();
                attributes.put("color", Value::number(1.0));
                value.put("attr", Value::Dict(attributes));
            }
            value.put("layerId", Value::string(&master.id));
            let shapes: Vec<Value> = match literal {
                Some(shapes) => shapes,
                None => layer
                    .contours_iter()
                    .filter(|contour| !contour.is_empty())
                    .map(path_value)
                    .chain(layer.components_iter().map(component_value))
                    .collect(),
            };
            if !shapes.is_empty() {
                value.put("shapes", Value::Array(shapes));
            }
            value.put("width", Value::number(layer.width()));

            let mut written = vec![Value::Dict(value)];
            if let Some(colour) = &colour {
                written.extend(colour.extra_layers(master, layer.width()));
            }
            Some(written)
        })
        .flatten()
        .collect();
    dict.put("layers", Value::Array(layers));

    match glyph.unicodes() {
        [] => {}
        [single] => dict.put("unicode", Value::number(f64::from(*single))),
        many => dict.put(
            "unicode",
            Value::Raw(format!(
                "({})",
                many.iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )),
        ),
    }
    Value::Dict(dict)
}

fn kerning_group(kerning: &Kerning, position: KerningPosition, glyph: GlyphId) -> Option<String> {
    let id = kerning.group_of(position, glyph)?;
    Some(kerning.group(*id)?.name.clone())
}

fn path_value(contour: &Contour) -> Value {
    Value::Dict(path_dict(contour.points(), contour.is_closed()))
}

/// Writes points as a Glyphs path.
fn path_dict(points: &[Point], closed: bool) -> Dict {
    let nodes = points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            format!(
                "({},{},{})",
                format_number(point.x()),
                format_number(point.y()),
                node_type(point, index, points, closed)
            )
        })
        .collect::<Vec<_>>();
    let mut dict = Dict::new();
    if closed {
        dict.put("closed", Value::number(1.0));
    }
    dict.put("nodes", Value::Raw(format!("(\n{}\n)", nodes.join(",\n"))));
    dict
}

/// Names the segment that arrives at `point`, the way a Glyphs node does.
fn node_type(point: &Point, index: usize, points: &[Point], closed: bool) -> &'static str {
    let previous_is_off_curve = if index > 0 {
        points[index - 1].point_type() == PointType::OffCurve
    } else {
        closed
            && points
                .last()
                .is_some_and(|last| last.point_type() == PointType::OffCurve)
    };
    match (point.point_type(), point.is_smooth()) {
        (PointType::OffCurve, _) => "o",
        (PointType::QCurve, false) => "q",
        (PointType::QCurve, true) => "qs",
        (PointType::OnCurve, smooth) => match (previous_is_off_curve, smooth) {
            (true, false) => "c",
            (true, true) => "cs",
            (false, false) => "l",
            (false, true) => "ls",
        },
    }
}

fn component_value(component: &Component) -> Value {
    let matrix = component.matrix();
    let mut dict = Dict::new();
    dict.put("ref", Value::string(component.base_glyph_name()));

    let scale_x = matrix.xx.hypot(matrix.xy);
    let angle = matrix.xy.atan2(matrix.xx);
    let (sin, cos) = angle.sin_cos();
    let scale_y = matrix.yy * cos - matrix.yx * sin;
    let plain = scale_x > 0.0
        && (matrix.yx + scale_y * sin).abs() < MATRIX_TOLERANCE
        && (matrix.yy - scale_y * cos).abs() < MATRIX_TOLERANCE;
    if plain {
        if angle.abs() > MATRIX_TOLERANCE {
            dict.put("angle", Value::number(angle.to_degrees()));
        }
        dict.put(
            "pos",
            Value::Raw(format!(
                "({},{})",
                format_number(matrix.dx),
                format_number(matrix.dy)
            )),
        );
        if (scale_x - 1.0).abs() > MATRIX_TOLERANCE || (scale_y - 1.0).abs() > MATRIX_TOLERANCE {
            dict.put(
                "scale",
                Value::Raw(format!(
                    "({},{})",
                    format_number(scale_x),
                    format_number(scale_y)
                )),
            );
        }
    } else {
        // A shear has no angle and scale; the full matrix is the one form that holds it
        dict.put(
            "transform",
            Value::string(&format!(
                "{{{}, {}, {}, {}, {}, {}}}",
                format_number(matrix.xx),
                format_number(matrix.xy),
                format_number(matrix.yx),
                format_number(matrix.yy),
                format_number(matrix.dx),
                format_number(matrix.dy)
            )),
        );
    }
    Value::Dict(dict)
}

/// Writes every master's pairs, naming groups the way Glyphs does.
fn kerning_value(
    kerning: &Kerning,
    masters: &Masters<'_>,
    glyph_names: &HashMap<GlyphId, String>,
) -> Option<Value> {
    let side = |side: &KerningSide, prefix: &str| match side {
        KerningSide::Glyph(id) => glyph_names.get(id).cloned(),
        KerningSide::Group(id) => kerning
            .group(*id)
            .map(|group| format!("{prefix}{}", group.name)),
    };

    let mut by_master = Dict::new();
    for master in &masters.all {
        let Some(source) = kerning.source(master.source.id()) else {
            continue;
        };
        let mut firsts: Vec<(String, Vec<(String, f64)>)> = Vec::new();
        for (pair, value) in source.pairs() {
            let (Some(first), Some(second)) = (
                side(&pair.first, FIRST_GROUP_PREFIX),
                side(&pair.second, SECOND_GROUP_PREFIX),
            ) else {
                continue;
            };
            match firsts.iter_mut().find(|(name, _)| *name == first) {
                Some((_, seconds)) => seconds.push((second, value)),
                None => firsts.push((first, vec![(second, value)])),
            }
        }
        if firsts.is_empty() {
            continue;
        }
        firsts.sort_by(|left, right| left.0.cmp(&right.0));
        let mut first_dict = Dict::new();
        for (first, mut seconds) in firsts {
            seconds.sort_by(|left, right| left.0.cmp(&right.0));
            let mut second_dict = Dict::new();
            for (second, value) in seconds {
                second_dict.put(&second, Value::number(value));
            }
            first_dict.put(&first, Value::Dict(second_dict));
        }
        by_master.put(&master.id, Value::Dict(first_dict));
    }
    (!by_master.is_empty()).then_some(Value::Dict(by_master))
}

fn put_text(dict: &mut Dict, key: &str, text: Option<&str>) {
    if let Some(text) = text.filter(|text| !text.is_empty()) {
        dict.put(key, Value::string(text));
    }
}

/// Formats a number with no more digits than the file needs.
fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{value:.0}");
    }
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Quotes a string unless the plain-text property list format allows it bare.
fn format_string(text: &str) -> String {
    let bare = !text.is_empty()
        && text.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '$')
        })
        && text.parse::<f64>().is_err();
    if bare {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[derive(Clone)]
enum Value {
    String(String),
    Number(f64),
    /// Text that is already in property list syntax.
    Raw(String),
    Array(Vec<Self>),
    Dict(Dict),
}

impl Value {
    fn string(text: &str) -> Self {
        Self::String(text.to_string())
    }

    fn number(value: f64) -> Self {
        Self::Number(value)
    }

    fn render(&self, out: &mut String) {
        match self {
            Self::String(text) => out.push_str(&format_string(text)),
            Self::Number(value) => out.push_str(&format_number(*value)),
            Self::Raw(text) => out.push_str(text),
            Self::Array(items) if items.is_empty() => out.push_str("()"),
            Self::Array(items) => {
                out.push_str("(\n");
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push_str(",\n");
                    }
                    item.render(out);
                }
                out.push_str("\n)");
            }
            Self::Dict(dict) => dict.render(out),
        }
    }
}

#[derive(Clone, Default)]
struct Dict {
    entries: Vec<(String, Value)>,
}

impl Dict {
    fn new() -> Self {
        Self::default()
    }

    fn put(&mut self, key: &str, value: Value) {
        self.entries.push((key.to_string(), value));
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn render(&self, out: &mut String) {
        out.push_str("{\n");
        for (key, value) in &self.entries {
            let _ = write!(out, "{} = ", format_string(key));
            value.render(out);
            out.push_str(";\n");
        }
        out.push('}');
    }
}

impl crate::traits::FontWriter for GlyphsWriter {
    fn save(&self, font: &shift_font::Font, path: &str) -> FormatBackendResult<()> {
        let text = self.render(font)?;
        crate::atomic::write_file_atomic(std::path::Path::new(path), text.as_bytes())
            .map_err(|error| FormatBackendError::Glyphs(format!("cannot write {path}: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FontReader;
    use crate::formats::glyphs::GlyphsReader;
    use shift_font::Font;
    use shift_font::test_support::{default_kerning_between, sample_variable_font};

    fn read_back(text: &str) -> Font {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Written.glyphs");
        std::fs::write(&path, text).unwrap();
        GlyphsReader::new().load(path.to_str().unwrap()).unwrap()
    }

    fn points(font: &Font, glyph: &str) -> Vec<(f64, f64, bool, bool)> {
        let source = font.default_source_id().unwrap();
        font.glyph_by_name(glyph)
            .unwrap()
            .layer_for_source(source)
            .unwrap()
            .contours_iter()
            .flat_map(|contour| contour.points().iter())
            .map(|point| (point.x(), point.y(), point.is_on_curve(), point.is_smooth()))
            .collect()
    }

    #[test]
    fn numbers_and_strings_follow_the_property_list_rules() {
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(12.0), "12");
        assert_eq!(format_number(12.5), "12.5");
        assert_eq!(format_number(1.0 / 3.0), "0.333333");
        assert_eq!(format_string("A_B.alt"), "A_B.alt");
        assert_eq!(format_string("cap height"), "\"cap height\"");
        assert_eq!(format_string("123"), "\"123\"");
        assert_eq!(format_string("say \"hi\"\n"), "\"say \\\"hi\\\"\\n\"");
        assert_eq!(format_string(""), "\"\"");
    }

    #[test]
    fn a_colour_file_imports_without_new_glyphs_or_changed_outlines() {
        use shift_font::{
            ColorData, ColorGlyph, ColorPalette, Contour, Glyph, GlyphLayer, LayerId, Paint,
            PaintColor, PointType, Rgba,
        };

        let mut font = sample_variable_font();
        let base = font.glyphs().next().unwrap().id();
        let base_name = font.glyphs().next().unwrap().name().to_string();
        let mut layer_glyph = Glyph::new("base.layer");
        for source in font.sources().to_vec() {
            let mut layer = GlyphLayer::with_width(LayerId::new(), source.id(), 600.0);
            let mut contour = Contour::new();
            contour.add_point(100.0, 0.0, PointType::OnCurve, false);
            contour.add_point(300.0, 700.0, PointType::OnCurve, false);
            contour.add_point(500.0, 0.0, PointType::OnCurve, false);
            contour.close();
            layer.add_contour(contour);
            layer_glyph.set_layer(layer);
        }
        let layer = layer_glyph.id();
        font.insert_glyph(layer_glyph).unwrap();
        font.set_color(ColorData {
            palettes: vec![ColorPalette {
                name: None,
                colors: vec![Rgba::new(1, 2, 3, 255)],
            }],
            glyphs: vec![ColorGlyph {
                glyph: base,
                paint: Paint::Glyph {
                    glyph: layer,
                    fill: Box::new(Paint::Solid {
                        color: PaintColor::Palette { index: 0 },
                        alpha: 1.0,
                    }),
                },
            }],
        })
        .unwrap();

        let text = GlyphsWriter::new().render(&font).unwrap();
        let back = read_back(&text);

        // Palette layers are not Shift glyphs, so the colour layers add nothing here
        assert_eq!(back.glyph_count(), font.glyph_count());
        assert_eq!(points(&back, &base_name), points(&font, &base_name));
    }

    #[test]
    fn the_variable_sample_survives_a_round_trip() {
        let font = sample_variable_font();
        let text = GlyphsWriter::new().render(&font).unwrap();
        let back = read_back(&text);

        assert_eq!(back.metadata().family_name, font.metadata().family_name);
        assert_eq!(back.metrics().units_per_em, font.metrics().units_per_em);
        assert_eq!(back.axes().len(), font.axes().len());
        for (before, after) in font.axes().iter().zip(back.axes()) {
            assert_eq!(before.tag(), after.tag());
            assert_eq!(before.name(), after.name());
        }
        assert_eq!(
            back.sources()
                .iter()
                .filter(|source| source.is_master())
                .count(),
            font.sources()
                .iter()
                .filter(|source| source.is_master())
                .count()
        );
        assert_eq!(back.glyph_count(), font.glyph_count());
        for glyph in font.glyphs() {
            let restored = back
                .glyph_by_name(glyph.name())
                .unwrap_or_else(|| panic!("glyph {} is missing", glyph.name()));
            assert_eq!(restored.unicodes(), glyph.unicodes(), "{}", glyph.name());
            assert_eq!(
                points(&back, glyph.name()),
                points(&font, glyph.name()),
                "{}",
                glyph.name()
            );
        }
    }

    #[test]
    fn homenaje_keeps_outlines_kerning_components_and_features() {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts/Homenaje.glyphs");
        let original = GlyphsReader::new().load(source.to_str().unwrap()).unwrap();
        let back = read_back(&GlyphsWriter::new().render(&original).unwrap());

        assert_eq!(back.glyph_count(), original.glyph_count());
        for name in ["A", "B", "ampersand", "Aacute", "a"] {
            assert_eq!(points(&back, name), points(&original, name), "{name}");
        }
        assert_eq!(default_kerning_between(&back, "A", "V"), Some(-55.0));
        assert!(
            back.features()
                .fea_source()
                .unwrap()
                .contains("feature frac")
        );

        let source_id = back.default_source_id().unwrap();
        let aacute = back
            .glyph_by_name("Aacute")
            .unwrap()
            .layer_for_source(source_id)
            .unwrap();
        let before = original.glyph_by_name("Aacute").unwrap();
        let before = before
            .layer_for_source(original.default_source_id().unwrap())
            .unwrap();
        assert_eq!(aacute.components().len(), before.components().len());
        for (after, before) in aacute.components_iter().zip(before.components_iter()) {
            assert_eq!(after.base_glyph_name(), before.base_glyph_name());
            let (after, before) = (after.matrix(), before.matrix());
            for (left, right) in [
                (after.xx, before.xx),
                (after.xy, before.xy),
                (after.yx, before.yx),
                (after.yy, before.yy),
                (after.dx, before.dx),
                (after.dy, before.dy),
            ] {
                assert!((left - right).abs() < 1e-4, "{after:?} against {before:?}");
            }
        }
    }

    #[test]
    fn masters_axis_mappings_and_instances_survive_a_round_trip() {
        for file in [
            "MappedWeight.glyphs",
            "MultiFamily.glyphs",
            "MutatorSansVariable.glyphs",
        ] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/fonts")
                .join(file);
            if !path.is_file() {
                continue;
            }
            let original = GlyphsReader::new().load(path.to_str().unwrap()).unwrap();
            let back = read_back(&GlyphsWriter::new().render(&original).unwrap());

            assert_eq!(back.axes().len(), original.axes().len(), "{file}");
            for (before, after) in original.axes().iter().zip(back.axes()) {
                assert_eq!(
                    (before.minimum(), before.default(), before.maximum()),
                    (after.minimum(), after.default(), after.maximum()),
                    "{file}: {}",
                    before.tag()
                );
            }
            let masters = |font: &Font| {
                font.sources()
                    .iter()
                    .filter(|source| source.is_master())
                    .map(|source| source.name().to_string())
                    .collect::<Vec<_>>()
            };
            assert_eq!(masters(&back), masters(&original), "{file}");
            let instances = |font: &Font| {
                font.named_instances()
                    .iter()
                    .map(|instance| instance.name().to_string())
                    .collect::<Vec<_>>()
            };
            assert_eq!(instances(&back), instances(&original), "{file}");
        }
    }

    #[test]
    fn the_exported_file_compiles_to_the_same_glyphs() {
        let font = sample_variable_font();
        let back = read_back(&GlyphsWriter::new().render(&font).unwrap());

        let before = crate::FontExporter::new().compile_ttf(&font).unwrap();
        let after = crate::FontExporter::new().compile_ttf(&back).unwrap();
        let count = |bytes: &[u8]| {
            use skrifa::MetadataProvider;
            skrifa::FontRef::new(bytes)
                .unwrap()
                .glyph_names()
                .iter()
                .count()
        };
        assert_eq!(count(&before), count(&after));
    }
}
