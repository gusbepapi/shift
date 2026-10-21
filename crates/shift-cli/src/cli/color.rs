use std::path::PathBuf;

use clap::{Args, Subcommand, ValueHint};

use super::MutationArgs;

#[derive(Debug, Subcommand)]
pub(crate) enum ColorCommand {
    /// Author the colour palettes shared by every colour glyph.
    Palette {
        #[command(subcommand)]
        command: PaletteCommand,
    },
    /// Author colour glyphs: base glyphs painted from layers of other glyphs.
    Glyph {
        #[command(subcommand)]
        command: ColorGlyphCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum PaletteCommand {
    /// Replace every palette. All palettes need the same number of colours.
    Set(SetPalettesArgs),
}

#[derive(Debug, Args)]
pub(crate) struct SetPalettesArgs {
    /// Path to the canonical SQLite .shift document.
    #[arg(value_hint = ValueHint::FilePath)]
    pub path: PathBuf,

    /// One palette as [NAME:]#RRGGBB[AA],#RRGGBB[AA],...; repeat for more palettes.
    #[arg(long = "palette", value_name = "[NAME:]COLORS")]
    pub palettes: Vec<String>,

    /// Remove every palette.
    #[arg(long, conflicts_with = "palettes")]
    pub clear: bool,

    #[command(flatten)]
    pub mutation: MutationArgs,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ColorGlyphCommand {
    /// Create or replace the layers that paint one base glyph.
    Set(SetColorGlyphArgs),
    /// Remove the colour of one base glyph, leaving its outline.
    Remove(RemoveColorGlyphArgs),
}

#[derive(Debug, Args)]
pub(crate) struct SetColorGlyphArgs {
    /// Path to the canonical SQLite .shift document.
    #[arg(value_hint = ValueHint::FilePath)]
    pub path: PathBuf,

    /// Base glyph name or full stable glyph id.
    pub glyph: String,

    /// One layer from bottom to top as GLYPH=FILL; repeat for more layers.
    ///
    /// FILL is palette:INDEX[@ALPHA], foreground[@ALPHA],
    /// linear:X0,Y0,X1,Y1:STOPS, or radial:X0,Y0,R0,X1,Y1,R1:STOPS, where
    /// STOPS is OFFSET=COLOUR joined by /, for example 0=palette:0/1=palette:1@0.5.
    #[arg(long = "layer", value_name = "GLYPH=FILL", required = true)]
    pub layers: Vec<String>,

    #[command(flatten)]
    pub mutation: MutationArgs,
}

#[derive(Debug, Args)]
pub(crate) struct RemoveColorGlyphArgs {
    /// Path to the canonical SQLite .shift document.
    #[arg(value_hint = ValueHint::FilePath)]
    pub path: PathBuf,

    /// Base glyph name or full stable glyph id.
    pub glyph: String,

    #[command(flatten)]
    pub mutation: MutationArgs,
}
