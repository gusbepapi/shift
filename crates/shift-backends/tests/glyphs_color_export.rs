//! Colour glyphs and palettes are written into `.glyphs` files that Glyphs readers and fontc understand.

use glyphs_reader::{Color, Font as ParsedFont, Glyph as ParsedGlyph, Shape};
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

fn solid(color: PaintColor, alpha: f64) -> Paint {
    Paint::Solid { color, alpha }
}

fn palette(index: u16) -> PaintColor {
    PaintColor::Palette { index }
}

fn layer(font: &Font, name: &str, fill: Paint) -> Paint {
    Paint::Glyph {
        glyph: id(font, name),
        fill: Box::new(fill),
    }
}

/// Two palettes of two entries each.
fn palettes() -> Vec<ColorPalette> {
    vec![
        ColorPalette {
            name: Some("Light".to_string()),
            colors: vec![Rgba::new(10, 20, 30, 255), Rgba::new(200, 100, 50, 255)],
        },
        ColorPalette {
            name: Some("Dark".to_string()),
            colors: vec![Rgba::new(1, 2, 3, 255), Rgba::new(4, 5, 6, 128)],
        },
    ]
}

/// Exports the font with one colour glyph for `A` and parses the text that comes out.
fn export(base: &Font, layers: Vec<Paint>) -> (String, ParsedFont) {
    let mut font = base.clone();
    font.set_color(ColorData {
        palettes: palettes(),
        glyphs: vec![ColorGlyph {
            glyph: id(base, "A"),
            paint: Paint::Layers { layers },
        }],
    })
    .unwrap();
    let bytes = FontExporter::new()
        .compile(&font, ExportFormat::Glyphs)
        .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let parsed = ParsedFont::load_from_string(&text).unwrap();
    (text, parsed)
}

fn glyph<'a>(font: &'a ParsedFont, name: &str) -> &'a ParsedGlyph {
    font.glyphs.get(name).unwrap()
}

/// The names of the components of a layer.
fn components(layer: &glyphs_reader::Layer) -> Vec<String> {
    layer
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Component(component) => Some(component.name.to_string()),
            Shape::Path(_) => None,
        })
        .collect()
}

#[test]
fn palettes_are_written_as_the_color_palettes_parameter() {
    let base = sample_font();
    let (_, parsed) = export(&base, vec![layer(&base, "B", solid(palette(0), 1.0))]);

    let palettes = parsed.color_palettes().unwrap();

    assert_eq!(palettes.len(), 2);
    assert_eq!(palettes[0][1], Color::rgba(200, 100, 50, 255));
    assert_eq!(palettes[1][1], Color::rgba(4, 5, 6, 128));
}

#[test]
fn opaque_solid_layers_become_palette_layers_beside_the_master_layers() {
    let base = sample_font();
    let (_, parsed) = export(
        &base,
        vec![
            layer(&base, "B", solid(palette(1), 1.0)),
            layer(&base, "acute", solid(PaintColor::Foreground, 1.0)),
        ],
    );
    let glyph = glyph(&parsed, "A");

    // The master layers keep their outlines, so the monochrome glyph is unchanged
    let masters: Vec<_> = glyph.layers.iter().filter(|l| l.is_master()).collect();
    assert!(!masters.is_empty());
    assert!(masters.iter().all(|layer| !layer.attributes.color));

    let colour_layers: Vec<_> = glyph
        .layers
        .iter()
        .filter(|l| l.attributes.color_palette.is_some())
        .collect();
    assert_eq!(colour_layers.len(), 2 * masters.len());
    let first_master = masters[0].layer_id.clone();
    let of_first: Vec<_> = colour_layers
        .iter()
        .filter(|l| l.associated_master_id.as_deref() == Some(first_master.as_str()))
        .collect();
    assert_eq!(of_first[0].attributes.color_palette, Some(1));
    assert_eq!(components(of_first[0]), ["B"]);
    // The foreground colour is the palette index 0xFFFF, written as "*"
    assert_eq!(of_first[1].attributes.color_palette, Some(0xFFFF));
    assert_eq!(components(of_first[1]), ["acute"]);
}

