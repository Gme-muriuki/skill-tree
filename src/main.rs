//! skill-tree - dependency-aware roadmap tool
//!
//! Reads a TOML skill tree definition and produces Graphviz DOT output,
//! or validates the tree for errors.
//!

use std::{fs::File, path::PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};
use skill_tree::SkillTree;

#[derive(Parser, Debug)]
#[command(name = "skill-tree", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Render a skill tree to a Graphviz DOT file
    Render(RenderArgs),

    /// Validate a skill tree file and report any errors
    ///
    /// Exits with code 0 if the tree is valid, 1 if there are errors.
    /// Suitable for use in CI pipelines.
    Validate(ValidateArgs),
}

#[derive(Parser, Debug)]
struct RenderArgs {
    /// Path to the skill tree TOML file
    skill_tree: PathBuf,

    /// Output for the generated dot file.
    output_path: PathBuf,
}

#[derive(Parser, Debug)]
struct ValidateArgs {
    /// Path to the skill tree TOML file (or multiple files)
    #[arg(required = true)]
    skill_trees: Vec<PathBuf>,

    /// Surpress output on success; only print errors
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Render(render_args) => cmd_render(render_args),
        Command::Validate(validate_args) => cmd_validate(validate_args),
    }
}

//  -------------------------------- Render --------------------------
fn cmd_render(args: RenderArgs) -> anyhow::Result<()> {
    let skill_tree = SkillTree::load(&args.skill_tree)
        .with_context(|| format!("loading `{}`", args.skill_tree.display()))?;

    skill_tree.validate()?;

    let mut dot_file = File::create(&args.output_path)
        .with_context(|| format!("craeteing `{}`", args.output_path.display()))?;

    skill_tree
        .write_graphviz(&mut dot_file)
        .with_context(|| format!("writing `{}`", args.output_path.display()))?;

    Ok(())
}
//  -------------------------------- Validate ------------------------
fn cmd_validate(args: ValidateArgs) -> anyhow::Result<()> {
    let mut all_ok = true;

    for path in &args.skill_trees {
        match validate_one(path) {
            Ok(()) => {
                if !args.quiet {
                    eprintln!("✓ {}", path.display());
                }
            }
            Err(err) => {
                eprintln!("✗ {}", path.display());
                let mut source = err.source();

                while let Some(cause) = source {
                    eprintln!("  caused by: {}", cause);
                    source = cause.source();
                }

                all_ok = false;
            }
        }
    }

    if all_ok {
        if !args.quiet && args.skill_trees.iter().len() > 1 {
            eprintln!("All {} files valid.", args.skill_trees.len());
        }
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn validate_one(path: &PathBuf) -> anyhow::Result<()> {
    let tree = SkillTree::load(path).with_context(|| format!("loading `{}`", path.display()))?;

    tree.validate()
        .with_context(|| format!("validating `{}`", path.display()))?;

    Ok(())
}
