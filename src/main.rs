//! skill-tree binary entry point.
//! Parses CLI arguments and dispatches to render, unblocked, or validate.

use skill_tree::config::SkillTree;

fn main() {
    println!("Hello world!");

    let config = SkillTree::from_dir(".").unwrap();

    println!("{:#?}", config);
}
