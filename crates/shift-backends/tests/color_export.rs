//! Colour glyphs compile into `COLR` and `CPAL` tables in every export format.

use shift_backends::{ColorTable, ColorTables, ExportFormat, FontExportRequest, FontExporter};
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
        .map(|(glyph, _)| u16::try_from(glyph.to_u32()).unwrap())
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

/// The compiled font with `names` as palette names and one colour glyph.
fn compile_named(names: [Option<&str>; 2]) -> Vec<u8> {
    let base = sample_font();
    let mut data = ColorData {
        palettes: palettes(),
        glyphs: vec![ColorGlyph {
            glyph: id(&base, "A"),
            paint: layer(&base, "B", solid(0, 1.0)),
        }],
    };
    for (palette, name) in data.palettes.iter_mut().zip(names) {
        palette.name = name.map(str::to_string);
    }
    let mut font = base;
    font.set_color(data).unwrap();
    FontExporter::new()
        .compile(&font, ExportFormat::Ttf)
        .unwrap()
}

/// Returns the English `name` table string for a name ID.
fn name_string(font: &FontRef<'_>, name_id: u16) -> Option<String> {
    let name = font.name().unwrap();
    name.name_record()
        .iter()
        .find(|record| record.name_id().to_u16() == name_id && record.language_id() == 0x0409)
        .map(|record| record.string(name.string_data()).unwrap().chars().collect())
}

#[test]
fn palette_names_become_cpal_labels_in_the_name_table() {
    let bytes = compile_named([Some("Light"), Some("Dark")]);
    let parsed = FontRef::new(&bytes).unwrap();
    let cpal = parsed.cpal().unwrap();

    assert_eq!(cpal.version(), 1);
    let labels = cpal.palette_labels_array().unwrap().unwrap();
    assert_eq!(labels.len(), 2);
    let ids: Vec<u16> = labels.iter().map(|label| label.get().to_u16()).collect();
    assert!(ids.iter().all(|id| (256..=32767).contains(id)));
    assert_ne!(ids[0], ids[1]);
    assert_eq!(name_string(&parsed, ids[0]).as_deref(), Some("Light"));
    assert_eq!(name_string(&parsed, ids[1]).as_deref(), Some("Dark"));

    // The colour records are unchanged by the version 1 header
    let records = cpal.color_records_array().unwrap().unwrap();
    assert_eq!((records[0].red(), records[2].red()), (10, 200));
    assert_eq!(records[3].alpha(), 128);
}

#[test]
fn unnamed_palettes_in_a_named_font_have_no_label() {
    let bytes = compile_named([None, Some("Dark")]);
    let parsed = FontRef::new(&bytes).unwrap();
    let labels = parsed
        .cpal()
        .unwrap()
        .palette_labels_array()
        .unwrap()
        .unwrap();

    assert_eq!(labels[0].get().to_u16(), 0xFFFF);
    let dark = labels[1].get().to_u16();
    assert_eq!(name_string(&parsed, dark).as_deref(), Some("Dark"));
}

#[test]
fn palettes_with_the_same_name_share_one_name_record() {
    let bytes = compile_named([Some("Brand"), Some("Brand")]);
    let parsed = FontRef::new(&bytes).unwrap();
    let labels = parsed
        .cpal()
        .unwrap()
        .palette_labels_array()
        .unwrap()
        .unwrap();

    assert_eq!(labels[0].get(), labels[1].get());
}

#[test]
fn fonts_without_palette_names_keep_cpal_version_0() {
    let bytes = compile_named([None, None]);
    let parsed = FontRef::new(&bytes).unwrap();
    let cpal = parsed.cpal().unwrap();

    assert_eq!(cpal.version(), 0);
    assert!(cpal.palette_labels_array().is_none());
}

#[test]
fn palette_names_leave_the_existing_name_records_intact() {
    let plain = compile_named([None, None]);
    let named = compile_named([Some("Light"), Some("Dark")]);
    let before = FontRef::new(&plain).unwrap();
    let after = FontRef::new(&named).unwrap();

    for id in [1u16, 2, 4, 6] {
        assert_eq!(name_string(&before, id), name_string(&after, id), "{id}");
    }
    assert!(name_string(&before, 1).is_some());
}

/// Returns the SVG document that the `SVG ` table holds for a glyph.
fn svg_document(bytes: &[u8], glyph: u16) -> Option<String> {
    let parsed = FontRef::new(bytes).unwrap();
    let svg = parsed.svg().ok()?;
    let data = svg
        .glyph_data(write_fonts::types::GlyphId::new(u32::from(glyph)))
        .unwrap()?;
    Some(String::from_utf8(data.to_vec()).unwrap())
}

