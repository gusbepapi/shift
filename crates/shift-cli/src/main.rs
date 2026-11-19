mod authoring;
mod check;
mod cli;
mod format;
mod glyph_inspect;
mod import;
mod inspect;
mod kerning;
mod pack;
mod proof;
mod selector;
mod skill;

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use authoring::{
    AuthoringReport, ExportSources, add_axis, add_glyph, add_glyph_axis, add_instance, add_layer,
    add_page, add_source, assign_kerning_group, copy_layer, create_font, create_kerning_group,
    delete_kerning_group, edit_glyph_axis, export_glyphs, font_info, list_pages, move_layer,
    remove_color_glyph, remove_glyph_axis, remove_instance, remove_kerning, remove_layer,
    remove_page, rename_kerning_group, rename_page, set_axis, set_color_glyph, set_font,
    set_glyphs, set_instance, set_kerning, set_layer, set_mark_color, set_palettes,
    unassign_kerning_group,
};
use authoring::{
    add_alternate, move_alternate, remove_alternate, set_alternate, set_component_condition,
};
use clap::Parser;
use cli::{
    AxisCommand, Cli, ColorCommand, ColorGlyphCommand, Command, CompileArgs, ExportArgs,
    FontCommand, GlyphAxisCommand, GlyphCommand, InstanceCommand, KerningCommand,
    KerningGroupCommand, LayerCommand, PageCommand, PaletteCommand, SkillCommand, SourceCommand,
};
use cli::{ComponentCommand, ComponentConditionCommand, GlyphAlternateCommand};
use glyph_inspect::GlyphInspection;
use inspect::{InspectReport, RenderMode};
use kerning::{get_kerning, kerning_groups, list_kerning};
use miette::IntoDiagnostic;
use serde::Serialize;
use shift_backends::{
    BinaryEncoding, BinaryEncodings, ColorTable, ColorTables, ExportFormat, FontExportRequest,
    FontExportResult, FontExporter, VariableComponents,
};
use shift_store::ShiftStore;
use skill::{install_skill, show_skill, skill_status};

