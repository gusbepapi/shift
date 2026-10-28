//! Palettes and colour glyphs of `.glyphs` files import into Shift's colour data.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use shift_backends::{ExportFormat, FontExporter, ImportLossKind, font_loader::FontLoader};
use shift_font::{
    ColorData, ColorGlyph, ColorPalette, ColorPoint, ColorStop, Contour, Font, Glyph, GlyphId,
    GlyphLayer, LayerId, Paint, PaintColor, PointType, Rgba, test_support::sample_variable_font,
};

/// Numbers the scratch files, because tests of one process run in parallel and may share a name.
static NEXT_SCRATCH: AtomicUsize = AtomicUsize::new(0);

/// A scratch file that is removed when it goes out of scope.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str, text: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "shift-colour-import-{}-{}-{name}.glyphs",
            std::process::id(),
            NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, text).unwrap();
        Self(path)
    }

    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn import(name: &str, text: &str) -> Font {
    let file = Scratch::new(name, text);
    FontLoader::new().read_font(file.path()).unwrap()
}

fn name_of(font: &Font, id: GlyphId) -> String {
    font.glyph(id).unwrap().name().to_string()
}

/// The layer glyphs of a colour glyph by name, with their fills.
fn layers_of(font: &Font, base: &str) -> Vec<(String, Paint)> {
    let entry = font
        .color()
        .glyph(font.glyph_by_name(base).unwrap().id())
        .unwrap();
    let Paint::Layers { layers } = &entry.paint else {
        panic!("colour glyph {base} should be a layer stack");
    };
    layers
        .iter()
        .map(|layer| match layer {
            Paint::Glyph { glyph, fill } => (name_of(font, *glyph), (**fill).clone()),
            other => panic!("unexpected layer {other:?}"),
        })
        .collect()
}

fn palette(index: u16) -> PaintColor {
    PaintColor::Palette { index }
}

fn solid(color: PaintColor) -> Paint {
    Paint::Solid { color, alpha: 1.0 }
}

// A hand-written Glyphs 3 file in the shape the Glyphs application saves

fn path(nodes: &[(i32, i32)], attr: &str) -> String {
    let nodes = nodes
        .iter()
        .map(|(x, y)| format!("({x},{y},l)"))
        .collect::<Vec<_>>()
        .join(",\n");
    format!("{{\n{attr}closed = 1;\nnodes = (\n{nodes}\n);\n}}")
}

fn palette_layer(master: &str, id: &str, palette: &str, shapes: &str) -> String {
    format!(
        "{{\nassociatedMasterId = {master};\nattr = {{\ncolorPalette = {palette};\n}};\nlayerId = {id};\nshapes = (\n{shapes}\n);\nwidth = 600;\n}}"
    )
}

fn colour_layer(master: &str, id: &str, shapes: &str) -> String {
    format!(
        "{{\nassociatedMasterId = {master};\nattr = {{\ncolor = 1;\n}};\nlayerId = {id};\nshapes = (\n{shapes}\n);\nwidth = 600;\n}}"
    )
}

fn master_layer(master: &str, shapes: &str) -> String {
    format!("{{\nlayerId = {master};\nshapes = (\n{shapes}\n);\nwidth = 600;\n}}")
}

/// Two masters, one palette of two entries, a palette glyph `A` and a colour glyph `B`.
fn authored_file() -> String {
    let triangle = path(&[(100, 0), (500, 0), (300, 700)], "");
    let bar = path(&[(0, 0), (600, 0), (600, 100), (0, 100)], "");
    let square = path(&[(0, 0), (200, 0), (200, 400), (0, 400)], "");
    let orange = path(
        &[(0, 0), (100, 0), (100, 100), (0, 100)],
        "attr = {\nfillColor = (255,128,0,255);\n};\n",
    );
    let gradient = path(
        &[(0, 0), (200, 0), (200, 400), (0, 400)],
        "attr = {\ngradient = {\ncolors = (\n((0,0,0,255),0),\n((255,255,255,255),1)\n);\nend = (1,1);\nstart = (0,0);\n};\n};\n",
    );

    let mut a_layers = Vec::new();
    let mut b_layers = Vec::new();
    for master in ["master01", "master02"] {
        a_layers.push(master_layer(master, &triangle));
        a_layers.push(palette_layer(master, &format!("{master}-a1"), "1", &bar));
        a_layers.push(palette_layer(
            master,
            &format!("{master}-a2"),
            "\"*\"",
            &square,
        ));
        a_layers.push(palette_layer(master, &format!("{master}-a3"), "7", &square));
        b_layers.push(master_layer(master, &square));
        b_layers.push(colour_layer(
            master,
            &format!("{master}-b"),
            &format!("{orange},\n{gradient}"),
        ));
    }

    format!(
        "{{\n.appVersion = \"3343\";\n.formatVersion = 3;\naxes = (\n{{\nname = Weight;\ntag = wght;\n}}\n);\ncustomParameters = (\n{{\nname = \"Color Palettes\";\nvalue = (\n(\n(255,0,0,255),\n(0,0,255,255)\n)\n);\n}}\n);\nfamilyName = \"Colour Import\";\nfontMaster = (\n{{\naxesValues = (\n40\n);\nid = master01;\nname = Regular;\n}},\n{{\naxesValues = (\n200\n);\nid = master02;\nname = Bold;\n}}\n);\nglyphs = (\n{{\nglyphname = A;\nlayers = (\n{}\n);\nunicode = 65;\n}},\n{{\nglyphname = B;\nlayers = (\n{}\n);\nunicode = 66;\n}}\n);\nunitsPerEm = 1000;\n}}\n",
        a_layers.join(",\n"),
        b_layers.join(",\n")
    )
}

