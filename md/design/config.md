# Configuration

skill-tree is configured via a `.skill-tree.toml` file in the current
directory. This file tells skill-tree which GitHub Project to read and
how to display what it finds.

## File format

```toml
[github]
owner   = "rust-lang"
project = 42

[[field]]
display-name = "status"
github-name  = "Status"

[[field]]
display-name = "priority"
github-name  = "Priority"

[colors]
github-name = "Status"

[colors.values]
"In Progress" = "#4a90d9"
"Blocked"     = "#e05252"
"Complete"    = "#57a85a"
"Not Started" = "#888888"
```

## Sections

### `[github]`

Identifies the GitHub Project to fetch data from.

| Field | Type | Description |
|---|---|---|
| `owner` | string | GitHub organization or user that owns the project |
| `project` | integer | Project number from the GitHub Projects URL |

For `github.com/orgs/rust-lang/projects/42`, `owner` is `"rust-lang"`
and `project` is `42`. For a user project at
`github.com/users/nikomatsakis/projects/1`, `owner` is `"nikomatsakis"`.

### `[[field]]`

Declares a GitHub Projects custom field that skill-tree should read.
You can have as many `[[field]]` entries as you like. Each one declares
one field.

| Field | Type | Description |
|---|---|---|
| `display-name` | string | How skill-tree refers to this field internally |
| `github-name` | string | Exact field name as it appears in GitHub Projects |

`github-name` is case-sensitive and must match the field name in GitHub
Projects character for character. `display-name` is your choice — it is
only used internally and does not need to match anything in GitHub.

### `[colors]`

Controls node color in the rendered graph.

| Field | Type | Description |
|---|---|---|
| `github-name` | string | Which GitHub field drives node color |
| `values` | table | Maps field option values to hex colors |

`github-name` must match the `github-name` of one of your `[[field]]`
entries. The keys in `[colors.values]` must match the option names in
that field's single-select options in GitHub Projects exactly, including
case and spacing.

Nodes whose field value does not appear in `[colors.values]` render with
a default gray (`#dddddd`).

## Validation

skill-tree validates the config file on startup and fails with exit code 4
if any of the following are true:

- `[github]` is missing or incomplete
- `[colors] github-name` does not match any declared `[[field]]`
- Any value in `[colors.values]` is not a valid hex color (`#rgb` or `#rrggbb`)
- No `[[field]]` entries are declared

## Example: minimal config

The smallest valid config — one field, colors driven by that field:

```toml
[github]
owner   = "nikomatsakis"
project = 1

[[field]]
display-name = "status"
github-name  = "Status"

[colors]
github-name = "Status"

[colors.values]
"In Progress" = "#4a90d9"
"Done"        = "#57a85a"
```

## Example: multiple fields

Declaring additional fields does not change rendering unless you wire
them to colors. Extra fields are fetched and stored on each node for
future use by filters and queries:

```toml
[github]
owner   = "rust-lang"
project = 42

[[field]]
display-name = "status"
github-name  = "Status"

[[field]]
display-name = "priority"
github-name  = "Priority"

[[field]]
display-name = "assignee"
github-name  = "Assignee"

[colors]
github-name = "Status"

[colors.values]
"In Progress" = "#4a90d9"
"Blocked"     = "#e05252"
"Complete"    = "#57a85a"
"Not Started" = "#888888"
```