fn main() -> miette::Result<()> {
    match Cli::parse().command {
        Command::Inspect(args) => inspect(&args),
        Command::Check(args) => {
            let report = check::check(&args)?;
            write_report(&report, args.json)?;
            if report.has_errors() {
                std::process::exit(1);
            }
            Ok(())
        }
        Command::Compile(args) => {
            for file in compile(args)?.iter().flat_map(|result| &result.files) {
                write_stdout(&file.display().to_string())?;
            }
            Ok(())
        }
        Command::Export(args) => {
            let result = export(args)?;
            write_stdout(&result.path.display().to_string())
        }
        Command::Import(args) => write_report(&import::import(&args)?, args.json),
        Command::Proof(args) => {
            let summary = proof::proof(&args)?;
            for warning in &summary.warnings {
                write_stderr(&format!("warning: {}", warning.message()))?;
            }
            write_report(&summary, args.json)
        }
        Command::Font { command } => match command {
            FontCommand::Create(args) => {
                let json = args.json;
                write_authoring_result(create_font(args), json)
            }
            FontCommand::Info(args) => {
                let json = args.json;
                write_report(&font_info(args)?, json)
            }
            FontCommand::Set(args) => {
                let json = args.mutation.json;
                write_authoring_result(set_font(&args), json)
            }
            FontCommand::Pack(args) => {
                pack::pack_font(&args)?;
                write_stdout(&args.output.display().to_string())
            }
        },
        Command::Axis { command } => match command {
            AxisCommand::Add(args) => {
                let json = args.mutation.json;
                write_authoring_result(add_axis(args), json)
            }
            AxisCommand::Set(args) => {
                let json = args.mutation.json;
                write_authoring_result(set_axis(args), json)
            }
        },
        Command::Instance { command } => match command {
            InstanceCommand::Add(args) => {
                let json = args.mutation.json;
                write_authoring_result(add_instance(args), json)
            }
            InstanceCommand::Set(args) => {
                let json = args.mutation.json;
                write_authoring_result(set_instance(args), json)
            }
            InstanceCommand::Remove(args) => {
                let json = args.mutation.json;
                write_authoring_result(remove_instance(&args), json)
            }
        },
        Command::Page { command } => match command {
            PageCommand::List(args) => {
                let json = args.json;
                write_report(&list_pages(&args)?, json)
            }
            PageCommand::Add(args) => {
                let json = args.mutation.json;
                write_authoring_result(add_page(&args), json)
            }
            PageCommand::Rename(args) => {
                let json = args.mutation.json;
                write_authoring_result(rename_page(&args), json)
            }
            PageCommand::Remove(args) => {
                let json = args.mutation.json;
                write_authoring_result(remove_page(&args), json)
            }
        },
        Command::Color { command } => color_command(command),
        Command::Source { command } => match command {
            SourceCommand::Add(args) => {
                let json = args.mutation.json;
                write_authoring_result(add_source(args), json)
            }
        },
        Command::Glyph { command } => glyph_command(command),
        Command::Component {
            command:
                ComponentCommand::Condition {
                    command: ComponentConditionCommand::Set(args),
                },
        } => {
            let json = args.mutation.json;
            write_authoring_result(set_component_condition(&args), json)
        }
        Command::Layer { command } => match command {
            LayerCommand::Add(args) => {
                let json = args.mutation.json;
                write_authoring_result(add_layer(&args), json)
            }
            LayerCommand::Set(args) => {
                let json = args.mutation.json;
                write_authoring_result(set_layer(&args), json)
            }
            LayerCommand::Copy(args) => {
                let json = args.mutation.json;
                write_authoring_result(copy_layer(&args), json)
            }
            LayerCommand::Move(args) => {
                let json = args.mutation.json;
                write_authoring_result(move_layer(&args), json)
            }
            LayerCommand::Remove(args) => {
                let json = args.mutation.json;
                write_authoring_result(remove_layer(&args), json)
            }
        },
        Command::Kerning { command } => kerning_command(command),
        Command::Skill { command } => match command {
            SkillCommand::Show(args) => write_stdout(show_skill(&args)?.trim_end()),
            SkillCommand::Install(args) => write_report(&install_skill(&args)?, args.json),
            SkillCommand::Status(args) => write_report(&skill_status(&args)?, args.json),
        },
    }
}

fn color_command(command: ColorCommand) -> miette::Result<()> {
    match command {
        ColorCommand::Palette { command } => match command {
            PaletteCommand::Set(args) => {
                let json = args.mutation.json;
                write_authoring_result(set_palettes(&args), json)
            }
        },
        ColorCommand::Glyph { command } => match command {
            ColorGlyphCommand::Set(args) => {
                let json = args.mutation.json;
                write_authoring_result(set_color_glyph(&args), json)
            }
            ColorGlyphCommand::Remove(args) => {
                let json = args.mutation.json;
                write_authoring_result(remove_color_glyph(&args), json)
            }
        },
    }
}

fn glyph_command(command: GlyphCommand) -> miette::Result<()> {
    match command {
        GlyphCommand::Add(args) => {
            let json = args.mutation.json;
            write_authoring_result(add_glyph(args), json)
        }
        GlyphCommand::Set(args) => {
            let json = args.mutation.json;
            write_authoring_result(set_glyphs(&args), json)
        }
        GlyphCommand::Inspect(args) => inspect_glyph(&args),
        GlyphCommand::Export(args) => export_glyph_batch(args),
        GlyphCommand::Color(args) => {
            let json = args.mutation.json;
            write_authoring_result(set_mark_color(&args), json)
        }
        GlyphCommand::Axis { command } => match command {
            GlyphAxisCommand::Add(args) => {
                let json = args.mutation.json;
                write_authoring_result(add_glyph_axis(args), json)
            }
            GlyphAxisCommand::Edit(args) => {
                let json = args.mutation.json;
                write_authoring_result(edit_glyph_axis(args), json)
            }
            GlyphAxisCommand::Remove(args) => {
                let json = args.mutation.json;
                write_authoring_result(remove_glyph_axis(&args), json)
            }
        },
        GlyphCommand::Alternate { command } => match command {
            GlyphAlternateCommand::Add(args) => {
                let json = args.mutation.json;
                write_authoring_result(add_alternate(&args), json)
            }
            GlyphAlternateCommand::Remove(args) => {
                let json = args.mutation.json;
                write_authoring_result(remove_alternate(&args), json)
            }
            GlyphAlternateCommand::Reorder(args) => {
                let json = args.mutation.json;
                write_authoring_result(move_alternate(&args), json)
            }
            GlyphAlternateCommand::Set(args) => {
                let json = args.mutation.json;
                write_authoring_result(set_alternate(&args), json)
            }
        },
    }
}