/// Counts the elements named `name`, failing when the document is not well-formed XML.
fn count_elements(document: &str, name: &str) -> usize {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_str(document);
    let mut count = 0;
    loop {
        match reader.read_event().expect("well-formed SVG") {
            Event::Start(element) | Event::Empty(element)
                if element.name().as_ref() == name =>
            {
                count += 1;
            }
            Event::Eof => return count,
            _ => {}
        }
    }
}

#[test]
fn colour_glyphs_are_written_as_svg_documents_with_palette_variables() {
    let base = sample_font();
    let font = coloured(
        base.clone(),
        vec![ColorGlyph {
            glyph: id(&base, "A"),
            paint: Paint::Layers {
                layers: vec![
                    layer(&base, "acute", solid(0, 1.0)),
                    layer(&base, "B", solid(1, 0.5)),
                ],
            },
        }],
    );
    let bytes = FontExporter::new()
        .compile(&font, ExportFormat::Ttf)
        .unwrap();
    let a = gid(&bytes, "A");

    let document = svg_document(&bytes, a).expect("a document for A");

    assert_eq!(count_elements(&document, "path"), 2);
    assert!(document.contains(&format!(r#"id="glyph{a}""#)));
    // The fallbacks are palette 0, and layer opacity is kept separately
    assert!(document.contains("fill:var(--color0,#0a141e);fill-opacity:1"));
    assert!(document.contains("fill:var(--color1,#0a141e);fill-opacity:0.5"));
    // Outlines are flipped to SVG's downward y axis by one group
    assert!(document.contains(r#"transform="scale(1,-1)""#));
    assert!(document.contains(r#"d="M"#));
    // Only the colour glyph has a document
    assert!(svg_document(&bytes, gid(&bytes, "B")).is_none());
}

#[test]
fn gradients_and_foreground_colour_are_written_into_svg_defs() {
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
                    layer(
                        &base,
                        "B",
                        Paint::LinearGradient {
                            stops: stops.clone(),
                            p0: ColorPoint::new(0.0, 0.0),
                            p1: ColorPoint::new(0.0, 700.0),
                            p2: None,
                        },
                    ),
                    layer(
                        &base,
                        "acute",
                        Paint::RadialGradient {
                            stops,
                            c0: ColorPoint::new(300.0, 300.0),
                            r0: None,
                            c1: ColorPoint::new(300.0, 300.0),
                            r1: Some(250.0),
                        },
                    ),
                ],
            },
        }],
    );
    let bytes = FontExporter::new()
        .compile(&font, ExportFormat::Ttf)
        .unwrap();

    let document = svg_document(&bytes, gid(&bytes, "A")).unwrap();

    assert_eq!(count_elements(&document, "linearGradient"), 1);
    assert_eq!(count_elements(&document, "radialGradient"), 1);
    assert_eq!(count_elements(&document, "stop"), 4);
    assert!(document.contains(r#"x1="0" y1="0" x2="0" y2="700""#));
    assert!(document.contains(r#"cx="300" cy="300" r="250" fx="300" fy="300" fr="0""#));
    assert!(document.contains("stop-color:var(--color1,#0a141e)"));
    assert!(document.contains("stop-color:context-fill;stop-opacity:0.5"));
    // Each path refers to a gradient that the defs define
    for id in document
        .split(r#"fill="url(#"#)
        .skip(1)
        .map(|rest| rest.split(')').next().unwrap())
    {
        assert!(document.contains(&format!(r#"id="{id}""#)), "{id}");
    }
}

#[test]
fn fonts_without_colour_glyphs_carry_no_svg_table() {
    let bytes = FontExporter::new()
        .compile(&sample_font(), ExportFormat::Ttf)
        .unwrap();
    assert!(FontRef::new(&bytes).unwrap().svg().is_err());

    // Palettes alone do not need a colour glyph document
    let base = sample_font();
    let palettes_only = coloured(base, Vec::new());
    let bytes = FontExporter::new()
        .compile(&palettes_only, ExportFormat::Ttf)
        .unwrap();
    assert!(FontRef::new(&bytes).unwrap().svg().is_err());
}

#[test]
fn svg_documents_survive_woff_wrapping() {
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
    let mut tags = (0..table_count).map(|index| &woff[44 + index * 20..48 + index * 20]);
    assert!(tags.any(|tag| tag == b"SVG "));
}

/// A decoded `sbix` bitmap with its offset from the glyph origin.
struct Bitmap {
    origin_x: i16,
    origin_y: i16,
    width: usize,
    height: usize,
    rgba: Vec<u8>,
    ppem: u16,
}

/// Floors a pixel coordinate; a negative one saturates to 0 and fails the bounds assertion below.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "float-to-integer `as` saturates, which is the wanted clamp"
)]
fn pixel_index(value: f64) -> usize {
    value.floor() as usize
}

impl Bitmap {
    /// Returns the pixel under a point in font units.
    fn pixel_at(&self, bytes: &[u8], x: f64, y: f64) -> [u8; 4] {
        let upem = f64::from(FontRef::new(bytes).unwrap().head().unwrap().units_per_em());
        let scale = f64::from(self.ppem) / upem;
        let column = pixel_index(x * scale - f64::from(self.origin_x));
        let row = pixel_index(f64::from(self.origin_y) + self.height as f64 - y * scale);
        assert!(column < self.width && row < self.height, "{column},{row}");
        let at = (row * self.width + column) * 4;
        self.rgba[at..at + 4].try_into().unwrap()
    }
}

/// Decodes the bitmap of a glyph in the strike of a pixel size.
fn sbix_bitmap(bytes: &[u8], ppem: u16, glyph: u16) -> Option<Bitmap> {
    let parsed = FontRef::new(bytes).unwrap();
    let sbix = parsed.sbix().ok()?;
    let strikes = sbix.strikes();
    let strike = (0..strikes.len())
        .map(|index| strikes.get(index).unwrap())
        .find(|strike| strike.ppem() == ppem)?;
    let data = strike
        .glyph_data(write_fonts::types::GlyphId::new(u32::from(glyph)))
        .unwrap()?;
    assert_eq!(data.graphic_type().to_string(), "png ");
    let mut reader = png::Decoder::new(data.data()).read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    Some(Bitmap {
        origin_x: data.origin_offset_x(),
        origin_y: data.origin_offset_y(),
        width: info.width as usize,
        height: info.height as usize,
        rgba,
        ppem,
    })
}

/// Compiles `base` with one colour glyph for `A` whose layers are `layers`.
fn compile_ttf_with(base: &Font, layers: Vec<Paint>) -> Vec<u8> {
    let font = coloured(
        base.clone(),
        vec![ColorGlyph {
            glyph: id(base, "A"),
            paint: Paint::Layers { layers },
        }],
    );
    FontExporter::new()
        .compile(&font, ExportFormat::Ttf)
        .unwrap()
}

#[test]
fn colour_glyphs_get_a_png_in_every_sbix_strike() {
    let base = sample_font();
    let bytes = compile_ttf_with(&base, vec![layer(&base, "B", solid(0, 1.0))]);
    let a = gid(&bytes, "A");

    let parsed = FontRef::new(&bytes).unwrap();
    let sbix = parsed.sbix().unwrap();
    assert_eq!(sbix.strikes().len(), 3);
    for ppem in [32, 64, 128] {
        let strike_ppi = (0..3)
            .map(|index| sbix.strikes().get(index).unwrap())
            .find(|strike| strike.ppem() == ppem)
            .unwrap()
            .ppi();
        assert_eq!(strike_ppi, 72);
        let bitmap = sbix_bitmap(&bytes, ppem, a).expect("a bitmap for A");
        assert!(bitmap.width > 0 && bitmap.height > 0);
        // Glyphs that are not colour base glyphs have no bitmap
        assert!(sbix_bitmap(&bytes, ppem, gid(&bytes, "B")).is_none());
    }
}

#[test]
fn sbix_bitmaps_hold_the_palette_colour_inside_the_outline_only() {
    let base = sample_font();
    let bytes = compile_ttf_with(&base, vec![layer(&base, "B", solid(0, 1.0))]);
    let bitmap = sbix_bitmap(&bytes, 128, gid(&bytes, "A")).unwrap();

    // The middle of the triangle (100,0) (300,700) (500,0) is palette 0, entry 0
    assert_eq!(bitmap.pixel_at(&bytes, 300.0, 200.0), [10, 20, 30, 255]);
    // The padded corner and the area beside the apex are transparent
    assert_eq!(bitmap.rgba[3], 0);
    assert_eq!(bitmap.pixel_at(&bytes, 120.0, 600.0)[3], 0);
}

#[test]
fn sbix_layer_opacity_scales_the_alpha_of_the_bitmap() {
    let base = sample_font();
    let bytes = compile_ttf_with(&base, vec![layer(&base, "B", solid(0, 0.5))]);
    let bitmap = sbix_bitmap(&bytes, 128, gid(&bytes, "A")).unwrap();

    let [_, _, _, alpha] = bitmap.pixel_at(&bytes, 300.0, 200.0);
    assert!((i32::from(alpha) - 128).abs() <= 2, "{alpha}");
}

#[test]
fn sbix_gradients_vary_along_their_axis() {
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
            alpha: 0.0,
        },
    ];
    let bytes = compile_ttf_with(
        &base,
        vec![layer(
            &base,
            "B",
            Paint::LinearGradient {
                stops,
                p0: ColorPoint::new(0.0, 0.0),
                p1: ColorPoint::new(0.0, 700.0),
                p2: None,
            },
        )],
    );
    let bitmap = sbix_bitmap(&bytes, 128, gid(&bytes, "A")).unwrap();

    let low = bitmap.pixel_at(&bytes, 300.0, 100.0)[3];
    let high = bitmap.pixel_at(&bytes, 300.0, 500.0)[3];
    assert!(low > 200 && high < 110 && low > high, "{low} {high}");
}

#[test]
fn fonts_without_colour_glyphs_carry_no_sbix_table() {
    let bytes = FontExporter::new()
        .compile(&sample_font(), ExportFormat::Ttf)
        .unwrap();
    assert!(FontRef::new(&bytes).unwrap().sbix().is_err());
}

/// Compiles a font with one colour glyph into the chosen colour tables.
fn compile_into(tables: &[ColorTable]) -> Vec<u8> {
    let base = sample_font();
    let font = coloured(
        base.clone(),
        vec![ColorGlyph {
            glyph: id(&base, "A"),
            paint: layer(&base, "B", solid(0, 0.5)),
        }],
    );
    FontExporter::new()
        .compile_with(
            &font,
            ExportFormat::Ttf,
            &tables.iter().copied().collect::<ColorTables>(),
        )
        .unwrap()
}

/// Lists which of `COLR`, `CPAL`, `SVG ` and `sbix` a font carries.
fn colour_tags(bytes: &[u8]) -> Vec<&'static str> {
    let font = FontRef::new(bytes).unwrap();
    ["COLR", "CPAL", "SVG ", "sbix"]
        .into_iter()
        .filter(|tag| {
            font.table_data(write_fonts::types::Tag::new_checked(tag.as_bytes()).unwrap())
                .is_some()
        })
        .collect()
}

#[test]
fn every_colour_table_is_written_by_default() {
    let base = sample_font();
    let font = coloured(
        base.clone(),
        vec![ColorGlyph {
            glyph: id(&base, "A"),
            paint: layer(&base, "B", solid(0, 1.0)),
        }],
    );

    let bytes = FontExporter::new()
        .compile(&font, ExportFormat::Ttf)
        .unwrap();

    assert_eq!(colour_tags(&bytes), ["COLR", "CPAL", "SVG ", "sbix"]);
}

#[test]
fn colr_alone_writes_the_palettes_and_paint_graphs_only() {
    let bytes = compile_into(&[ColorTable::Colr]);

    assert_eq!(colour_tags(&bytes), ["COLR", "CPAL"]);
}

#[test]
fn svg_alone_writes_the_palettes_the_documents_refer_to() {
    let bytes = compile_into(&[ColorTable::Svg]);

    assert_eq!(colour_tags(&bytes), ["CPAL", "SVG "]);
    assert!(svg_document(&bytes, gid(&bytes, "A")).is_some());
}

#[test]
fn sbix_alone_writes_bitmaps_without_palettes() {
    let bytes = compile_into(&[ColorTable::Sbix]);

    assert_eq!(colour_tags(&bytes), ["sbix"]);
    assert!(sbix_bitmap(&bytes, 64, gid(&bytes, "A")).is_some());
}

#[test]
fn choosing_no_table_writes_no_colour_data() {
    let bytes = compile_into(&[]);

    assert!(colour_tags(&bytes).is_empty());
}

#[test]
fn an_export_request_carries_its_colour_tables_to_the_file() {
    let base = sample_font();
    let font = coloured(
        base.clone(),
        vec![ColorGlyph {
            glyph: id(&base, "A"),
            paint: layer(&base, "B", solid(0, 1.0)),
        }],
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Web.woff2");

    FontExporter::new()
        .export(
            &font,
            FontExportRequest {
                path: path.clone(),
                format: ExportFormat::Woff2,
                color_tables: [ColorTable::Colr].into_iter().collect(),
            },
        )
        .unwrap();

    // The same font with every table is larger, because it adds SVG and bitmaps
    let everything = FontExporter::new()
        .compile(&font, ExportFormat::Woff2)
        .unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() < everything.len() as u64);
}
