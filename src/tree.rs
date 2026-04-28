use anyhow::Context;
use serde::{Deserialize, de::Visitor};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::error::SkillTreeError;

#[derive(Debug, Deserialize)]
pub struct SkillTree {
    pub group: Option<Vec<Group>>,
    pub cluster: Option<Vec<Cluster>>,
    pub graphviz: Option<Graphviz>,
    pub doc: Option<Doc>,
}

#[derive(Debug, Deserialize)]
pub struct Graphviz {
    pub rankdir: Option<String>,
}

#[derive(Default, Debug, Deserialize)]
pub struct Doc {
    pub columns: Option<Vec<String>>,
    pub defaults: Option<HashMap<String, String>>,
    pub emoji: Option<HashMap<String, EmojiMap>>,
    pub include: Option<Vec<PathBuf>>,
}

pub type EmojiMap = HashMap<String, String>;

#[derive(Debug, Deserialize)]
pub struct Cluster {
    pub name: String,
    pub label: String,
    pub color: Option<String>,
    pub style: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Group {
    pub name: String,
    pub cluster: Option<String>,
    pub label: Option<String>,
    pub requires: Option<Vec<String>>,
    pub description: Option<Vec<String>>,
    pub items: Vec<Item>,
    pub width: Option<f64>,
    pub status: Option<Status>,
    pub href: Option<String>,
    pub header_color: Option<String>,
    pub description_color: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct GroupIndex(pub usize);

#[derive(Debug, Clone)]
pub struct Item {
    pub label: String,
    pub href: Option<String>,
    pub status: Option<Status>,
    pub attrs: HashMap<String, String>,
}

#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct ItemIndex(pub usize);

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Can't work on it now
    Blocked,

    /// Would like to work on it, but need someone
    Unassigned,

    /// People are actively working on it
    Assigned,

    /// This is done!
    Complete,
}

// -------------------------------- SkillTree impl -------------------
impl SkillTree {
    pub fn load(path: &Path) -> anyhow::Result<SkillTree> {
        let loaded = &mut HashSet::default();
        loaded.insert(path.to_owned());
        Self::load_included_path(path, loaded)
    }

    fn load_included_path(path: &Path, loaded: &mut HashSet<PathBuf>) -> anyhow::Result<SkillTree> {
        fn load(path: &Path, loaded: &mut HashSet<PathBuf>) -> anyhow::Result<SkillTree> {
            let skill_tree_text = std::fs::read_to_string(path)?;
            let mut tree = SkillTree::parse(&skill_tree_text)?;
            tree.import(path, loaded)?;
            Ok(tree)
        }

        load(path, loaded).with_context(|| format!("loading skill tree from `{}`", path.display()))
    }

    fn import(&mut self, root_path: &Path, loaded: &mut HashSet<PathBuf>) -> anyhow::Result<()> {
        if let Some(doc) = &mut self.doc {
            if let Some(include) = &mut doc.include {
                let include = include.clone();
                for include_path in include {
                    if !loaded.insert(include_path.clone()) {
                        continue;
                    }

                    let tree_path = root_path.parent().unwrap().join(&include_path);
                    let mut toml: SkillTree = SkillTree::load_included_path(&tree_path, loaded)?;

                    // merge columns, and any defaults/emojis associated with the new columns
                    let self_doc = self.doc.get_or_insert(Doc::default());
                    let toml_doc = toml.doc.get_or_insert(Doc::default());
                    for column in toml_doc.columns.get_or_insert(vec![]).iter() {
                        let columns = self_doc.columns.get_or_insert(vec![]);
                        if !columns.contains(column) {
                            columns.push(column.clone());

                            if let Some(value) =
                                toml_doc.emoji.get_or_insert(HashMap::default()).get(column)
                            {
                                self_doc
                                    .emoji
                                    .get_or_insert(HashMap::default())
                                    .insert(column.clone(), value.clone());
                            }

                            if let Some(value) = toml_doc
                                .defaults
                                .get_or_insert(HashMap::default())
                                .get(column)
                            {
                                self_doc
                                    .defaults
                                    .get_or_insert(HashMap::default())
                                    .insert(column.clone(), value.clone());
                            }
                        }
                    }

                    self.group
                        .get_or_insert(vec![])
                        .extend(toml.groups().cloned());

                    self.cluster
                        .get_or_insert(vec![])
                        .extend(toml.cluster.into_iter().flatten());
                }
            }
        }
        Ok(())
    }

    pub fn parse(text: &str) -> anyhow::Result<SkillTree> {
        Ok(toml::from_str(text)?)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        // 1. Per-group validation (unknown deps, missing labels).
        for group in self.groups() {
            group.validate(self)?;
        }

        // 2. Cycle detection across the whole graph
        let groups: Vec<&Group> = self.groups().collect();
        detect_cycles(&groups)?;
        Ok(())
    }

    pub fn groups(&self) -> impl Iterator<Item = &Group> {
        match self.group {
            Some(ref g) => g.iter(),
            None => [].iter(),
        }
    }

    pub fn group_named(&self, name: &str) -> Option<&Group> {
        self.groups().find(|g| g.name == name)
    }

    /// Returns the expected column titles for each item (excluding the label).
    pub fn columns(&self) -> &[String] {
        if let Some(doc) = &self.doc {
            if let Some(columns) = &doc.columns {
                return columns;
            }
        }

        &[]
    }