fn kerning_command(command: KerningCommand) -> miette::Result<()> {
    match command {
        KerningCommand::List(args) => {
            let json = args.json;
            write_report(&list_kerning(args)?, json)
        }
        KerningCommand::Groups(args) => {
            let json = args.json;
            write_report(&kerning_groups(args)?, json)
        }
        KerningCommand::Get(args) => {
            let json = args.json;
            write_report(&get_kerning(args)?, json)
        }
        KerningCommand::Set(args) => {
            let json = args.mutation.json;
            write_authoring_result(set_kerning(&args), json)
        }
        KerningCommand::Remove(args) => {
            let json = args.mutation.json;
            write_authoring_result(remove_kerning(&args), json)
        }
        KerningCommand::Group { command } => match command {
            KerningGroupCommand::Create(args) => {
                let json = args.mutation.json;
                write_authoring_result(create_kerning_group(args), json)
            }
            KerningGroupCommand::Rename(args) => {
                let json = args.mutation.json;
                write_authoring_result(rename_kerning_group(args), json)
            }
            KerningGroupCommand::Delete(args) => {
                let json = args.mutation.json;
                write_authoring_result(delete_kerning_group(&args), json)
            }
            KerningGroupCommand::Assign(args) => {
                let json = args.mutation.json;
                write_authoring_result(assign_kerning_group(&args), json)
            }
            KerningGroupCommand::Unassign(args) => {
                let json = args.mutation.json;
                write_authoring_result(unassign_kerning_group(&args), json)
            }
        },
    }
}

/// A command's result, printed as text or, with `--json`, as JSON.
trait Report: Serialize {
    /// The report as text for a terminal.
    fn render(&self) -> String;
}

fn write_report(report: &impl Report, json: bool) -> miette::Result<()> {
    let output = if json {
        serde_json::to_string_pretty(report).into_diagnostic()?
    } else {
        report.render()
    };
    write_stdout(&output)
}

