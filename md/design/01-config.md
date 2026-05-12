# Configuration

skill-tree is configured via a `.skill-tree.toml` file in the current
directory. This file tells skill-tree which GitHub Project to read and
how to display what it finds.

Configuration is read once at startup. Changes to `.skill-tree.toml` take
effect on the next invocation.

## Field auto-discovery

skill-tree fetches **all** custom fields GitHub returns for every project
item, regardless of what is declared in `[[field]]`. You do not need to
declare a field to have it fetched.

`[[field]]` entries are display declarations only — they give a field a
friendly `display-name` for CLI output. Fields not declared in `[[field]]`
are still fetched and stored on each node. Adding a new `[[field]]` entry
or a new value in `[colors.values]` later does not require changing what
gets fetched.

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
`github.com/users/your-username/projects/1`, `owner` is `"your-username"`.

### `[[field]]`

Gives a GitHub Projects custom field a friendly display name for CLI
output. Optional — skill-tree fetches all fields regardless.

| Field | Type | Description |
|---|---|---|
| `display-name` | string | How skill-tree refers to this field in CLI output |
| `github-name` | string | Exact field name as it appears in GitHub Projects |

`github-name` is case-sensitive and must match the field name in GitHub
Projects character for character. Unknown keys are rejected at parse time.

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

## The `SkillTree` application context

The parsed `Config` is wrapped in a `SkillTree` struct that also carries
the directory containing the config file. The rest of the pipeline takes
`&SkillTree` rather than `&Config` directly — this keeps configuration
threading explicit and avoids global state. Constructors:

- `SkillTree::from_dir(dir)` — load from a directory (production and tests)
- `SkillTree::from_path(path)` — load from an explicit file path

## Validation

After parsing, skill-tree runs validation on the config and fails with
exit code 4 if any value in `[colors.values]` is not a valid hex color
(`#rgb` or `#rrggbb`).

Other failures happen at parse time, not validation time:

- Missing `[github]` or its required keys
- A `[[field]]` entry with unknown keys
- Type mismatches on any field

## Example: minimal config

The smallest valid config — no field declarations, no colors:

```toml
[github]
owner   = "your-org"
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

## Common pitfalls

- **Case mismatch on `github-name`.** `"status"` and `"Status"` are different
  fields. The value must match GitHub's field header character for character.
- **Forgetting the `#` on a hex color.** `"4a90d9"` is rejected. The leading
  `#` is required.
- **Quoting numeric values.** `project = "42"` is rejected — the field is
  an integer, not a string.
- **Mixing case in `[colors.values]` keys.** `"in progress"` does not match
  `"In Progress"`. Match GitHub's option names exactly.