#[test]
fn palettes_come_from_the_color_palettes_parameter() {
    let font = import("palettes", &authored_file());

    let palettes = &font.color().palettes;

    assert_eq!(palettes.len(), 1);
    assert_eq!(palettes[0].colors[0], Rgba::new(255, 0, 0, 255));
    assert_eq!(palettes[0].colors[1], Rgba::new(0, 0, 255, 255));
}

#[test]
fn palette_layers_with_their_own_outlines_become_new_layer_glyphs() {
    let font = import("palette-layers", &authored_file());

    let layers = layers_of(&font, "A");

    // The third palette layer names entry 7, which the one palette lacks
    assert_eq!(
        layers,
        vec![
            ("A.color1".to_string(), solid(palette(1))),
            ("A.color2".to_string(), solid(PaintColor::Foreground)),
        ]
    );
    let bar = font.glyph_by_name("A.color1").unwrap();
    assert_eq!(bar.layers().len(), 2, "one layer in each master");
    for layer in bar.layers().values() {
        assert_eq!(layer.contours_iter().count(), 1);
    }
}

#[test]
fn colour_layer_shapes_become_layers_with_literal_colours_appended_to_the_palettes() {
    let font = import("colour-layer", &authored_file());

    let layers = layers_of(&font, "B");

    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].0, "B.color1");
    assert_eq!(layers[0].1, solid(palette(2)));
    let colors = &font.color().palettes[0].colors;
    assert_eq!(colors[2], Rgba::new(255, 128, 0, 255));

    let (name, Paint::LinearGradient { stops, p0, p1, p2 }) = &layers[1] else {
        panic!("second layer should be a linear gradient: {:?}", layers[1]);
    };
    assert_eq!(name, "B.color2");
    assert_eq!(*p2, None);
    // The fractions (0,0) and (1,1) span the 200 by 400 bounds of the shape
    assert_eq!((p0.x, p0.y), (0.0, 0.0));
    assert_eq!((p1.x, p1.y), (200.0, 400.0));
    assert_eq!(stops.len(), 2);
    let resolved = |stop: &ColorStop| match stop.color {
        PaintColor::Palette { index } => colors[usize::from(index)],
        PaintColor::Foreground => panic!("stops use palette entries"),
    };
    assert_eq!(resolved(&stops[0]), Rgba::new(0, 0, 0, 255));
    assert_eq!(resolved(&stops[1]), Rgba::new(255, 255, 255, 255));
}

#[test]
fn imported_colour_data_is_valid_for_the_glyphs_that_exist() {
    let font = import("valid", &authored_file());

    font.color()
        .validate(|id| font.glyph(*id).is_some())
        .unwrap();
    // The two outline glyphs of the file, the two palette glyphs and the two colour glyphs
    assert_eq!(font.glyph_count(), 6);
}

#[test]
fn the_import_report_lists_what_the_colour_import_left_out() {
    let file = Scratch::new("report", &authored_file());
    let import = FontLoader::new().stream_font(file.path()).unwrap();

    let messages: Vec<_> = import
        .report()
        .losses
        .iter()
        .map(|loss| (loss.kind, loss.message.as_str()))
        .collect();

    assert!(messages.iter().any(|(kind, message)| {
        *kind == ImportLossKind::Omitted && message.contains("1 Glyphs palette layers")
    }));
    assert!(messages.iter().any(|(kind, message)| {
        *kind == ImportLossKind::Approximated && message.contains("default master only")
    }));
}

#[test]
fn the_directory_declares_the_new_glyphs_that_the_colour_data_names() {
    let file = Scratch::new("directory", &authored_file());
    let import = FontLoader::new().stream_font(file.path()).unwrap();

    let declared: Vec<_> = import
        .directory()
        .into_iter()
        .map(|entry| entry.glyph_id)
        .collect();
    let color = import.color();

    assert_eq!(import.glyph_count(), declared.len());
    color.validate(|id| declared.contains(id)).unwrap();
    assert!(!color.is_empty());
}

