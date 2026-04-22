use anyhow::Context;
use clap::{Parser, Subcommand};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
};
use toml_edit::{value, Array, DocumentMut, Item, Table, Value};

mod preprocessor;
use preprocessor::SkillTreePreprocessor;

struct AdditionalFile {
    name: &'static str,
    bytes: &'static [u8],
    ty: &'static str,
}

// NB: Ordering matters here!
const ADDITIONAL_FILES: &[AdditionalFile] = &[
    AdditionalFile {
        name: "skill-tree.css",
        bytes: include_bytes!("../js/skill-tree.css"),
        ty: "css",
    },
    AdditionalFile {
        name: "viz.js",
        bytes: include_bytes!("../js/viz.js"),
        ty: "js",
    },
    AdditionalFile {
        name: "full.render.js",
        bytes: include_bytes!("../js/full.render.js"),
        ty: "js",
    },
    AdditionalFile {
        name: "panzoom.min.js",
        bytes: include_bytes!("../js/panzoom.min.js"),
        ty: "js",
    },
    AdditionalFile {
        name: "skill-tree.js",
        bytes: include_bytes!("../js/skill-tree.js"),
        ty: "js",
    },
];

/// mdbook preprocessor to add skill-tree support
#[derive(Parser)]
#[command(name = "mdbook-skill-tree", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Check whether a renderer is supported by this preprocessor
    Supports { renderer: String },
    /// Install the required asset files and include them in the config
    Install {
        /// Root directory for the book, should contain the configuration file (`book.toml`)
        #[arg(default_value = ".")]
        dir: String,
    },
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    match cli.command {
        Some(Command::Supports { renderer }) => handle_supports(&renderer),
        Some(Command::Install { dir }) => handle_install(&dir)?,
        None => {
            if let Err(e) = handle_preprocessing() {
                eprintln!("{}", e);
                process::exit(1);
            }
        }
    }

    Ok(())
}

fn handle_preprocessing() -> Result<(), Error> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    if ctx.mdbook_version != mdbook::MDBOOK_VERSION {
        eprintln!(
            "Warning: The mdbook-skill-tree preprocessor was built against version \
             {} of mdbook, but we're being called from version {}",
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = SkillTreePreprocessor.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}

fn handle_supports(renderer: &str) -> ! {
    let supported = SkillTreePreprocessor.supports_renderer(renderer);

    // Signal whether the renderer is supported by exiting with 1 or 0.
    if supported {
        process::exit(0);
    } else {
        process::exit(1);
    }
}

fn handle_install(dir: &str) -> anyhow::Result<()> {
    let proj_dir = PathBuf::from(dir);
    let config = proj_dir.join("book.toml");

    if !config.exists() {
        log::error!("Configuration file '{}' missing", config.display());
        process::exit(1);
    }

    log::info!("Reading configuration file {}", config.display());
    let toml = fs::read_to_string(&config).expect("can't read configuration file");
    let mut doc = toml
        .parse::<DocumentMut>()
        .expect("configuration is not valid TOML");

    let has_pre = has_preprocessor(&mut doc);
    if !has_pre {
        log::info!("Adding preprocessor configuration");
        add_preprocessor(&mut doc);
    }

    let added_additional_files = add_additional_files(&mut doc);
    if !has_pre || added_additional_files {
        log::info!("Saving changed configuration to {}", config.display());
        let toml = doc.to_string();
        let mut file = File::create(config).expect("can't open configuration file for writing.");
        file.write_all(toml.as_bytes())
            .expect("can't write configuration");
    }

    let mut printed = false;

    // Copy into it the content from viz-js folder
    for file in ADDITIONAL_FILES {
        let output_path = proj_dir.join(file.name);
        if output_path.exists() {
            log::debug!(
                "'{}' already exists (Path: {}). Skipping.",
                file.name,
                output_path.display()
            );
            continue;
        }
        if !printed {
            printed = true;
            log::info!(
                "Writing additional files to project directory at {}",
                proj_dir.display()
            );
        }
        log::debug!(
            "Writing content for '{}' into {}",
            file.name,
            output_path.display()
        );
        write_static_file(&output_path, file.bytes)
            .with_context(|| format!("creating static file `{}`", output_path.display()))?;
    }

    log::info!(
        "Files & configuration for mdbook-skill-tree are installed. \
         You can start using it in your book."
    );
    Ok(())
}

fn write_static_file(output_path: &Path, file_text: &[u8]) -> anyhow::Result<()> {
    let mut file = File::create(output_path)?;
    file.write_all(file_text)?;
    Ok(())
}

fn add_additional_files(doc: &mut DocumentMut) -> bool {
    let mut changed = false;
    let mut printed = false;

    for file in ADDITIONAL_FILES {
        let additional = additional(doc, file.ty);
        if has_file(&additional, file.name) {
            log::debug!(
                "'{}' already in 'additional-{}'. Skipping",
                file.name,
                file.ty
            )
        } else {
            if !printed {
                printed = true;
                log::info!("Adding additional files to configuration");
            }
            log::debug!("Adding '{}' to 'additional-{}'", file.name, file.ty);
            insert_additional(doc, file.ty, file.name);
            changed = true;
        }
    }

    changed
}

/// Returns a mutable reference to the `additional-{type}` array under `[output.html]`
/// if it already exists, or `None` otherwise.
fn additional<'a>(doc: &'a mut DocumentMut, additional_type: &str) -> Option<&'a mut Array> {
    doc.as_table_mut()
        .get_mut("output")?
        .as_table_mut()?
        .get_mut("html")?
        .as_table_mut()?
        .get_mut(&format!("additional-{}", additional_type))?
        .as_value_mut()?
        .as_array_mut()
}

fn has_preprocessor(doc: &mut DocumentMut) -> bool {
    matches!(doc["preprocessor"]["skill-tree"], Item::Table(_))
}

fn empty_implicit_table() -> Item {
    let mut empty_table = Table::default();
    empty_table.set_implicit(true);
    Item::Table(empty_table)
}

fn add_preprocessor(doc: &mut DocumentMut) {
    let doc = doc.as_table_mut();

    let item = doc.entry("preprocessor").or_insert(empty_implicit_table());
    let item = item
        .as_table_mut()
        .unwrap()
        .entry("skill-tree")
        .or_insert(empty_implicit_table());
    item["command"] = value("mdbook-skill-tree");
}

fn has_file(elem: &Option<&mut Array>, file: &str) -> bool {
    match elem {
        Some(elem) => elem.iter().any(|elem| match elem.as_str() {
            None => true,
            Some(s) => s.ends_with(file),
        }),
        None => false,
    }
}

fn insert_additional(doc: &mut DocumentMut, additional_type: &str, file: &str) {
    let doc = doc.as_table_mut();

    let empty_table = Item::Table(Table::default());
    let empty_array = Item::Value(Value::Array(Array::default()));
    let item = doc.entry("output").or_insert(empty_table.clone());
    let item = item
        .as_table_mut()
        .unwrap()
        .entry("html")
        .or_insert(empty_table.clone());
    let array = item
        .as_table_mut()
        .unwrap()
        .entry(&format!("additional-{}", additional_type))
        .or_insert(empty_array);
    array
        .as_value_mut()
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(file);
}
