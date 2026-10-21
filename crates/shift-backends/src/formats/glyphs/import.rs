use crate::SourceGlyphIds;
use std::{collections::HashMap, path::Path, sync::Arc};

use glyphs_reader::Font as GlyphsFont;
use rayon::prelude::*;
use shift_font::{ColorData, Font, Glyph, GlyphId};

use super::{
    color::{ColorImport, SynthesizedGlyph, plan_color},
    conversion::{
        GlyphsLayerSources, add_intermediate_sources, convert_glyph, convert_kerning, font_header,
        imported_layer_count,
    },
    families::InstanceFamilies,
    report::import_report,
};
use crate::{
    FormatBackendError, FormatBackendResult, ImportReport,
    import::{GlyphDirectoryEntry, GlyphStream, ImportBatchLimit},
};

/// Bounded Shift conversion over one parsed Glyphs source.
pub(crate) struct GlyphsGlyphStream {
    source: Arc<GlyphsFont>,
    glyph_ids: HashMap<String, GlyphId>,
    glyph_names: Vec<String>,
    /// Layer glyphs of colour glyphs that the file does not name, after `glyph_names`.
    synthesized: Vec<SynthesizedGlyph>,
    color: ColorData,
    sources: GlyphsLayerSources,
    next_glyph: usize,
}

impl GlyphsGlyphStream {
    fn len(&self) -> usize {
        self.glyph_names.len() + self.synthesized.len()
    }

    /// Returns how many layers the glyph at `index` imports.
    fn layer_count(&self, index: usize) -> usize {
        match self.glyph_names.get(index) {
            Some(name) => imported_layer_count(&self.source.glyphs[name.as_str()], &self.sources),
            None => self.synthesized[index - self.glyph_names.len()].layer_count(),
        }
    }

    fn convert(&self, index: usize) -> FormatBackendResult<Glyph> {
        match self.glyph_names.get(index) {
            Some(name) => convert_glyph(
                &self.source.glyphs[name.as_str()],
                &self.glyph_ids,
                &self.sources,
            ),
            None => self.synthesized[index - self.glyph_names.len()]
                .convert(&self.source, &self.glyph_ids),
        }
    }
}

impl GlyphStream for GlyphsGlyphStream {
    fn directory(&self) -> Vec<GlyphDirectoryEntry> {
        let named = self.glyph_names.iter().map(|name| GlyphDirectoryEntry {
            glyph_id: self.glyph_ids[name].clone(),
            name: name.clone().into(),
        });
        let synthesized = self.synthesized.iter().map(|glyph| GlyphDirectoryEntry {
            glyph_id: glyph.id.clone(),
            name: glyph.name.clone().into(),
        });
        named.chain(synthesized).collect()
    }

    fn glyph_count(&self) -> usize {
        self.len()
    }

    fn color(&self) -> ColorData {
        self.color.clone()
    }

    fn next_batch(&mut self, limit: ImportBatchLimit) -> FormatBackendResult<Vec<Glyph>> {
        if self.next_glyph == self.len() {
            return Ok(Vec::new());
        }

        let mut end = self.next_glyph;
        let mut layer_count = 0;
        while end < self.len() && end - self.next_glyph < limit.max_glyphs() {
            let next_layers = self.layer_count(end);
            if end > self.next_glyph && layer_count + next_layers > limit.max_layers() {
                break;
            }

            layer_count += next_layers;
            end += 1;
        }

        let glyphs = (self.next_glyph..end)
            .into_par_iter()
            .map(|index| self.convert(index))
            .collect::<FormatBackendResult<Vec<_>>>()?;
        self.next_glyph = end;
        Ok(glyphs)
    }
}

pub(crate) fn stream_font(
    path: &str,
) -> FormatBackendResult<(Font, GlyphsGlyphStream, ImportReport)> {
    let source = Arc::new(
        GlyphsFont::load(Path::new(path))
            .map_err(|error| FormatBackendError::Glyphs(error.to_string()))?,
    );
    stream_retained(Path::new(path), source)
}

pub(crate) fn stream_retained(
    path: &Path,
    source: Arc<GlyphsFont>,
) -> FormatBackendResult<(Font, GlyphsGlyphStream, ImportReport)> {
    let mut report = import_report(&source);
    let families = InstanceFamilies::read(path, &source);
    report.losses.extend(families.loss());
    let (mut header, source_ids_by_master_id) =
        font_header(&source, path, &families, &mut report.losses)?;
    let glyph_names = source
        .glyphs
        .values()
        .map(|glyph| glyph.name.to_string())
        .collect::<Vec<_>>();
    let source_glyph_ids = SourceGlyphIds::for_path(path);
    let mut glyph_ids = glyph_names
        .iter()
        .map(|name| (name.clone(), source_glyph_ids.glyph_id(name)))
        .collect::<HashMap<_, _>>();
    *header.kerning_mut() =
        convert_kerning(&source, &glyph_ids, &source_ids_by_master_id, &mut report);
    let sources = add_intermediate_sources(&mut header, &source, source_ids_by_master_id);
    let ColorImport {
        data: color,
        glyphs: synthesized,
    } = plan_color(
        &source,
        &source_glyph_ids,
        &sources,
        &mut glyph_ids,
        &mut report.losses,
    )?;

    Ok((
        header,
        GlyphsGlyphStream {
            source,
            glyph_ids,
            glyph_names,
            synthesized,
            color,
            sources,
            next_glyph: 0,
        },
        report,
    ))
}
