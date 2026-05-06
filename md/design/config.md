# Configuration

skill-tree is configured via a `.skill-tree.toml` file in the current
directory. This file tells skill-tree which GitHub Project to read and
how to display what it finds.

## Field auto-discovery

skill-tree fetches **all** custom fields GitHub returns for every project
item, regardless of what is declared in `[[field]]`. You do not need to
declare a field to have it fetched.

`[[field]]` entries are display declarations only — they give a field a
friendly `display-name` for CLI output. Fields not declared in `[[field]]`
are still fetched and stored on each node. Adding a new `[[field]]` entry
or a new `[[color-rule]]` later does not require changing what gets fetched.

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

| Field | Type | Required | Description |
|---|---|---|---|
| `owner` | string | yes | GitHub organization or user that owns the project |
| `project` | integer | yes | Project number from the GitHub Projects URL |

For `github.com/orgs/rust-lang/projects/42`, `owner` is `"rust-lang"`
and `project` is `42`. For a user project at
`github.com/users/nikomatsakis/projects/1`, `owner` is `"nikomatsakis"`.

### `[[field]]`

Gives a GitHub Projects custom field a friendly display name for CLI
output. Optional — skill-tree fetches all fields regardless.

| Field | Type | Description |
|---|---|---|
| `display-name` | string | How skill-tree refers to this field in CLI output |
| `github-name` | string | Exact field name as it appears in GitHub Projects |

`github-name` is case-sensitive and must match the field name in GitHub
Projects character for character. Unknown keys are rejected.

### `[colors]`

Controls node color in the rendered graph. The entire section is optional.
If omitted, all nodes render with the default gray.

| Field | Type | Description |
|---|---|---|
| `github-name` | string | Which GitHub field drives node color |
| `values` | table | Maps field option values to hex colors |

`github-name` does not need to match a declared `[[field]]` entry —
it refers directly to the GitHub field name. The keys in `[colors.values]`
must match the option names in that field's single-select options in GitHub
Projects exactly, including case and spacing.

Nodes whose field value does not appear in `[colors.values]` render with
the default gray (`#dddddd`).

## Validation

skill-tree validates the config file on startup and fails with exit code 4
if any of the following are true:

- `[github]` is missing or incomplete
- Any value in `[colors.values]` is not a valid hex color (`#rgb` or `#rrggbb`)
- A `[[field]]` entry contains unknown keys

## Example: minimal config

The smallest valid config — no field declarations, no colors:

```toml
[github]
owner   = "nikomatsakis"
project = 1
```

skill-tree fetches all fields from the board and renders nodes in the
default gray. Add `[colors]` when you are ready to add color.

## Example: colors only, no field declarations

```toml
[github]
owner   = "rust-lang"
project = 42

[colors]
github-name = "Status"

[colors.values]
"In Progress" = "#4a90d9"
"Blocked"     = "#e05252"
"Complete"    = "#57a85a"
```

No `[[field]]` declarations needed. skill-tree fetches the Status field
automatically along with everything else on the board.

## Example: full config with display names

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