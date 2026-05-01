# Validate a preset before distributing

`agx validate` checks a preset file (or many files) for correctness without
rendering an image. It catches typos, type mismatches, out-of-range values,
missing LUT files, and broken `extends` chains.

## When to use it

- **Before distributing a preset** to other users or publishing to a preset library.
- **In CI** for a preset library, to ensure all presets stay valid as the schema evolves.
- **As a pre-commit hook** in a presets repo.
- **When `agx apply` produces unexpected output** — the validator gives more detail than apply-time warnings.

## Single file

```bash ignore
agx validate looks/portra.toml
```

Output for a clean preset:

```
looks/portra.toml: ok

1 file checked, all ok
```

Output for a preset with errors:

```
looks/portra.toml: 2 problems
  error: unknown table `tone_curves` (line 12)
  error: `tone.exposure` value 99.0 outside allowed range (line 5)

1 file checked, 1 with error
```

## Multiple files

Use a shell glob:

```bash ignore
agx validate looks/*.toml
```

The shell expands the glob; `agx validate` accepts each as a positional
argument. Exits 0 if all files are clean, 1 if any has errors.

## CI integration

Use `--format=json` for machine-parseable output:

```bash ignore
agx validate --format=json looks/*.toml | jq '.files[] | select(.status == "error")'
```

The JSON shape is documented in the [CLI reference](../reference/cli.md#agx-validate).
Diagnostic codes (`unknown-table`, `out-of-range`, etc.) are stable for
filtering and suppression in tooling.

A typical GitHub Actions step:

```yaml
- name: Validate presets
  run: agx validate looks/*.toml
```

The job fails if any preset has errors.

## Quiet mode

`--quiet` (or `-q`) skips "ok" lines, useful when validating many files
and only the broken ones matter:

```bash ignore
agx validate --quiet looks/*.toml
```

## What gets checked

| Category | Check |
|---|---|
| **Structure** | Unknown fields and tables (e.g., typo `[tone_curves]` vs `[tone_curve]`) |
| **Structure** | Type mismatches (e.g., `exposure = "high"` instead of a number) |
| **Structure** | Missing required fields |
| **Semantic** | Out-of-range numeric values (e.g., `exposure = 99.0` outside \[-5.0, 5.0\]) |
| **Filesystem** | LUT file referenced by `[lut] path` exists on disk |
| **Filesystem** | `extends` chain references existing files and has no cycles |

## Difference from `agx apply`

`agx apply` is forgiving — it warns about unknown fields on stderr but still
produces output. `agx validate` is strict — anything sketchy fails. Use
validate when you want to know your preset is correct; use apply when you
want the rendered image.

## See also

- [Write your own preset](write-preset.md)
- [Extend a preset](extend-preset.md)
- [CLI reference: `agx validate`](../reference/cli.md#agx-validate)
