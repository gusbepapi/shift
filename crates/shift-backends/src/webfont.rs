//! Packs an SFNT binary into the WOFF and WOFF2 web font containers.
//!
//! WOFF compresses every table with zlib. WOFF2 compresses all tables as one
//! Brotli stream; `glyf` and `loca` are stored with the null transform, which the
//! specification allows, so each table decompresses to exactly its SFNT bytes.

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use write_fonts::read::{FontRef, TableProvider};

/// Describes a failure to pack an SFNT binary into a web font container.
#[derive(Debug, thiserror::Error)]
pub enum WebFontError {
    #[error("cannot read the font to pack: {message}")]
    ReadFont { message: String },

    #[error("cannot compress the font: {message}")]
    Compress { message: String },
}

fn read_error(error: &impl std::fmt::Display) -> WebFontError {
    WebFontError::ReadFont {
        message: error.to_string(),
    }
}

fn compress_error(error: &impl std::fmt::Display) -> WebFontError {
    WebFontError::Compress {
        message: error.to_string(),
    }
}

/// One SFNT table as it sits in the source binary.
struct SourceTable<'a> {
    tag: [u8; 4],
    checksum: u32,
    data: &'a [u8],
}

struct Source<'a> {
    flavor: u32,
    version: (u16, u16),
    tables: Vec<SourceTable<'a>>,
}

impl<'a> Source<'a> {
    fn read(sfnt: &'a [u8]) -> Result<Self, WebFontError> {
        let font = FontRef::new(sfnt).map_err(|error| read_error(&error))?;
        let revision = font
            .head()
            .map_err(|error| read_error(&error))?
            .font_revision();
        let raw = revision.to_bits();

        let mut tables = Vec::new();
        for record in font.table_directory.table_records() {
            let tag = record.tag().to_be_bytes();
            let offset = record.offset() as usize;
            let length = record.length() as usize;
            let data = sfnt
                .get(offset..offset.saturating_add(length))
                .ok_or_else(|| WebFontError::ReadFont {
                    message: format!("table {} lies outside the file", tag_text(tag)),
                })?;
            tables.push(SourceTable {
                tag,
                checksum: record.checksum(),
                data,
            });
        }
        tables.sort_by_key(|table| table.tag);

        Ok(Self {
            flavor: font.table_directory.sfnt_version(),
            // The font revision is 16.16 fixed point; its halves give the container version
            version: ((raw >> 16) as u16, (raw & 0xffff) as u16),
            tables,
        })
    }

    /// The size of the SFNT these tables came from, with every table padded to four bytes.
    fn sfnt_size(&self) -> u32 {
        let directory = 12 + 16 * self.tables.len();
        let data: usize = self
            .tables
            .iter()
            .map(|table| padded(table.data.len()))
            .sum();
        (directory + data) as u32
    }
}

fn tag_text(tag: [u8; 4]) -> String {
    String::from_utf8_lossy(&tag).into_owned()
}

