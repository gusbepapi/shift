//! A `.glyphs` source with colour imports into a workspace that keeps the colour.

use shift_font::{Paint, PaintColor, Rgba};
use shift_workspace::FontWorkspace;
use tempfile::TempDir;

/// One master, one palette, and a glyph `A` whose palette layer has its own outline.
const COLOUR_FONT: &str = "{
.appVersion = \"3343\";
.formatVersion = 3;
axes = (
{
name = Weight;
tag = wght;
}
);
customParameters = (
{
name = \"Color Palettes\";
value = (
(
(200,10,20,255),
(30,40,50,255)
)
);
}
);
familyName = \"Colour Workspace\";
fontMaster = (
{
axesValues = (
40
);
id = master01;
name = Regular;
}
);
glyphs = (
{
glyphname = A;
layers = (
{
layerId = master01;
width = 600;
},
{
associatedMasterId = master01;
attr = {
colorPalette = 1;
};
layerId = master01-c1;
shapes = (
{
closed = 1;
nodes = (
(0,0,l),
(500,0,l),
(500,700,l)
);
}
);
width = 600;
}
);
unicode = 65;
}
);
unitsPerEm = 1000;
}
";

#[test]
fn imported_colour_is_kept_by_the_workspace_and_its_saved_document() {
    let dir = TempDir::new().unwrap();
    let source = dir.path().join("Colour.glyphs");
    std::fs::write(&source, COLOUR_FONT).unwrap();

    let mut workspace = FontWorkspace::open(&source, dir.path().join("import.sqlite")).unwrap();

    let color = workspace.font().color().clone();
    assert_eq!(color.palettes.len(), 1);
    assert_eq!(color.palettes[0].colors[1], Rgba::new(30, 40, 50, 255));
    let base = workspace.font().glyph_by_name("A").unwrap().id();
    let Paint::Layers { layers } = &color.glyph(&base).unwrap().paint else {
        panic!("A should be a layer stack");
    };
    assert_eq!(layers.len(), 1);
    let Paint::Glyph { glyph, fill } = &layers[0] else {
        panic!("the layer should fill a glyph");
    };
    assert_eq!(
        **fill,
        Paint::Solid {
            color: PaintColor::Palette { index: 1 },
            alpha: 1.0
        }
    );
    // The layer's own outline became a glyph that the document stores
    assert_eq!(workspace.font().glyph(glyph).unwrap().name(), "A.color1");

    let document = dir.path().join("Colour.shift");
    workspace
        .save_as_document(&document, dir.path().join("save-recovery.sqlite"))
        .unwrap();
    drop(workspace);
    let reopened =
        FontWorkspace::open_document(&document, dir.path().join("recovery.sqlite")).unwrap();

    assert_eq!(reopened.font().color(), &color);
}
