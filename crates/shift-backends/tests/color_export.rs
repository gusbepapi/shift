//! Colour glyphs compile into `COLR` and `CPAL` tables in every export format.

use shift_backends::{ExportFormat, FontExporter};
use shift_font::{
    ColorData, ColorGlyph, ColorPalette, ColorPoint, ColorStop, Contour, Font, Glyph, GlyphId,
    GlyphLayer, LayerId, Paint, PaintColor, PointType, Rgba, test_support::sample_variable_font,
};
use write_fonts::read::tables::colr::Paint as ReadPaint;
use write_fonts::read::{FontRef, TableProvider};

/// The variable sample plus layer glyphs `acute` and `B`, drawn on every master.
fn sample_font() -> Font {
    let mut font = sample_variable_font();
    for (name, unicode) in [("acute", None), ("B", Some(0x42))] {
        let mut glyph = match unicode {
            Some(unicode) => Glyph::with_unicode(name, unicode),
            None => Glyph::new(name),
        };
        for source in font.sources().to_vec() {
            let mut layer = GlyphLayer::with_width(LayerId::new(), source.id(), 600.0);
            let mut contour = Contour::new();
            contour.add_point(100.0, 0.0, PointType::OnCurve, false);
            contour.add_point(300.0, 700.0, PointType::OnCurve, false);
            contour.add_point(500.0, 0.0, PointType::OnCurve, false);
            contour.close();
            layer.add_contour(contour);
            glyph.set_layer(layer);
        }
        font.insert_glyph(glyph).unwrap();
    }
    font
}

fn id(font: &Font, name: &str) -> GlyphId {
    font.glyph_by_name(name).unwrap().id()
}

fn solid(index: u16, alpha: f64) -> Paint {
    Paint::Solid {
        color: PaintColor::Palette { index },
        alpha,
    }
}

fn layer(font: &Font, name: &str, fill: Paint) -> Paint {
    Paint::Glyph {
        glyph: id(font, name),
        fill: Box::new(fill),
    }
}

/// Two palettes whose entries 0 and 1 share a colour in palette 0 only.
fn palettes() -> Vec<ColorPalette> {
    vec![
        ColorPalette {
            name: None,
            colors: vec![Rgba::new(10, 20, 30, 255), Rgba::new(10, 20, 30, 255)],
        },
        ColorPalette {
            name: None,
            colors: vec![Rgba::new(200, 0, 0, 255), Rgba::new(0, 0, 200, 128)],
        },
    ]
}

fn coloured(mut font: Font, glyphs: Vec<ColorGlyph>) -> Font {
    font.set_color(ColorData {
        palettes: palettes(),
        glyphs,
    })
    .unwrap();
    font
}

fn gid(bytes: &[u8], name: &str) -> u16 {
    use skrifa::MetadataProvider;
    let font = skrifa::FontRef::new(bytes).unwrap();
    font.glyph_names()
        .iter()
        .find(|(_, glyph)| glyph.as_str() == name)
        .map(|(glyph, _)| glyph.to_u32() as u16)
        .unwrap()
}

#[test]
fn opaque_solid_layers_become_colr_version_0_with_every_palette() {
    let base = sample_font();
    let font = coloured(
        base.clone(),
        vec![ColorGlyph {
            glyph: id(&base, "A"),
            paint: Paint::Layers {
                layers: vec![
                    layer(&base, "acute", solid(0, 1.0)),
                    layer(&base, "B", solid(1, 1.0)),
                ],
            },
        }],
    );

    let bytes = FontExporter::new()
        .compile(&font, ExportFormat::Ttf)
        .unwrap();
    let parsed = FontRef::new(&bytes).unwrap();

    let cpal = parsed.cpal().unwrap();
    assert_eq!(cpal.num_palettes(), 2);
    assert_eq!(cpal.num_palette_entries(), 2);
    let records = cpal.color_records_array().unwrap().unwrap();
    assert_eq!(records.len(), 4);
    assert_eq!((records[2].red(), records[3].alpha()), (200, 128));
    // Entries that share a colour in palette 0 stay separate entries
    assert_eq!((records[0].red(), records[1].red()), (10, 10));

    let colr = parsed.colr().unwrap();
    assert_eq!(colr.num_base_glyph_records(), 1);
    assert!(colr.base_glyph_list().is_none());
    let base_records = colr.base_glyph_records().unwrap().unwrap();
    assert_eq!(
        base_records[0].glyph_id().to_u32(),
        u32::from(gid(&bytes, "A"))
    );
    assert_eq!(base_records[0].num_layers(), 2);
    let layers = colr.layer_records().unwrap().unwrap();
    assert_eq!(
        layers[0].glyph_id().to_u32(),
        u32::from(gid(&bytes, "acute"))
    );
    assert_eq!(
        (layers[0].palette_index(), layers[1].palette_index()),
        (0, 1)
    );
}

