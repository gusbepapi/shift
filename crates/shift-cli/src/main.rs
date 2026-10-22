mod authoring;
mod check;
mod cli;
mod format;
mod glyph_inspect;
mod inspect;
mod kerning;
mod proof;
mod selector;
mod skill;

use std::io::{self, IsTerminal, Write};

use authoring::{
    AuthoringReport, ExportSources, add_axis, add_glyph, add_instance, add_layer, add_source,
    assign_kerning_group, copy_layer, create_font, create_kerning_group, delete_kerning_group,
    export_glyphs, font_info, remove_color_glyph, remove_instance, remove_kerning,
    rename_kerning_group, set_axis, set_color_glyph, set_font, set_glyphs, set_instance,
    set_kerning, set_layer, set_palettes, unassign_kerning_group,
};
use clap::Parser;
use cli::{
    AxisCommand, Cli, ColorCommand, ColorGlyphCommand, Command, CompileArgs, FontCommand,
    GlyphCommand, InstanceCommand, KerningCommand, KerningGroupCommand, LayerCommand,
    PaletteCommand, SkillCommand, SourceCommand,
};
use glyph_inspect::GlyphInspection;
use inspect::{InspectReport, RenderMode};
use kerning::{get_kerning, kerning_groups, list_kerning};
use miette::IntoDiagnostic;
use serde::Serialize;
use shift_backends::{
    ColorTable, ColorTables, ExportFormat, FontExportRequest, FontExportResult, FontExporter,
};
use shift_store::ShiftStore;
use skill::{SkillReport, install_skill, show_skill, skill_status};

fn main() -> miette::Result<()> {
    match Cli::parse().command {
        Command::Inspect(args) => inspect(&args),
        Command::Check(args) => {
            let report = check::check(&args)?;
            write_report(&report, args.json, check::CheckReport::render)?;
            if report.has_errors() {
                std::process::exit(1);
            }
            Ok(())
        }
        Command::Compile(args) => {
            let result = compile(args)?;
            write_stdout(&result.path.display().to_string())
        }
        Command::Proof(args) => {
            let summary = proof::proof(&args)?;
            write_report(&summary, args.json, proof::ProofSummary::render)
        }
        Command::Font { command } => match command {
            FontCommand::Create(args) => {
                let json = args.json;
                write_authoring_result(create_font(args), json)
            }
            FontCommand::Info(args) => {
                let json = args.json;
                let report = font_info(args)?;
                let output = if json {
                    serde_json::to_string_pretty(&report).into_diagnostic()?
                } else {
                    report.render()
                };
                write_stdout(&output)
            }
            FontCommand::Set(args) => {
                let json = args.mutation.json;
                write_authoring_result(set_font(&args), json)
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
        Command::Color { command } => match command {
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
        },
        Command::Source { command } => match command {
            SourceCommand::Add(args) => {
                let json = args.mutation.json;
                write_authoring_result(add_source(args), json)
            }
        },
        Command::Glyph { command } => match command {
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
        },
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
        },
        Command::Kerning { command } => kerning_command(command),
        Command::Skill { command } => match command {
            SkillCommand::Show(args) => write_stdout(show_skill(&args)?.trim_end()),
            SkillCommand::Install(args) => write_skill_report(&install_skill(&args)?, args.json),
            SkillCommand::Status(args) => write_skill_report(&skill_status(&args)?, args.json),
        },
    }
}

#[allow(
    clippy::redundant_closure_for_method_calls,
    reason = "each report type is private to its module"
)]
fn kerning_command(command: KerningCommand) -> miette::Result<()> {
    match command {
        KerningCommand::List(args) => {
            let json = args.json;
            write_report(&list_kerning(args)?, json, |report| report.render())
        }
        KerningCommand::Groups(args) => {
            let json = args.json;
            write_report(&kerning_groups(args)?, json, |report| report.render())
        }
        KerningCommand::Get(args) => {
            let json = args.json;
            write_report(&get_kerning(args)?, json, |report| report.render())
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

fn write_report<T: Serialize>(
    report: &T,
    json: bool,
    render: impl FnOnce(&T) -> String,
) -> miette::Result<()> {
    let output = if json {
        serde_json::to_string_pretty(report).into_diagnostic()?
    } else {
        render(report)
    };
    write_stdout(&output)
}

fn write_authoring_result(
    result: miette::Result<AuthoringReport>,
    json: bool,
) -> miette::Result<()> {
    match result {
        Ok(report) => write_authoring_report(&report, json),
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

fn write_authoring_report(report: &AuthoringReport, json: bool) -> miette::Result<()> {
    let output = if json {
        serde_json::to_string_pretty(&report).into_diagnostic()?
    } else {
        report.render()
    };
    write_stdout(&output)
}

fn compile(args: CompileArgs) -> miette::Result<FontExportResult> {
    let font = ShiftStore::open_document(&args.path)
        .and_then(|store| store.load_font_state())
        .into_diagnostic()?;
    let format = compile_format(&args);
    let color_tables = compile_color_tables(&args);
    FontExporter::new()
        .export(
            &font,
            FontExportRequest {
                path: args.output,
                format,
                color_tables,
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

fn write_skill_report(report: &SkillReport, json: bool) -> miette::Result<()> {
    let output = if json {
        serde_json::to_string_pretty(report).into_diagnostic()?
    } else {
        report.render()
    };
    write_stdout(&output)
}

fn write_stdout(output: &str) -> miette::Result<()> {
    let mut stdout = io::stdout().lock();
    match writeln!(stdout, "{output}") {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error).into_diagnostic(),
    }
}

#[cfg(test)]
mod tests {
    use shift_font::test_support::sample_variable_font;

    use super::*;

    #[test]
    fn compile_command_writes_ttf_from_shift_document() {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("Dogfood.shift");
        let output_path = temp.path().join("Dogfood.ttf");
        drop(ShiftStore::create_document(&source_path, &sample_variable_font()).unwrap());

        let result = compile(CompileArgs {
            path: source_path,
            output: output_path.clone(),
            format: None,
            color_tables: None,
        })
        .unwrap();

        assert_eq!(
            result,
            FontExportResult {
                path: output_path,
                format: ExportFormat::Ttf,
            }
        );
        let bytes = std::fs::read(result.path).unwrap();
        assert_eq!(&bytes[..4], &[0x00, 0x01, 0x00, 0x00]);
    }

    fn args_with_color_tables(color_tables: Option<Vec<cli::CompileColorTable>>) -> CompileArgs {
        CompileArgs {
            path: "Dogfood.shift".into(),
            output: "Dogfood.ttf".into(),
            format: None,
            color_tables,
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
        let result = compile(CompileArgs {
            path: source_path.clone(),
            output: glyphs_path.clone(),
            format: None,
            color_tables: None,
        })
        .unwrap();
        assert_eq!(result.format, ExportFormat::Glyphs);
        assert!(
            std::fs::read_to_string(glyphs_path)
                .unwrap()
                .starts_with("{")
        );

        let woff2_path = temp.path().join("Dogfood.woff2");
        let result = compile(CompileArgs {
            path: source_path,
            output: woff2_path.clone(),
            format: Some(cli::CompileFormat::Woff2),
            color_tables: None,
        })
        .unwrap();
        assert_eq!(result.format, ExportFormat::Woff2);
        assert_eq!(&std::fs::read(woff2_path).unwrap()[..4], b"wOF2");
    }
}