#[test]
fn gradients_and_translucent_fills_become_color_layers_with_literal_colours() {
    let base = sample_font();
    let stops = vec![
        ColorStop {
            offset: 0.0,
            color: palette(0),
            alpha: 1.0,
        },
        ColorStop {
            offset: 1.0,
            color: palette(1),
            alpha: 0.5,
        },
    ];
    let (_, parsed) = export(
        &base,
        vec![
            layer(&base, "B", solid(palette(0), 0.5)),
            layer(
                &base,
                "acute",
                Paint::LinearGradient {
                    stops: stops.clone(),
                    p0: ColorPoint::new(300.0, 0.0),
                    p1: ColorPoint::new(300.0, 700.0),
                    p2: None,
                },
            ),
            layer(
                &base,
                "B",
                Paint::RadialGradient {
                    stops,
                    c0: ColorPoint::new(300.0, 300.0),
                    r0: None,
                    c1: ColorPoint::new(300.0, 350.0),
                    r1: None,
                },
            ),
        ],
    );
    let glyph = glyph(&parsed, "A");
    let master = glyph.layers.iter().find(|l| l.is_master()).unwrap();

    assert!(master.attributes.color);
    // The solid fill refers to its glyph, and each gradient carries the outline that fontc measures
    assert_eq!(master.shapes.len(), 3);
    assert_eq!(components(master), ["B"]);
    assert!(matches!(master.shapes[1], Shape::Path(_)));
    assert!(matches!(master.shapes[2], Shape::Path(_)));
    let attributes = |index: usize| master.shapes[index].attributes();

    // Palette 0 entry 0 with the layer opacity multiplied into its alpha
    assert_eq!(attributes(0).fill_color, Some(Color::rgba(10, 20, 30, 128)));

    // Points are fractions of the outline's bounds, x 100 to 500 and y 0 to 700
    let linear = attributes(1).gradient.as_ref().unwrap();
    assert_eq!(linear.style, "");
    assert_eq!(
        linear.start.iter().map(|v| v.0).collect::<Vec<_>>(),
        [0.5, 0.0]
    );
    assert_eq!(
        linear.end.iter().map(|v| v.0).collect::<Vec<_>>(),
        [0.5, 1.0]
    );
    assert_eq!(linear.colors[0].color, Color::rgba(10, 20, 30, 255));
    assert_eq!(linear.colors[1].color, Color::rgba(200, 100, 50, 128));
    assert_eq!(linear.colors[1].stop_offset.0, 1.0);

    let radial = attributes(2).gradient.as_ref().unwrap();
    assert_eq!(radial.style, "circle");
    assert_eq!(radial.start[0].0, 0.5);
    assert!((radial.start[1].0 - 0.5).abs() < 1e-9);
}

#[test]
fn fonts_without_colour_glyphs_write_no_colour_data() {
    let base = sample_font();
    let bytes = FontExporter::new()
        .compile(&base, ExportFormat::Glyphs)
        .unwrap();
    let text = String::from_utf8(bytes).unwrap();

    assert!(!text.contains("Color Palettes"));
    assert!(!text.contains("colorPalette"));
}

/// Compiles Glyphs text with fontc, as Glyphs-based pipelines do.
fn compile_with_fontc(text: &str) -> Vec<u8> {
    let source = fontc::Input::from_glyphs(text.to_string())
        .create_source()
        .unwrap();
    fontc::generate_font(source, fontc::Options::default()).unwrap()
}

#[test]
fn fontc_compiles_palette_layers_into_colr_version_0_with_the_palettes() {
    let base = sample_font();
    let (text, _) = export(
        &base,
        vec![
            layer(&base, "B", solid(palette(1), 1.0)),
            layer(&base, "acute", solid(PaintColor::Foreground, 1.0)),
        ],
    );

    let bytes = compile_with_fontc(&text);
    let font = FontRef::new(&bytes).unwrap();

    let cpal = font.cpal().unwrap();
    assert_eq!(cpal.num_palettes(), 2);
    assert_eq!(cpal.num_palette_entries(), 2);
    let colr = font.colr().unwrap();
    assert_eq!(colr.num_base_glyph_records(), 1);
    let layers = colr.layer_records().unwrap().unwrap();
    assert_eq!(
        layers
            .iter()
            .map(write_fonts::read::tables::colr::Layer::palette_index)
            .collect::<Vec<_>>(),
        [1, 0xFFFF]
    );
}