#[test]
fn gradients_and_opacity_become_a_colr_version_1_paint_graph_with_a_clip() {
    let base = sample_font();
    let stops = vec![
        ColorStop {
            offset: 0.0,
            color: PaintColor::Palette { index: 1 },
            alpha: 1.0,
        },
        ColorStop {
            offset: 1.0,
            color: PaintColor::Foreground,
            alpha: 0.5,
        },
    ];
    let font = coloured(
        base.clone(),
        vec![ColorGlyph {
            glyph: id(&base, "A"),
            paint: Paint::Layers {
                layers: vec![
                    layer(&base, "B", solid(0, 0.25)),
                    layer(
                        &base,
                        "acute",
                        Paint::LinearGradient {
                            stops,
                            p0: ColorPoint::new(0.0, 0.0),
                            p1: ColorPoint::new(0.0, 700.0),
                            p2: None,
                        },
                    ),
                ],
            },
        }],
    );

    let bytes = FontExporter::new()
        .compile(&font, ExportFormat::Ttf)
        .unwrap();
    let parsed = FontRef::new(&bytes).unwrap();
    let colr = parsed.colr().unwrap();

    assert_eq!(colr.num_base_glyph_records(), 0);
    let list = colr.base_glyph_list().unwrap().unwrap();
    let record = &list.base_glyph_paint_records()[0];
    assert_eq!(record.glyph_id().to_u32(), u32::from(gid(&bytes, "A")));
    let ReadPaint::ColrLayers(layers) = record.paint(list.offset_data()).unwrap() else {
        panic!("expected a layer stack");
    };
    assert_eq!(layers.num_layers(), 2);

    let layer_list = colr.layer_list().unwrap().unwrap();
    let ReadPaint::Glyph(first) = layer_list.paints().get(0).unwrap() else {
        panic!("expected a glyph paint");
    };
    let ReadPaint::Solid(solid_fill) = first.paint().unwrap() else {
        panic!("expected a solid fill");
    };
    assert_eq!(solid_fill.palette_index(), 0);
    assert!((solid_fill.alpha().to_f32() - 0.25).abs() < 0.001);

    let ReadPaint::Glyph(second) = layer_list.paints().get(1).unwrap() else {
        panic!("expected a glyph paint");
    };
    let ReadPaint::LinearGradient(gradient) = second.paint().unwrap() else {
        panic!("expected a linear gradient");
    };
    let line = gradient.color_line().unwrap();
    assert_eq!(line.color_stops()[0].palette_index(), 1);
    assert_eq!(line.color_stops()[1].palette_index(), 0xFFFF);
    assert!((line.color_stops()[1].alpha().to_f32() - 0.5).abs() < 0.001);
    // The perpendicular default is p0 + (dy, -dx)
    assert_eq!((gradient.x2().to_i16(), gradient.y2().to_i16()), (700, 0));

    assert!(colr.clip_list().is_some());
}

#[test]
fn fonts_without_colour_carry_no_colour_tables() {
    let bytes = FontExporter::new()
        .compile(&sample_font(), ExportFormat::Ttf)
        .unwrap();
    let parsed = FontRef::new(&bytes).unwrap();
    assert!(parsed.colr().is_err());
    assert!(parsed.cpal().is_err());
}

#[test]
fn colour_tables_survive_woff_and_woff2_wrapping() {
    let base = sample_font();
    let font = coloured(
        base.clone(),
        vec![ColorGlyph {
            glyph: id(&base, "A"),
            paint: layer(&base, "B", solid(0, 1.0)),
        }],
    );

    let woff = FontExporter::new()
        .compile(&font, ExportFormat::Woff)
        .unwrap();
    let table_count = usize::from(u16::from_be_bytes([woff[12], woff[13]]));
    let tags: Vec<&[u8]> = (0..table_count)
        .map(|index| &woff[44 + index * 20..48 + index * 20])
        .collect();
    assert!(tags.contains(&&b"COLR"[..]) && tags.contains(&&b"CPAL"[..]));

    let woff2 = FontExporter::new()
        .compile(&font, ExportFormat::Woff2)
        .unwrap();
    assert_eq!(&woff2[..4], b"wOF2");
}
