//! Chooses which tables carry colour glyphs in an exported font.

use std::collections::BTreeSet;

use crate::export::ExportError;

/// A table that can carry colour glyphs.
///
/// A font may carry several, and a renderer uses the one it understands.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ColorTable {
    /// Vector paint graphs with palettes, read by most current renderers.
    Colr,
    /// SVG documents, read by Firefox and Safari.
    Svg,
    /// PNG bitmaps at fixed sizes, read by Apple renderers. The largest table.
    Sbix,
}

impl ColorTable {
    /// Every table, in the order that they are written.
    pub const ALL: [Self; 3] = [Self::Colr, Self::Svg, Self::Sbix];

    /// Returns the lowercase token used by commands and the bridge.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Colr => "colr",
            Self::Svg => "svg",
            Self::Sbix => "sbix",
        }
    }
}

impl TryFrom<&str> for ColorTable {
    type Error = ExportError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_ascii_lowercase().as_str() {
            "colr" => Ok(Self::Colr),
            "svg" => Ok(Self::Svg),
            "sbix" => Ok(Self::Sbix),
            table => Err(ExportError::UnsupportedColorTable {
                table: table.to_string(),
            }),
        }
    }
}

/// The set of tables that an export writes colour glyphs into.
///
/// The default is every table. Palettes are written when `COLR` or `SVG ` is
/// chosen, because both refer to them. An empty set writes no colour data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColorTables(BTreeSet<ColorTable>);

impl ColorTables {
    /// Writes no colour data.
    pub fn none() -> Self {
        Self(BTreeSet::new())
    }

    /// Returns whether `table` is written.
    pub fn contains(&self, table: ColorTable) -> bool {
        self.0.contains(&table)
    }

    /// Returns whether no table is written.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterates over the chosen tables in the order that they are written.
    pub fn iter(&self) -> impl Iterator<Item = ColorTable> + '_ {
        self.0.iter().copied()
    }
}

impl Default for ColorTables {
    fn default() -> Self {
        ColorTable::ALL.into_iter().collect()
    }
}

impl FromIterator<ColorTable> for ColorTables {
    fn from_iter<I: IntoIterator<Item = ColorTable>>(tables: I) -> Self {
        Self(tables.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_writes_every_table() {
        let tables = ColorTables::default();
        assert!(
            ColorTable::ALL
                .into_iter()
                .all(|table| tables.contains(table))
        );
    }

    #[test]
    fn tokens_round_trip_and_ignore_case() {
        for table in ColorTable::ALL {
            assert_eq!(ColorTable::try_from(table.as_str()).unwrap(), table);
        }
        assert_eq!(ColorTable::try_from("SBIX").unwrap(), ColorTable::Sbix);
    }

    #[test]
    fn an_unknown_token_names_itself_in_the_error() {
        let error = ColorTable::try_from("cbdt").unwrap_err();
        assert!(matches!(
            error,
            ExportError::UnsupportedColorTable { ref table } if table == "cbdt"
        ));
    }

    #[test]
    fn collecting_keeps_only_the_listed_tables() {
        let tables: ColorTables = [ColorTable::Svg].into_iter().collect();
        assert!(tables.contains(ColorTable::Svg));
        assert!(!tables.contains(ColorTable::Colr) && !tables.contains(ColorTable::Sbix));
        assert!(ColorTables::none().is_empty());
    }
}
