use std::{fs, path::Path};

use glyphs_reader::{Font as GlyphsFont, Shape};

use crate::{ImportLoss, ImportLossKind, ImportReport};

/// Tag of the axis that Glyphs uses for layered-colour masters.
const LAYERED_COLOR_TAG: &str = "COLR";

pub(super) fn import_report(source: &GlyphsFont) -> ImportReport {
    let mut report = ImportReport::default();

    let bracket_layers = source
        .glyphs
        .values()
        .map(|glyph| glyph.bracket_layers.len())
        .sum::<usize>();
    if bracket_layers > 0 {
        report.losses.push(ImportLoss {
            kind: ImportLossKind::Omitted,
            message: format!(
                "Shift does not represent Glyphs bracket layers; {bracket_layers} conditional layers were omitted."
            ),
        });
    }

    let master_locations = source
        .masters
        .iter()
        .map(|master| &master.axes_values)
        .collect::<Vec<_>>();
    let master_located_intermediate_layers = source
        .glyphs
        .values()
        .flat_map(|glyph| &glyph.layers)
        .filter(|layer| {
            layer.is_intermediate() && master_locations.contains(&&layer.attributes.coordinates)
        })
        .count();
    if master_located_intermediate_layers > 0 {
        report.losses.push(ImportLoss {
            kind: ImportLossKind::Omitted,
            message: format!(
                "{master_located_intermediate_layers} Glyphs intermediate layers sit exactly on a master location and were omitted."
            ),
        });
    }

    let smart_component_glyphs = source
        .glyphs
        .values()
        .filter(|glyph| {
            !glyph.smart_component_axes.is_empty()
                || glyph
                    .layers
                    .iter()
                    .chain(&glyph.bracket_layers)
                    .any(|layer| {
                        !layer.smart_component_positions.is_empty()
                            || layer.shapes.iter().any(|shape| {
                                matches!(shape, Shape::Component(component) if !component.smart_component_values.is_empty())
                            })
                    })
        })
        .count();
    if smart_component_glyphs > 0 {
        report.losses.push(ImportLoss {
            kind: ImportLossKind::Approximated,
            message: format!(
                "Shift does not represent Glyphs smart-component axes and locations; {smart_component_glyphs} smart glyphs keep only their master layers, and their pole layers were omitted."
            ),
        });
    }

    let rtl_kerning_pairs = source
        .kerning_rtl
        .iter()
        .map(|(_, pairs)| pairs.len())
        .sum::<usize>();
    if rtl_kerning_pairs > 0 {
        report.losses.push(ImportLoss {
            kind: ImportLossKind::Omitted,
            message: format!(
                "Shift does not represent Glyphs RTL kerning separately; {rtl_kerning_pairs} RTL kerning pairs were omitted."
            ),
        });
    }

    if let Some(axis) = source
        .axes
        .iter()
        .find(|axis| axis.tag == LAYERED_COLOR_TAG)
    {
        report.losses.push(ImportLoss {
            kind: ImportLossKind::Approximated,
            message: format!(
                "Shift does not represent Glyphs layered-colour masters; the '{}' axis ({LAYERED_COLOR_TAG}) was imported as an ordinary axis, and no colour glyphs were built from it.",
                axis.name
            ),
        });
    }

    report
}

/// Reports the Glyphs bitmap and SVG colour layers that the reader discards.
///
/// `glyphs-reader` skips the `svg` and `sbixSize` layer attributes, so those
/// layers reach the converter as plain drafts and vanish from the parsed
/// source. They are counted in the raw source text instead. A source that
/// cannot be read contributes nothing, because the import itself already
/// succeeded.
pub(super) fn discarded_colour_layer_losses(path: &Path) -> Vec<ImportLoss> {
    let counts = count_discarded_colour_layers(&source_texts(path));
    let mut losses = Vec::new();

    if counts.svg > 0 {
        losses.push(ImportLoss {
            kind: ImportLossKind::Omitted,
            message: format!(
                "Shift does not import Glyphs SVG colour layers; {} were omitted.",
                counts.svg
            ),
        });
    }

    if counts.sbix > 0 {
        losses.push(ImportLoss {
            kind: ImportLossKind::Omitted,
            message: format!(
                "Shift does not import Glyphs sbix bitmap layers; {} were omitted.",
                counts.sbix
            ),
        });
    }

    losses
}