fn write_authoring_result(
    result: miette::Result<AuthoringReport>,
    json: bool,
) -> miette::Result<()> {
    match result {
        Ok(report) => write_report(&report, json),
        Err(error) if json => {
            let mut messages = vec![error.to_string()];
            let error_ref: &(dyn std::error::Error + Send + Sync + 'static) = error.as_ref();
            let mut source = error_ref.source();
            while let Some(cause) = source {
                messages.push(cause.to_string());
                source = cause.source();
            }
            let output = serde_json::to_string_pretty(&serde_json::json!({
                "valid": false,
                "error": {
                    "summary": messages[0],
                    "causes": &messages[1..],
                }
            }))
            .into_diagnostic()?;
            write_stdout(&output)?;
            Err(error)
        }
        Err(error) => Err(error),
    }
}

/// Compiles the document to one binary or source in the requested format, or
/// with `--static` to one static binary per named instance; with `--woff2`,
/// each TrueType binary is also written as WOFF2.
fn compile(args: CompileArgs) -> miette::Result<Vec<FontExportResult>> {
    let format = compile_format(&args);
    if args.woff2 && format != ExportFormat::Ttf {
        return Err(miette::miette!(
            "--woff2 writes beside a TrueType binary; use --format woff2 to write only WOFF2"
        ));
    }
    let color_tables = compile_color_tables(&args);
    let encodings = if args.woff2 {
        BinaryEncodings::new([BinaryEncoding::Ttf, BinaryEncoding::Woff2]).into_diagnostic()?
    } else {
        BinaryEncodings::ttf()
    };
    let font = load_document(&args.path)?;
    let variable_components = if args.varc {
        VariableComponents::Varc
    } else {
        VariableComponents::Decompose
    };
    let exporter = FontExporter::new().with_variable_components(variable_components);
    if args.static_instances {
        return exporter
            .export_static_instances_with(&font, &args.output, &encodings, &color_tables)
            .into_diagnostic();
    }
    let request = FontExportRequest {
        path: args.output,
        format,
        color_tables,
    };
    Ok(vec![
        exporter
            .export_with(&font, request, &encodings)
            .into_diagnostic()?,
    ])
}

fn export(args: ExportArgs) -> miette::Result<FontExportResult> {
    let format = ExportFormat::from_path(&args.output).into_diagnostic()?;
    export_document(&args.path, args.output, format)
}

/// Writes the `.shift` document at `path` to `output` as `format`, leaving the
/// document unchanged.
fn export_document(
    path: &Path,
    output: PathBuf,
    format: ExportFormat,
) -> miette::Result<FontExportResult> {
    let font = load_document(path)?;
    FontExporter::new()
        .export(
            &font,
            FontExportRequest {
                path: output,
                format,
                color_tables: ColorTables::default(),
            },
        )
        .into_diagnostic()
}

/// Resolves the colour tables from `--color-tables`, defaulting to all of them.
fn compile_color_tables(args: &CompileArgs) -> ColorTables {
    let Some(listed) = &args.color_tables else {
        return ColorTables::default();
    };
    listed
        .iter()
        .filter_map(|table| match table {
            cli::CompileColorTable::Colr => Some(ColorTable::Colr),
            cli::CompileColorTable::Svg => Some(ColorTable::Svg),
            cli::CompileColorTable::Sbix => Some(ColorTable::Sbix),
            cli::CompileColorTable::None => None,
        })
        .collect()
}

/// Resolves the export format from `--format`, then the output extension, then TTF.
fn compile_format(args: &CompileArgs) -> ExportFormat {
    if let Some(format) = args.format {
        return match format {
            cli::CompileFormat::Ttf => ExportFormat::Ttf,
            cli::CompileFormat::Otf => ExportFormat::Otf,
            cli::CompileFormat::Woff => ExportFormat::Woff,
            cli::CompileFormat::Woff2 => ExportFormat::Woff2,
            cli::CompileFormat::Glyphs => ExportFormat::Glyphs,
        };
    }
    args.output
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| ExportFormat::try_from(extension).ok())
        .filter(|format| {
            matches!(
                format,
                ExportFormat::Ttf
                    | ExportFormat::Otf
                    | ExportFormat::Woff
                    | ExportFormat::Woff2
                    | ExportFormat::Glyphs
            )
        })
        .unwrap_or(ExportFormat::Ttf)
}

fn inspect(args: &cli::InspectArgs) -> miette::Result<()> {
    let report = InspectReport::load(&args.path)?;
    let output = if args.json {
        serde_json::to_string_pretty(&report).into_diagnostic()?
    } else {
        report.render(args.view, render_mode())
    };

    write_stdout(&output)
}

fn export_glyph_batch(args: cli::ExportGlyphsArgs) -> miette::Result<()> {
    let sources = match args.source.as_deref() {
        None => ExportSources::Default,
        Some("all") => ExportSources::All,
        Some(name) => ExportSources::Named(name.to_string()),
    };
    let batch = export_glyphs(&args.path, &args.glyphs, &sources)?;
    let json = serde_json::to_string_pretty(&batch).into_diagnostic()?;
    match args.output {
        Some(path) => std::fs::write(&path, format!("{json}\n"))
            .into_diagnostic()
            .map_err(|error| error.wrap_err(format!("failed to write {}", path.display()))),
        None => {
            let mut stdout = io::stdout().lock();
            writeln!(stdout, "{json}").into_diagnostic()
        }
    }
}

fn inspect_glyph(args: &cli::InspectGlyphArgs) -> miette::Result<()> {
    let report = GlyphInspection::load(&args.path, &args.glyph, &args.location)?;
    let output = if args.json {
        serde_json::to_string_pretty(&report).into_diagnostic()?
    } else {
        report.render(args.view, render_mode())
    };

    write_stdout(&output)
}

fn render_mode() -> RenderMode {
    if io::stdout().is_terminal() {
        RenderMode::Styled
    } else {
        RenderMode::Plain
    }
}

