use shift_font::test_support::fixtures;

use shift_backends::{ImportLossKind, font_loader::FontLoader};
use shift_font::test_support::kerning_between;

#[test]
fn glyphs_import_reports_unrepresented_bracket_layers() {
    let path = fixtures::font("GlyphsImportLosses.glyphs");
    let import = FontLoader::new()
        .stream_font(path.to_str().unwrap())
        .expect("Glyphs source should import");

    assert_eq!(import.report().losses.len(), 1);
    let bracket_loss = &import.report().losses[0];
    assert_eq!(bracket_loss.kind, ImportLossKind::Omitted);
    assert!(bracket_loss.message.contains("2 conditional layers"));
    assert!(bracket_loss.message.contains("bracket layers"));

    let font = import
        .collect_font()
        .expect("reported source concepts must not block import");
    let glyph = font.glyph_by_name("space").expect("space should import");
    assert_eq!(glyph.layers().len(), 2);
}

#[test]
fn glyphs_import_keeps_non_default_master_kerning() {
    let path = fixtures::font("GlyphsImportLosses.glyphs");
    let font = FontLoader::new()
        .stream_font(path.to_str().unwrap())
        .expect("Glyphs source should import")
        .collect_font()
        .expect("Glyphs source should collect");

    let default_source_id = font.default_source_id().unwrap();
    let kerned_source = font
        .sources()
        .iter()
        .find(|source| source.id() != default_source_id && source.is_master())
        .expect("the second master should import");
    assert_eq!(
        kerning_between(&font, &kerned_source.id(), "space", "space"),
        Some(-10.0)
    );
    assert_eq!(
        kerning_between(&font, &default_source_id, "space", "space"),
        None
    );
}

#[test]
fn glyphs_4_palettes_import_as_integer_colours() {
    let path = fixtures::font("Glyphs4ColorPalettes.glyphs");
    let import = FontLoader::new()
        .stream_font(path.to_str().unwrap())
        .expect("a Glyphs 4 source should import");

    assert!(
        import
            .report()
            .losses
            .iter()
            .any(|loss| loss.message.contains("6 were omitted")),
        "the palette entry names are reported"
    );

    let font = import
        .collect_font()
        .expect("Glyphs 4 source should collect");
    assert_eq!(
        font.metadata().family_name.as_deref(),
        Some("Sample Gustavo COLR Colour")
    );
    let palette = &font.color().palettes[0];
    assert_eq!(palette.colors.len(), 6);
    assert_eq!(
        (
            palette.colors[0].r,
            palette.colors[0].g,
            palette.colors[0].b
        ),
        (255, 56, 60)
    );
    assert!(!font.color().glyphs.is_empty());
}

#[test]
fn glyphs_4_instances_take_their_names_from_style_names() {
    let path = fixtures::font("Glyphs4LayeredColor.glyphs");
    let font = FontLoader::new()
        .stream_font(path.to_str().unwrap())
        .expect("a Glyphs 4 source should import")
        .collect_font()
        .expect("Glyphs 4 source should collect");

    let mut names = font
        .named_instances()
        .iter()
        .map(|instance| instance.name().to_string())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["Bottom", "Front", "Side"]);
}