#[test]
fn fontc_compiles_color_layers_into_a_colr_version_1_paint_graph() {
    let base = sample_font();
    let stops = vec![
        ColorStop {
            offset: 0.0,
            color: palette(0),
            alpha: 1.0,
        },
        ColorStop {
            offset: 1.0,
            color: palette(1),
            alpha: 1.0,
        },
    ];
    let (text, _) = export(
        &base,
        vec![
            layer(&base, "B", solid(palette(0), 0.5)),
            layer(
                &base,
                "acute",
                Paint::LinearGradient {
                    stops,
                    p0: ColorPoint::new(300.0, 0.0),
                    p1: ColorPoint::new(300.0, 700.0),
                    p2: None,
                },
            ),
        ],
    );

    let bytes = compile_with_fontc(&text);
    let font = FontRef::new(&bytes).unwrap();

    let colr = font.colr().unwrap();
    let list = colr.base_glyph_list().unwrap().unwrap();
    assert_eq!(list.num_base_glyph_paint_records(), 1);

    // fontc rebuilds the absolute geometry that Shift authored from the fractions
    let layer_list = colr.layer_list().unwrap().unwrap();
    let ReadPaint::Glyph(second) = layer_list.paints().get(1).unwrap() else {
        panic!("expected a glyph paint");
    };
    let ReadPaint::LinearGradient(gradient) = second.paint().unwrap() else {
        panic!("expected a linear gradient");
    };
    assert_eq!(
        (
            gradient.x0().to_i16(),
            gradient.y0().to_i16(),
            gradient.x1().to_i16(),
            gradient.y1().to_i16()
        ),
        (300, 0, 300, 700)
    );

    // The translucent fill is a palette entry of its own, with the alpha in the colour
    let cpal = font.cpal().unwrap();
    let records = cpal.color_records_array().unwrap().unwrap();
    assert!(
        records
            .iter()
            .any(|record| (record.red(), record.alpha()) == (10, 128))
    );
}

/// Returns the geometry of the first two gradient layers of the colour glyph `A`.
fn gradient_geometry(bytes: &[u8]) -> (Vec<i32>, Vec<i32>) {
    let font = FontRef::new(bytes).unwrap();
    let layer_list = font.colr().unwrap().layer_list().unwrap().unwrap();
    let ReadPaint::Glyph(first) = layer_list.paints().get(0).unwrap() else {
        panic!("expected a glyph paint");
    };
    let ReadPaint::LinearGradient(linear) = first.paint().unwrap() else {
        panic!("expected a linear gradient");
    };
    let ReadPaint::Glyph(second) = layer_list.paints().get(1).unwrap() else {
        panic!("expected a glyph paint");
    };
    let ReadPaint::RadialGradient(radial) = second.paint().unwrap() else {
        panic!("expected a radial gradient");
    };
    (
        vec![
            linear.x0().to_i16().into(),
            linear.y0().to_i16().into(),
            linear.x1().to_i16().into(),
            linear.y1().to_i16().into(),
            linear.x2().to_i16().into(),
            linear.y2().to_i16().into(),
        ],
        vec![
            radial.x0().to_i16().into(),
            radial.y0().to_i16().into(),
            radial.radius0().to_u16().into(),
            radial.x1().to_i16().into(),
            radial.y1().to_i16().into(),
            radial.radius1().to_u16().into(),
        ],
    )
}

#[test]
fn the_glyphs_file_compiles_to_the_gradient_geometry_that_shift_compiles() {
    let base = sample_font();
    let stops = vec![
        ColorStop {
            offset: 0.0,
            color: palette(0),
            alpha: 1.0,
        },
        ColorStop {
            offset: 1.0,
            color: palette(1),
            alpha: 1.0,
        },
    ];
    let layers = vec![
        layer(
            &base,
            "B",
            Paint::LinearGradient {
                stops: stops.clone(),
                p0: ColorPoint::new(150.0, 100.0),
                p1: ColorPoint::new(450.0, 600.0),
                p2: None,
            },
        ),
        layer(
            &base,
            "acute",
            Paint::RadialGradient {
                stops,
                c0: ColorPoint::new(300.0, 350.0),
                r0: None,
                c1: ColorPoint::new(300.0, 350.0),
                r1: None,
            },
        ),
    ];
    let (text, _) = export(&base, layers.clone());

    let mut font = base.clone();
    font.set_color(ColorData {
        palettes: palettes(),
        glyphs: vec![ColorGlyph {
            glyph: id(&base, "A"),
            paint: Paint::Layers { layers },
        }],
    })
    .unwrap();
    let shift = FontExporter::new()
        .compile(&font, ExportFormat::Ttf)
        .unwrap();

    let (shift_linear, shift_radial) = gradient_geometry(&shift);
    let (glyphs_linear, glyphs_radial) = gradient_geometry(&compile_with_fontc(&text));

    // fontc rounds the fractions it scales, so the coordinates may differ by a unit
    let close = |a: &[i32], b: &[i32]| a.iter().zip(b).all(|(a, b)| (a - b).abs() <= 1);
    assert!(
        close(&shift_linear, &glyphs_linear),
        "{shift_linear:?} {glyphs_linear:?}"
    );
    assert!(
        close(&shift_radial, &glyphs_radial),
        "{shift_radial:?} {glyphs_radial:?}"
    );
}
