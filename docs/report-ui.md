# Report panel

## Checking

The panel checks the theme when it opens. The upper-right Refresh icon (or R) forces a fresh check and is disabled while one runs. Closing the panel cancels the check. There is no startup audit, polling or queued work.

A check that finishes quickly (usually a cache hit) shows no transition. If one takes longer than about a third of a second, the verdict reads "Previous result" and the grade tile shows "—" until it finishes, so a retained result is never mistaken for the current one. Analyzer errors appear under the verdict, next to the retained result.

## Badge

The badge shows the last result's letter, `*` when provisional, and `?` when there is none. At startup it reads the last published result from the cache, which is a file read, not an audit. It also picks up results written by the optional theme-switch hook. When `theme.name` no longer matches the result's theme, the badge shows `~` before the letter and the tooltip names the previous theme.

## IPC

`open`, `close`, `toggle` and `openCategory` act on the copy on the focused monitor. `openCategory` selects the category and its most useful tab, including when the panel opens before any result has loaded. A background `check` or `refresh` shows "Checking…" on every badge and hands the result to every copy. The method reference is in `docs/installation.md`.

## Overview

The header shows the theme name (title-cased), a five-row grade card, and counts of problems, cautions and limitations. When some checks are paused because the installed Omarchy build changed a behavior this analyzer mirrors, a notice under the verdict names what changed. In the Limitations view, the paused checks collapse into a single card that lists them.

The color palette shows two ordered swatch banks for authored ANSI normal/bright colors and four resolved role chips (Base, Text, Accent, Urgent). Semantic-only themes show their named chromatic colors instead of invented ANSI slots. Hover or focus identifies a color; click or Enter copies its hex. This uses report evidence, never direct theme-file reads.

## Exploring findings

Select a category to filter; a healthy category opens **Passed** automatically. Tab counts reflect the selected category. Issues lists failures first, then cautions, in registry order.

Expand a check for:

- foreground/background previews and a contrast track marking the target (indicator and border checks show a boundary symbol rather than text);
- normal versus selected/focused swatches and an OKLab separation track;
- the four resolved core colors;
- wallpaper sample grids rebuilt from the report's 64 measured colors per image (coarse samples, not previews);
- context, source files and keys, and what to change. **Technical data** reveals the exact evidence JSON.

Preview hex values are rounded; unrounded measurements decide grades. Track lengths encode contrast or separation, not scores. A wallpaper deficit already charged under Text & contrast appears in Notes, not as a pass.

Expanded checks survive same-theme refreshes; a different theme resets exploration. Arrow and Page keys scroll; Escape closes. Category rows and check headers expose accessible names with their grade or state.

## Colors

Panel chrome follows the live popup palette, transparency and typography. Cautions use the theme's own yellow (`yellow`, `color3` or `color11`) when it reaches 3:1 against the popup background. Otherwise they use a blend of urgent toward the text color, so a caution never looks like a pass even when accent equals text color. Evidence swatches keep their measured RGB values. A very low-contrast theme can make the panel itself harder to read; CLI and JSON output stay available.

## Data contract

Schema v2. The panel reads optional evidence fields (`foreground`, `effectiveBackground`, `normalColor`, `stateColor`, `resolvedColors`, `paletteColors`, thumbnail `sampleColors`) and the optional top-level `runtimeContract`. Full source media never reaches QML.

## Tests

`python3 tests/panel.py` runs `tests/qml/report-smoke.qml` against a sandboxed copy of the native `Ui/` and `Commons/` modules. It checks palette rebinding, popup alpha, fonts, filters and counts, honesty rules, refresh behavior, the caution hue, paused-check grouping and banner, and header errors, then lints the QML. `python3 tests/refresh.py` drives `Main.qml` with real Quickshell processes. It checks that nothing audits at startup or on file changes, exact helper invocation counts, manual Refresh, cancellation on close, the shared cached result, and the stale badge after a theme switch.
