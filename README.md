# Theme Report

A read-only health check for Omarchy themes. A small bar badge shows your current theme's grade; click it for a report card with the evidence behind every finding.

![Native report panel](docs/evidence/ui-overview.png)

## What it checks

| Category | What it measures |
| --- | --- |
| Theme integrity | `colors.toml` parses, the four core colors resolve, shell values are in range, wallpapers decode |
| Text & contrast | Text against its background for the terminal, bar, popups, notifications, menus, launcher, polkit, lock and tooltips |
| State distinction | Accent, error and selection colors stand apart from the base |
| Shell surfaces | Selected/focused fills and popup borders are visible |
| Wallpaper resilience | Lock and launcher text over your theme's actual wallpapers (sampled) |

It does **not** judge taste, count optional files, or grade publishing polish. An A is a good sign, not an accessibility certificate.

## Reading the badge

| Badge | Meaning |
| --- | --- |
| `A`–`F` | Grade from the last check |
| `B*` | Provisional: some checks could not be measured (open the panel to see which) |
| `~B` | You switched themes since this result; open the panel to check the new one |
| `?` | No result yet |

The panel checks your theme when you open it. Use the Refresh icon (or press R) to force a fresh check. Nothing runs in the background unless you install the optional theme-switch hook.

The panel reads like a report card: the overall grade and a tally of problems up top, then one card per area with its own grade. Areas that need work open on their own, with their problems listed right under the grade. Click any area to open or close it, and any check to see color previews, contrast against its target, sampled wallpaper grids, the files and keys involved, and what to change.

## Fix it with your agent

When the report finds problems, the panel offers **Fix N issues with Pi**, using whichever agent you chose with `omarchy default agent`. It opens your agent in a new terminal window with a prompt built from the measurements: the exact checks, their targets, and the `colors.toml` and `shell.toml` keys behind them. Expanded checks offer the same for a single check, and **Copy prompt** or **Preview** let you see or paste the text instead of launching anything.

Omarchy starts agents without permission prompts, so the prompt sets the limits:
- the agent edits only the theme source, on a new git branch when the theme is a repository, otherwise in a `<name>-tuned` copy (stock themes are always copied);
- it never touches wallpapers or the applied theme;
- it treats the theme's files as data: it follows no instructions and runs no commands found in them, and stays off the network;
- it states its plan before editing, and asks before applying the result.

It verifies its work with `theme-report audit`. When nothing needs fixing but some checks could not be measured, the button asks the agent about those instead.

**Review with Pi** asks for a thorough, read-only review that goes beyond the measured checks. It's offered on every theme, including passing ones. The agent reads the theme's files and saves a Markdown write-up, which the panel shows as an **Agent review** card at the top. The card is marked out of date once the theme changes.

Install the optional `theme-report` agent skill with `bash scripts/install.sh --with-skill`. Prompts work without it; with it, agents get more detail on reading the report and making safe edits.

## Install

Theme Report has two parts: the bar widget in this repository, and the `theme-report` analyzer it runs to do the measuring. You build the analyzer once on your own machine, which needs Rust (`cargo`). The build fetches the crates pinned in `Cargo.lock`; after that nothing is downloaded, and nothing needs root.

**With Omarchy's plugin manager:**

```sh
omarchy plugin add https://github.com/OldJobobo/theme-report.git
cd ~/.config/omarchy/plugins/oldjobobo.theme-report
export CARGO_TARGET_DIR=~/.cache/theme-report/build   # keeps build files out of ~/.config
cargo build --release --locked
bash scripts/install.sh      # the analyzer; add --with-hook and/or --with-skill
omarchy plugin enable oldjobobo.theme-report
```

Run from the plugin folder, `install.sh` installs only the analyzer (and the hook or skill if you ask for them) and leaves the plugin files to `omarchy plugin update`. After an update, rebuild and run `install.sh` again so the analyzer matches the widget.

**From a source checkout:**

```sh
cargo build --release --locked
bash scripts/install.sh --dry-run   # preview
bash scripts/install.sh             # add --with-hook to re-check on every theme switch
omarchy-shell shell rescanPlugins
omarchy plugin enable oldjobobo.theme-report
```

Here `install.sh` also copies the plugin files. When you update an existing install, it tells you to run `omarchy restart shell` so the new plugin code loads cleanly.

Use one method or the other: both install to `~/.config/omarchy/plugins/oldjobobo.theme-report`. To switch, remove the first install.

**Removing it.** `bash scripts/uninstall.sh` disables the widget and deletes only files it installed, unchanged. For a plugin-manager install, run it from the plugin folder, then `omarchy plugin remove oldjobobo.theme-report --yes`.

**Dependencies.** Required: Rust to build the analyzer. Optional: `ffmpeg`/`ffprobe` for video wallpapers and ImageMagick for color-profiled or very large images; without them those wallpapers show up as limitations, never as failures. The agent features use the agent you chose with `omarchy default agent`, and `wl-copy` for `theme-report agent --copy`.

See `docs/installation.md` for what the widget, analyzer and agent prompts are allowed to do.

## Command line

```sh
theme-report audit --current              # your active theme
theme-report audit --current --format json
theme-report audit ~/path/to/theme --profile quattro-shell-2026-v1
theme-report runtime                      # can this analyzer verify your Omarchy build?
theme-report explain readability.bar
theme-report rules
theme-report prompt fix                   # print the agent prompt for the last report
theme-report agent fix --rule readability.bar   # open your agent on one check (or --copy)
```

## Keyboard and scripts

The widget answers IPC on `oldjobobo.theme-report`, so you can bind keys or script it:

```sh
omarchy-shell oldjobobo.theme-report toggle            # open/close on the focused monitor
omarchy-shell oldjobobo.theme-report openCategory readability
omarchy-shell oldjobobo.theme-report check             # re-check without opening the panel
omarchy-shell oldjobobo.theme-report status            # JSON: grade, state, counts, checking...
omarchy-shell oldjobobo.theme-report askAgent fix       # open your agent on the current issues
```

Also `open`, `close`, `refresh`, `grade`, `fixRule` and `copyPrompt`. See `docs/installation.md` for the full reference.

## When Omarchy updates

Theme Report mirrors a few pieces of Omarchy's runtime: how `colors.toml` loads, the default surface and control colors, and terminal selection fallbacks. It verifies each piece separately, so unrelated Omarchy changes do not affect it. If a piece it relies on changes, only the checks that depend on it pause, and the panel says so. It never guesses a grade. Run `theme-report runtime` to see what is verified.

## More

- `docs/installation.md`: install, CLI and troubleshooting
- `docs/report-ui.md`: the panel in detail
- `rules/v1.md`: every rule, its budget and thresholds
- `docs/measurement.md`: formulas, sampling and limits
- `docs/supported-runtime.md`: runtime verification and input resolution
- `schema/report-v2.schema.json`: JSON report format
- `CHANGELOG.md`: release history

MIT licensed.