#[derive(Debug, Default, PartialEq, Eq)]
struct DiscardedColourLayers {
    svg: usize,
    sbix: usize,
}

fn count_discarded_colour_layers(texts: &[String]) -> DiscardedColourLayers {
    let mut counts = DiscardedColourLayers::default();

    for line in texts.iter().flat_map(|text| text.lines()) {
        let line = line.trim();
        if line == "svg = 1;" {
            counts.svg += 1;
        } else if line.starts_with("sbixSize = ") {
            counts.sbix += 1;
        }
    }

    counts
}

/// Returns the text of a `.glyphs` file, or of each glyph in a `.glyphspackage`.
fn source_texts(path: &Path) -> Vec<String> {
    if !path.is_dir() {
        return fs::read_to_string(path).into_iter().collect();
    }

    let Ok(entries) = fs::read_dir(path.join("glyphs")) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use glyphs_reader::{AxisRule, Font as GlyphsFont, Glyph, Layer, LayerAttributes};
    use ordered_float::OrderedFloat;

    use super::*;

    #[test]
    fn reports_source_concepts_shift_cannot_represent() {
        let mut source = GlyphsFont::default();
        let mut glyph = Glyph {
            name: "A".into(),
            ..Glyph::default()
        };
        glyph.bracket_layers.push(Layer {
            attributes: LayerAttributes {
                axis_rules: vec![AxisRule {
                    min: Some(400),
                    max: None,
                }],
                ..LayerAttributes::default()
            },
            ..Layer::default()
        });
        glyph.layers.push(Layer {
            associated_master_id: Some("master01".to_string()),
            attributes: LayerAttributes {
                coordinates: vec![OrderedFloat(500.0)],
                ..LayerAttributes::default()
            },
            ..Layer::default()
        });
        glyph.smart_component_axes.insert("Width".into(), 0..=100);
        source.glyphs.insert("A".into(), glyph);

        let report = import_report(&source);

        assert_eq!(report.losses.len(), 2);
        assert_eq!(report.losses[0].kind, ImportLossKind::Omitted);
        assert!(report.losses[0].message.contains("bracket layers"));
        assert_eq!(report.losses[1].kind, ImportLossKind::Approximated);
        assert!(report.losses[1].message.contains("smart-component"));
        assert!(
            !report
                .losses
                .iter()
                .any(|loss| loss.message.contains("intermediate"))
        );
    }

    #[test]
    fn reports_the_layered_colour_axis() {
        let mut source = GlyphsFont::default();
        source.axes.push(glyphs_reader::Axis {
            name: "Color".into(),
            tag: "COLR".into(),
            hidden: None,
        });
        source.axes.push(glyphs_reader::Axis {
            name: "Weight".into(),
            tag: "wght".into(),
            hidden: None,
        });

        let report = import_report(&source);

        assert_eq!(report.losses.len(), 1);
        assert_eq!(report.losses[0].kind, ImportLossKind::Approximated);
        assert!(report.losses[0].message.contains("'Color' axis"));
    }

    #[test]
    fn counts_the_svg_and_sbix_layers_the_reader_discards() {
        let text = "attr = {\nsvg = 1;\n};\nattr = {\nsbixSize = 32;\n};\nattr = {\nsbixSize = 64;\n};\nname = \"svg = 1;x\";";

        let counts = count_discarded_colour_layers(&[text.to_string()]);

        assert_eq!(counts, DiscardedColourLayers { svg: 1, sbix: 2 });
    }

    #[test]
    fn reports_discarded_colour_layers_of_a_source_file() {
        let directory =
            std::env::temp_dir().join(format!("shift-colour-loss-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let file = directory.join("Sample.glyphs");
        fs::write(
            &file,
            "attr = {\nsvg = 1;\n};\nattr = {\nsbixSize = 32;\n};\n",
        )
        .unwrap();

        let losses = discarded_colour_layer_losses(&file);
        fs::remove_dir_all(&directory).unwrap();

        assert_eq!(losses.len(), 2);
        assert!(losses[0].message.contains("SVG colour layers; 1"));
        assert!(losses[1].message.contains("sbix bitmap layers; 1"));
    }
}
