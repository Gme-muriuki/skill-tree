use anyhow::Context;
use clap::Parser;
use skill_tree::SkillTree;
use std::fs::File;
use std::path::PathBuf;

/// Generate graphviz dot files to show roadmaps
#[derive(Parser, Debug)]
#[command(name = "skill-tree")]
struct Opts {
    /// Path to the skill tree TOML file
    skill_tree: PathBuf,

    /// Output path for the generated dot file
    output_path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    // Load the skill tree
    let skill_tree = SkillTree::load(&opts.skill_tree)
        .with_context(|| format!("loading skill tree from `{}`", opts.skill_tree.display()))?;

    // Validate it for errors.
    skill_tree.validate()?;

    // Write out the dot file
    write_dot_file(&skill_tree, &opts)
}

fn write_dot_file(skill_tree: &SkillTree, opts: &Opts) -> anyhow::Result<()> {
    let dot_path = &opts.output_path;
    let mut dot_file =
        File::create(dot_path).with_context(|| format!("creating `{}`", dot_path.display()))?;
    skill_tree
        .write_graphviz(&mut dot_file)
        .with_context(|| format!("writing to `{}`", dot_path.display()))?;
    Ok(())
}