    /// Translates an "input" into an emoji, returning "input" if not found.
    pub fn emoji<'me>(&'me self, column: &str, input: &'me str) -> &'me str {
        if let Some(doc) = &self.doc {
            if let Some(emoji_maps) = &doc.emoji {
                if let Some(emoji_map) = emoji_maps.get(column) {
                    if let Some(output) = emoji_map.get(input) {
                        return output;
                    }
                }
            }
        }
        input
    }
}

// ------------------------ Cycle Detection ---------------------

fn detect_cycles(groups: &[&Group]) -> anyhow::Result<()> {
    // State: 0 = unvisited, 1 = currently on the DFS stack, 2 = fully visited.

    let mut state: HashMap<&str, u8> = HashMap::new();

    let lookup = groups.iter().map(|gr| (gr.name.as_str(), *gr)).collect();

    for group in groups {
        if state.get(group.name.as_str()).copied().unwrap_or(0) == 0 {
            dfs(group.name.as_str(), &lookup, &mut state, &mut vec![])?;
        }
    }

    Ok(())
}

fn dfs<'a>(
    name: &'a str,
    lookup: &HashMap<&'a str, &'a Group>,
    state: &mut HashMap<&'a str, u8>,
    path: &mut Vec<&'a str>,
) -> anyhow::Result<()> {
    match state.get(name).copied().unwrap_or(0) {
        // Already fully explored - nothing to do.
        2 => return Ok(()),
        // Back-edge: we've found a cycle.
        1 => {
            let cycle_start = path.iter().position(|&n| n == name).unwrap_or(0);

            let cycle = path[cycle_start..].join(" → ") + " → " + name;

            anyhow::bail!(SkillTreeError::CircularDependency { cycle });
        }
        _ => {}
    }
    state.insert(name, 1);
    path.push(name);

    if let Some(group) = lookup.get(name) {
        for dep in group.requires.iter().flatten() {
            if lookup.contains_key(dep.as_str()) {
                dfs(dep.as_str(), lookup, state, path)?;
            }
        }
    }

    path.pop();
    state.insert(name, 2);
    Ok(())
}

// ------------------- Group impl -----------------------
impl Group {
    pub fn validate(&self, tree: &SkillTree) -> anyhow::Result<()> {
        for group_name in self.requires.iter().flatten() {
            if tree.group_named(group_name).is_none() {
                anyhow::bail!(SkillTreeError::UnknownDependency {
                    group: self.name.clone(),
                    dep: group_name.clone()
                })
            }
        }

        // Check that every item has a non-empty label.
        for item in &self.items {
            if item.label.trim().is_empty() {
                anyhow::bail!(SkillTreeError::MissingLabel {
                    group: self.name.clone()
                })
            }
        }

        Ok(())
    }

    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.items.iter()
    }
}

pub trait ItemExt {
    fn href(&self) -> Option<String>;
    fn label(&self) -> String;
    fn column_value<'me>(&'me self, tree: &'me SkillTree, c: &str) -> &'me str;
    fn validate(&self) -> anyhow::Result<()>;
}

impl ItemExt for Item {
    fn href(&self) -> Option<String> {
        self.href.clone()
    }

    fn label(&self) -> String {
        self.label.clone()
    }

    fn column_value<'me>(&'me self, tree: &'me SkillTree, c: &str) -> &'me str {
        if let Some(value) = self.attrs.get(c) {
            return value;
        }

        if let Some(doc) = &tree.doc {
            if let Some(defaults) = &doc.defaults {
                if let Some(default_value) = defaults.get(c) {
                    return default_value;
                }
            }
        }

        ""
    }

    fn validate(&self) -> anyhow::Result<()> {
        // check: each of the things in requires has the form
        //        `identifier` or `identifier:port` and that all those
        //        identifiers map to groups

        // check: only contains known keys

        Ok(())
    }
}

// ------------------------ Item Deserialization ---------------------
impl<'de> Deserialize<'de> for Item {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ItemVisitor;

        impl<'de> Visitor<'de> for ItemVisitor {
            type Value = Item;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an item table with at least a `label` field")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut label = None;
                let mut href = None;
                let mut status = None;
                let mut attrs = HashMap::new();

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "label" => {
                            label = Some(map.next_value::<String>()?);
                        }
                        "href" => {
                            href = Some(map.next_value::<String>()?);
                        }
                        "status" => {
                            let raw = map.next_value::<String>()?;
                            match raw.as_str() {
                                "complete" => status = Some(Status::Complete),
                                "assigned" => status = Some(Status::Assigned),
                                "unassigned" => status = Some(Status::Unassigned),
                                "blocked" => status = Some(Status::Blocked),
                                _ => {
                                    attrs.insert(key, raw);
                                }
                            }
                        }
                        _ => {
                            let value: toml::Value = map.next_value()?;
                            if let toml::Value::String(s) = value {
                                attrs.insert(key, s);
                            }
                        }
                    }
                }
                Ok(Item {
                    label: label.ok_or_else(|| serde::de::Error::missing_field("label"))?,
                    href,
                    status,
                    attrs,
                })
            }
        }
        deserializer.deserialize_map(ItemVisitor)
    }
}