fn padded(length: usize) -> usize {
    length.div_ceil(4) * 4
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn pad_to_four(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

/// Packs an SFNT binary into a WOFF 1.0 file.
///
/// # Errors
///
/// Returns [`WebFontError`] when the binary cannot be read or a table cannot be compressed.
pub fn to_woff(sfnt: &[u8]) -> Result<Vec<u8>, WebFontError> {
    const HEADER: usize = 44;
    const ENTRY: usize = 20;

    let source = Source::read(sfnt)?;

    // A table stays uncompressed when compression would not make it smaller
    let mut stored = Vec::with_capacity(source.tables.len());
    for table in &source.tables {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder
            .write_all(table.data)
            .map_err(|error| compress_error(&error))?;
        let compressed = encoder.finish().map_err(|error| compress_error(&error))?;
        stored.push(if compressed.len() < table.data.len() {
            compressed
        } else {
            table.data.to_vec()
        });
    }

    let mut offset = HEADER + ENTRY * source.tables.len();
    let mut directory = Vec::new();
    for (table, data) in source.tables.iter().zip(&stored) {
        directory.extend_from_slice(&table.tag);
        push_u32(&mut directory, offset as u32);
        push_u32(&mut directory, data.len() as u32);
        push_u32(&mut directory, table.data.len() as u32);
        push_u32(&mut directory, table.checksum);
        offset += padded(data.len());
    }

    let mut out = Vec::with_capacity(offset);
    out.extend_from_slice(b"wOFF");
    push_u32(&mut out, source.flavor);
    push_u32(&mut out, offset as u32);
    push_u16(&mut out, source.tables.len() as u16);
    push_u16(&mut out, 0);
    push_u32(&mut out, source.sfnt_size());
    push_u16(&mut out, source.version.0);
    push_u16(&mut out, source.version.1);
    // No metadata block and no private data
    for _ in 0..5 {
        push_u32(&mut out, 0);
    }
    out.extend_from_slice(&directory);
    for data in &stored {
        out.extend_from_slice(data);
        pad_to_four(&mut out);
    }
    Ok(out)
}

/// The tags that a WOFF2 directory entry names by index instead of spelling out.
const KNOWN_TAGS: [&[u8; 4]; 63] = [
    b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"name", b"OS/2", b"post", b"cvt ", b"fpgm",
    b"glyf", b"loca", b"prep", b"CFF ", b"VORG", b"EBDT", b"EBLC", b"gasp", b"hdmx", b"kern",
    b"LTSH", b"PCLT", b"VDMX", b"vhea", b"vmtx", b"BASE", b"GDEF", b"GPOS", b"GSUB", b"EBSC",
    b"JSTF", b"MATH", b"CBDT", b"CBLC", b"COLR", b"CPAL", b"SVG ", b"sbix", b"acnt", b"avar",
    b"bdat", b"bloc", b"bsln", b"cvar", b"fdsc", b"feat", b"fmtx", b"fvar", b"gvar", b"hsty",
    b"just", b"lcar", b"mort", b"morx", b"opbd", b"prop", b"trak", b"Zapf", b"Silf", b"Glat",
    b"Gloc", b"Feat", b"Sill",
];

const USER_TAG: u8 = 0x3f;
const NULL_TRANSFORM_GLYF_LOCA: u8 = 0xc0;

/// Writes a WOFF2 `UIntBase128`: seven bits per byte, most significant first.
fn push_base128(out: &mut Vec<u8>, value: u32) {
    let mut groups = vec![(value & 0x7f) as u8];
    let mut rest = value >> 7;
    while rest > 0 {
        groups.push(((rest & 0x7f) as u8) | 0x80);
        rest >>= 7;
    }
    groups.reverse();
    out.extend_from_slice(&groups);
}

/// Packs an SFNT binary into a WOFF2 file.
///
/// # Errors
///
/// Returns [`WebFontError`] when the binary cannot be read or compressed.
pub fn to_woff2(sfnt: &[u8]) -> Result<Vec<u8>, WebFontError> {
    const HEADER: usize = 48;

    let mut source = Source::read(sfnt)?;
    // The specification wants `loca` right behind `glyf` in the directory
    if let Some(loca) = source.tables.iter().position(|table| table.tag == *b"loca")
        && let Some(glyf) = source.tables.iter().position(|table| table.tag == *b"glyf")
    {
        let table = source.tables.remove(loca);
        let glyf = if loca < glyf { glyf - 1 } else { glyf };
        source.tables.insert(glyf + 1, table);
    }

    let mut directory = Vec::new();
    let mut stream = Vec::new();
    for table in &source.tables {
        let index = KNOWN_TAGS.iter().position(|known| **known == table.tag);
        let flags = match (index, table.tag) {
            (Some(index), tag) if tag == *b"glyf" || tag == *b"loca" => {
                index as u8 | NULL_TRANSFORM_GLYF_LOCA
            }
            (Some(index), _) => index as u8,
            (None, _) => USER_TAG,
        };
        directory.push(flags);
        if index.is_none() {
            directory.extend_from_slice(&table.tag);
        }
        push_base128(&mut directory, table.data.len() as u32);
        stream.extend_from_slice(table.data);
    }

    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        lgwin: 22,
        mode: brotli::enc::backward_references::BrotliEncoderMode::BROTLI_MODE_FONT,
        ..Default::default()
    };
    let mut compressed = Vec::new();
    brotli::BrotliCompress(&mut stream.as_slice(), &mut compressed, &params)
        .map_err(|error| compress_error(&error))?;

    let mut length = HEADER + directory.len() + compressed.len();
    length = padded(length);

    let mut out = Vec::with_capacity(length);
    out.extend_from_slice(b"wOF2");
    push_u32(&mut out, source.flavor);
    push_u32(&mut out, length as u32);
    push_u16(&mut out, source.tables.len() as u16);
    push_u16(&mut out, 0);
    push_u32(&mut out, source.sfnt_size());
    push_u32(&mut out, compressed.len() as u32);
    push_u16(&mut out, source.version.0);
    push_u16(&mut out, source.version.1);
    for _ in 0..5 {
        push_u32(&mut out, 0);
    }
    out.extend_from_slice(&directory);
    out.extend_from_slice(&compressed);
    pad_to_four(&mut out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn u16_at(bytes: &[u8], offset: usize) -> u16 {
        u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap())
    }

    /// Compiles a small static font to use as a container payload.
    fn sample_sfnt() -> Vec<u8> {
        use crate::FontExporter;
        use shift_font::test_support::sample_variable_font;

        FontExporter::new()
            .compile_ttf(&sample_variable_font())
            .unwrap()
    }

    #[test]
    fn base128_round_trips_the_specified_examples() {
        let mut out = Vec::new();
        push_base128(&mut out, 63);
        assert_eq!(out, [0x3f]);

        out.clear();
        push_base128(&mut out, 0x1234);
        assert_eq!(out, [0xa4, 0x34]);

        out.clear();
        push_base128(&mut out, 0);
        assert_eq!(out, [0]);
    }

    #[test]
    fn woff_tables_decompress_to_the_source_tables() {
        let sfnt = sample_sfnt();
        let woff = to_woff(&sfnt).unwrap();

        assert_eq!(&woff[0..4], b"wOFF");
        assert_eq!(u32_at(&woff, 8) as usize, woff.len());
        assert!(woff.len().is_multiple_of(4));

        let source = Source::read(&sfnt).unwrap();
        let count = u16_at(&woff, 12) as usize;
        assert_eq!(count, source.tables.len());
        assert_eq!(u32_at(&woff, 16), source.sfnt_size());

        for (index, table) in source.tables.iter().enumerate() {
            let entry = 44 + 20 * index;
            assert_eq!(&woff[entry..entry + 4], &table.tag);
            let offset = u32_at(&woff, entry + 4) as usize;
            let compressed = u32_at(&woff, entry + 8) as usize;
            let original = u32_at(&woff, entry + 12) as usize;
            assert_eq!(original, table.data.len());
            assert_eq!(u32_at(&woff, entry + 16), table.checksum);

            let stored = &woff[offset..offset + compressed];
            let restored = if compressed < original {
                let mut restored = Vec::new();
                ZlibDecoder::new(stored).read_to_end(&mut restored).unwrap();
                restored
            } else {
                stored.to_vec()
            };
            assert_eq!(restored, table.data, "table {}", tag_text(table.tag));
        }
    }

    #[test]
    fn woff2_stream_decompresses_to_the_source_tables_in_directory_order() {
        let sfnt = sample_sfnt();
        let woff2 = to_woff2(&sfnt).unwrap();

        assert_eq!(&woff2[0..4], b"wOF2");
        assert_eq!(u32_at(&woff2, 8) as usize, woff2.len());
        assert!(woff2.len().is_multiple_of(4));

        let source = Source::read(&sfnt).unwrap();
        let count = u16_at(&woff2, 12) as usize;
        assert_eq!(count, source.tables.len());

        // Walks the directory the way a decoder would
        let mut cursor = 48;
        let mut tags = Vec::new();
        let mut lengths = Vec::new();
        for _ in 0..count {
            let flags = woff2[cursor];
            cursor += 1;
            let tag = if flags & 0x3f == USER_TAG {
                let tag: [u8; 4] = woff2[cursor..cursor + 4].try_into().unwrap();
                cursor += 4;
                tag
            } else {
                *KNOWN_TAGS[(flags & 0x3f) as usize]
            };
            if tag == *b"glyf" || tag == *b"loca" {
                assert_eq!(flags >> 6, 3, "{} uses the null transform", tag_text(tag));
            }
            let mut length = 0u32;
            loop {
                let byte = woff2[cursor];
                cursor += 1;
                length = (length << 7) | u32::from(byte & 0x7f);
                if byte & 0x80 == 0 {
                    break;
                }
            }
            tags.push(tag);
            lengths.push(length as usize);
        }

        let compressed = u32_at(&woff2, 20) as usize;
        let mut restored = Vec::new();
        brotli::Decompressor::new(&woff2[cursor..cursor + compressed], 4096)
            .read_to_end(&mut restored)
            .unwrap();

        let mut start = 0;
        for (tag, length) in tags.iter().zip(&lengths) {
            let original = source
                .tables
                .iter()
                .find(|table| table.tag == *tag)
                .unwrap();
            assert_eq!(&restored[start..start + length], original.data);
            start += length;
        }
        assert_eq!(start, restored.len());
    }

    #[test]
    fn woff2_places_loca_directly_after_glyf() {
        let woff2 = to_woff2(&sample_sfnt()).unwrap();
        let mut cursor = 48;
        let mut tags = Vec::new();
        for _ in 0..u16_at(&woff2, 12) {
            let flags = woff2[cursor];
            cursor += 1;
            if flags & 0x3f == USER_TAG {
                cursor += 4;
            }
            tags.push(KNOWN_TAGS.get((flags & 0x3f) as usize).map(|tag| **tag));
            while woff2[cursor] & 0x80 != 0 {
                cursor += 1;
            }
            cursor += 1;
        }

        let glyf = tags.iter().position(|tag| *tag == Some(*b"glyf")).unwrap();
        assert_eq!(tags[glyf + 1], Some(*b"loca"));
    }
}
