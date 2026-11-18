use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, Result};

use swimflow::build::LoadOptions;
use swimflow::cli::Cli;
use swimflow::core::overlay::Overlay;
use swimflow::plugin::{self, DumpKind, RenderContext, RenderMode};
use swimflow::render::{Format, RenderOptions};

fn main() {
    if let Err(err) = run() {
        eprintln!("swimflow: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse_args();

    if cli.serve {
        let config = swimflow::server::ServeConfig::resolve(cli.env_file.as_deref().map(Path::new))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        return swimflow::server::run(config).map_err(|e| anyhow::anyhow!("{e}"));
    }

    let registry = plugin::Registry::builtin();

    if cli.list_types {
        print_plugins(&registry);
        return Ok(());
    }

    // The source is read once and handed to the plugin; the plugin decides
    // what to do with it.
    let source = read_source(cli.input.as_deref())?;
    let path = cli
        .input
        .as_deref()
        .filter(|input| *input != "-")
        .map(Path::new);

    let request = plugin::PluginRequest {
        type_name: cli.diagram_type.as_deref(),
        path,
        source: &source,
    };
    let diagram_plugin = registry
        .resolve(&request)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let font = swimflow::text::FontSpec::resolve(
        cli.font.as_deref().map(Path::new),
        cli.font_family.as_deref(),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut options = RenderOptions {
        embed_font: cli.embed_font,
        text_as_paths: cli.text_as_paths,
        nodoctype: cli.nodoctype,
        noviewbox: cli.noviewbox,
        transparent: cli.transparent,
        scale: if cli.antialias { 2.0 } else { cli.scale },
        bundle_root: cli.root.clone(),
        no_external: cli.no_external,
        font,
        view: Default::default(),
    };

    let load_options = LoadOptions {
        allow_broken_links: cli.allow_broken_links,
        follow_external_files: !cli.no_external_files,
    };
    // The command line has no in-memory files; the live preview does.
    let overlay = Overlay::new();

    if cli.dump_ast || cli.dump_model || cli.dump_geometry {
        let kind = if cli.dump_ast {
            DumpKind::Ast
        } else if cli.dump_model {
            DumpKind::Model
        } else {
            DumpKind::Geometry
        };
        let context = RenderContext {
            source: &source,
            path,
            options: &options,
            mode: RenderMode::Standalone,
            load: &load_options,
            pixels: cli.pixels,
            overlay: &overlay,
        };
        let dump = diagram_plugin
            .dump(kind, &context)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        print!("{dump}");
        return Ok(());
    }

    let format = if cli.bundle.is_some() {
        Format::Html
    } else {
        resolve_format(&cli)?
    };

    // PNG and PDF load the font into their own font database, so the
    // intermediate SVG must not embed it as well.
    if matches!(format, Format::Png | Format::Pdf) {
        options.embed_font = false;
    }

    let mode = if format == Format::Html {
        RenderMode::Bundle
    } else {
        RenderMode::Standalone
    };

    let context = RenderContext {
        source: &source,
        path,
        options: &options,
        mode,
        load: &load_options,
        pixels: false,
        overlay: &overlay,
    };
    let rendered = diagram_plugin
        .render(&context)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    for warning in &rendered.warnings {
        eprintln!("swimflow: warning: {warning}");
    }

    let output = resolve_output(&cli, format);
    let bytes = plugin::render_output(format, &rendered, &options)?;
    write_output(output.as_deref(), &bytes)?;

    Ok(())
}

/// Print the available diagram types.
fn print_plugins(registry: &plugin::Registry) {
    for plugin in registry.plugins() {
        let extensions: Vec<String> = plugin
            .extensions()
            .iter()
            .map(|extension| format!(".{extension}"))
            .collect();
        println!(
            "{:<10} {}  [{}]",
            plugin.name(),
            plugin.description(),
            extensions.join(", ")
        );
    }
}

/// Write the rendered bytes to the destination.
fn write_output(output: Option<&str>, bytes: &[u8]) -> Result<()> {
    match output {
        None | Some("-") => {
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            lock.write_all(bytes).context("failed to write output")?;
            lock.flush().context("failed to write output")?;
        }
        Some(path) => {
            let file =
                std::fs::File::create(path).with_context(|| format!("failed to create {path}"))?;
            let mut writer = std::io::BufWriter::new(file);
            writer.write_all(bytes).context("failed to write output")?;
            writer.flush().context("failed to write output")?;
        }
    }
    Ok(())
}

fn resolve_format(cli: &Cli) -> Result<Format> {
    if let Some(name) = &cli.format {
        return Format::from_name(name)
            .ok_or_else(|| anyhow::anyhow!("unknown output format: {name}"));
    }
    if let Some(path) = cli.output.as_deref().filter(|path| *path != "-")
        && let Some(extension) = Path::new(path).extension().and_then(|e| e.to_str())
        && let Some(format) = Format::from_extension(extension)
    {
        return Ok(format);
    }
    Ok(Format::Svg)
}

fn resolve_output(cli: &Cli, format: Format) -> Option<String> {
    if let Some(bundle) = &cli.bundle {
        return Some(bundle.clone());
    }
    if let Some(output) = &cli.output {
        return Some(output.clone());
    }
    match cli.input.as_deref() {
        Some(path) if path != "-" => Some(
            Path::new(path)
                .with_extension(format.as_str())
                .to_string_lossy()
                .into_owned(),
        ),
        _ => None,
    }
}

fn read_source(input: Option<&str>) -> Result<String> {
    match input {
        None | Some("-") => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("failed to read stdin")?;
            Ok(buf)
        }
        Some(path) => {
            std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))
        }
    }
}