fn load_document(path: &Path) -> miette::Result<shift_font::Font> {
    ShiftStore::open_document(path)
        .and_then(|store| store.load_font_state())
        .into_diagnostic()
}

fn write_stderr(output: &str) -> miette::Result<()> {
    write_line(io::stderr().lock(), output)
}

fn write_stdout(output: &str) -> miette::Result<()> {
    write_line(io::stdout().lock(), output)
}

/// Writes `output` and a newline to `stream`; a closed pipe is not an error.
fn write_line(mut stream: impl Write, output: &str) -> miette::Result<()> {
    match writeln!(stream, "{output}") {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error).into_diagnostic(),
    }
}

#[cfg(test)]
mod tests {
    use shift_font::test_support::sample_variable_font;

    use super::*;

    fn compile_args(path: PathBuf, output: PathBuf) -> CompileArgs {
        CompileArgs {
            path,
            output,
            format: None,
            color_tables: None,
            static_instances: false,
            woff2: false,
            varc: false,
        }
    }

    #[test]
    fn compile_command_writes_ttf_from_shift_document() {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("Dogfood.shift");
        let output_path = temp.path().join("Dogfood.ttf");
        drop(ShiftStore::create_document(&source_path, &sample_variable_font()).unwrap());

        let results = compile(compile_args(source_path, output_path.clone())).unwrap();

        assert_eq!(
            results,
            [FontExportResult {
                path: output_path.clone(),
                format: ExportFormat::Ttf,
                files: vec![output_path.clone()],
            }]
        );
        let bytes = std::fs::read(output_path).unwrap();
        assert_eq!(&bytes[..4], &[0x00, 0x01, 0x00, 0x00]);
    }

    fn args_with_color_tables(color_tables: Option<Vec<cli::CompileColorTable>>) -> CompileArgs {
        CompileArgs {
            color_tables,
            ..compile_args("Dogfood.shift".into(), "Dogfood.ttf".into())
        }
    }

    #[test]
    fn compile_writes_every_colour_table_unless_told_otherwise() {
        let tables = compile_color_tables(&args_with_color_tables(None));

        assert_eq!(tables, ColorTables::default());
    }

    #[test]
    fn compile_colour_tables_follow_the_listed_tables() {
        let listed = args_with_color_tables(Some(vec![
            cli::CompileColorTable::Colr,
            cli::CompileColorTable::Sbix,
        ]));

        let tables = compile_color_tables(&listed);

        assert!(tables.contains(ColorTable::Colr) && tables.contains(ColorTable::Sbix));
        assert!(!tables.contains(ColorTable::Svg));
    }

    #[test]
    fn compile_colour_tables_none_writes_no_colour_data() {
        let none = args_with_color_tables(Some(vec![cli::CompileColorTable::None]));

        assert!(compile_color_tables(&none).is_empty());
    }

    #[test]
    fn compile_command_infers_format_from_output_extension() {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("Dogfood.shift");
        drop(ShiftStore::create_document(&source_path, &sample_variable_font()).unwrap());

        let glyphs_path = temp.path().join("Dogfood.glyphs");
        let results = compile(compile_args(source_path.clone(), glyphs_path.clone())).unwrap();
        assert_eq!(results[0].format, ExportFormat::Glyphs);
        assert!(
            std::fs::read_to_string(glyphs_path)
                .unwrap()
                .starts_with("{")
        );

        let woff2_path = temp.path().join("Dogfood.woff2");
        let results = compile(CompileArgs {
            format: Some(cli::CompileFormat::Woff2),
            ..compile_args(source_path, woff2_path.clone())
        })
        .unwrap();
        assert_eq!(results[0].format, ExportFormat::Woff2);
        assert_eq!(&std::fs::read(woff2_path).unwrap()[..4], b"wOF2");
    }

    #[test]
    fn compile_command_refuses_woff2_beside_a_non_ttf_format() {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("Dogfood.shift");
        drop(ShiftStore::create_document(&source_path, &sample_variable_font()).unwrap());

        let result = compile(CompileArgs {
            woff2: true,
            ..compile_args(source_path, temp.path().join("Dogfood.glyphs"))
        });

        assert!(result.is_err());
    }
}
