# skill-tree

skill-tree fetches a GitHub Project and renders it as a directed dependency graph.

## What is a skill tree?

A "skill tree" is a way to map out the roadmap for a project. The term is
borrowed from video games, but it was first applied to project planning in
this [blog post about WebAssembly's post-MVP future][wasm] — at least, that
was the first time it was used that way.

[wasm]: https://hacks.mozilla.org/2018/10/webassemblys-post-mvp-future/

The idea: work items have dependencies, just like skills in a game. You cannot
unlock the next thing until the current thing is done. Mapping those
dependencies visually shows you the shape of a roadmap at a glance.

## How it works

skill-tree reads a GitHub Project — issues, their blocking relationships, and
their custom field values — and renders the result as a Graphviz DOT file or
SVG. Each node is a GitHub issue. Each edge is a blocking relationship. Node
color is driven by a custom field in GitHub Projects.

GitHub is the source of truth. There is no separate file to maintain.

## Usage

```bash
# Render the dependency graph as SVG
skill-tree render --format svg --output graph.svg

# List open issues with no incoming blocking edges
skill-tree unblocked

# Check for cycles and dangling references
skill-tree validate
```

## Configuration

Create a `.skill-tree.toml` in your project root:

```toml
[github]
owner   = "rust-lang"
project = 42

[[field]]
display-name = "status"
github-name  = "Status"

[colors]
github-name = "Status"

[colors.values]
"In Progress" = "#4a90d9"
"Blocked"     = "#e05252"
"Complete"    = "#57a85a"
"Not Started" = "#888888"
```

`owner` is the GitHub organization or user that owns the project.
`project` is the project number from the GitHub Projects URL.

skill-tree fetches all custom fields GitHub returns automatically. `[[field]]`
entries are display declarations only — they give a field a friendly
`display-name` for CLI output. Fields not declared in `[[field]]` are still
fetched and stored on each node.

`[colors]` specifies which GitHub field drives node color (`github-name`)
and maps that field's option values to hex colors (`[colors.values]`).
The entire section is optional — if omitted, all nodes render gray.

## Installation

```bash
cargo install skill-tree
```

Rendering SVG requires Graphviz:

```bash
# macOS
brew install graphviz

# Ubuntu
apt install graphviz
```

## Authentication

skill-tree reads your GitHub token from the `GITHUB_TOKEN` environment
variable:

```bash
export GITHUB_TOKEN=<your token>
```

The token requires `read:project` and `repo` scopes.

## Documentation

For architecture, design decisions, and contribution guide, see the
[skill-tree design book](https://nikomatsakis.github.io/skill-tree/).

## Status

⚠️ **Early development** — expect frequent changes.

## Community

skill-tree is open source. We welcome contributors and maintain a
[code of conduct](./CODE_OF_CONDUCT.md).