#[test]
fn fonts_without_colour_import_no_colour_data() {
    let text = "{\n.formatVersion = 3;\naxes = (\n{\nname = Weight;\ntag = wght;\n}\n);\nfamilyName = \"Plain\";\nfontMaster = (\n{\naxesValues = (\n40\n);\nid = master01;\nname = Regular;\n}\n);\nglyphs = (\n{\nglyphname = A;\nlayers = (\n{\nlayerId = master01;\nwidth = 600;\n}\n);\nunicode = 65;\n}\n);\nunitsPerEm = 1000;\n}\n";

    let font = import("plain", text);

    assert!(font.color().is_empty());
}

// Round trips through Shift's own export

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

fn layer(font: &Font, name: &str, fill: Paint) -> Paint {
    Paint::Glyph {
        glyph: id(font, name),
        fill: Box::new(fill),
    }
}

/// Exports `original` with one colour glyph for `A` and imports the file again.
fn round_trip(mut original: Font, layers: Vec<Paint>, palettes: Vec<ColorPalette>) -> Font {
    original
        .set_color(ColorData {
            palettes,
            glyphs: vec![ColorGlyph {
                glyph: id(&original, "A"),
                paint: Paint::Layers { layers },
            }],
        })
        .unwrap();
    let bytes = FontExporter::new()
        .compile(&original, ExportFormat::Glyphs)
        .unwrap();
    import("round-trip", &String::from_utf8(bytes).unwrap())
}

fn two_palettes() -> Vec<ColorPalette> {
    vec![
        ColorPalette {
            name: None,
            colors: vec![Rgba::new(10, 20, 30, 255), Rgba::new(200, 100, 50, 255)],
        },
        ColorPalette {
            name: None,
            colors: vec![Rgba::new(1, 2, 3, 255), Rgba::new(4, 5, 6, 128)],
        },
    ]
}

#[test]
fn palette_colour_glyphs_survive_an_export_and_import_unchanged() {
    let original = sample_font();
    let layers = vec![
        layer(&original, "B", solid(palette(1))),
        layer(&original, "acute", solid(PaintColor::Foreground)),
    ];

    let imported = round_trip(original.clone(), layers, two_palettes());

    assert_eq!(imported.color().palettes, two_palettes());
    // The exported layers are components of existing glyphs, so no glyph is added
    assert_eq!(imported.glyph_count(), original.glyph_count());
    assert_eq!(
        layers_of(&imported, "A"),
        vec![
            ("B".to_string(), solid(palette(1))),
            ("acute".to_string(), solid(PaintColor::Foreground)),
        ]
    );
}

#[test]
fn gradient_geometry_survives_an_export_and_import() {
    let original = sample_font();
    let gradient = Paint::LinearGradient {
        stops: vec![
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
        ],
        p0: ColorPoint::new(100.0, 0.0),
        p1: ColorPoint::new(500.0, 700.0),
        p2: None,
    };
    let layers = vec![layer(&original, "B", gradient)];

    let imported = round_trip(original, layers, two_palettes());

    let imported_layers = layers_of(&imported, "A");
    assert_eq!(imported_layers.len(), 1);
    let Paint::LinearGradient { stops, p0, p1, .. } = &imported_layers[0].1 else {
        panic!("the gradient should stay a linear gradient");
    };
    let close = |left: f64, right: f64| (left - right).abs() < 1e-6;
    assert!(close(p0.x, 100.0) && close(p0.y, 0.0), "{p0:?}");
    assert!(close(p1.x, 500.0) && close(p1.y, 700.0), "{p1:?}");
    let colors = &imported.color().palettes[0].colors;
    let resolved = |stop: &ColorStop| match stop.color {
        PaintColor::Palette { index } => colors[usize::from(index)],
        PaintColor::Foreground => panic!("stops use palette entries"),
    };
    // Literal colours come from palette 0 of the exported font
    assert_eq!(resolved(&stops[0]), Rgba::new(10, 20, 30, 255));
    assert_eq!(resolved(&stops[1]), Rgba::new(200, 100, 50, 255));
}

#[test]
fn translucent_fills_import_as_palette_entries_that_carry_the_opacity() {
    let original = sample_font();
    let layers = vec![layer(
        &original,
        "B",
        Paint::Solid {
            color: palette(0),
            alpha: 0.5,
        },
    )];

    let imported = round_trip(original, layers, two_palettes());

    let imported_layers = layers_of(&imported, "A");
    let Paint::Solid {
        color: PaintColor::Palette { index },
        alpha,
    } = imported_layers[0].1
    else {
        panic!("the fill should be a palette entry");
    };
    assert_eq!(alpha, 1.0);
    let entry = imported.color().palettes[0].colors[usize::from(index)];
    assert_eq!((entry.r, entry.g, entry.b), (10, 20, 30));
    assert_eq!(entry.a, 128);
}
