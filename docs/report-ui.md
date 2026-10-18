# Report panel

## Checking

The panel checks the theme when it opens. The Refresh icon in the upper right (or R) forces a fresh check and is disabled while one runs. Closing the panel cancels the check. Nothing checks at startup, on a timer, or in a queue.

A check that finishes quickly (usually a cache hit) shows no transition. If one takes longer than about a third of a second, the verdict reads "Previous result" and the grade tile shows "—" until it finishes, so an old result is never mistaken for the current one. Analyzer errors appear under the verdict, next to the last good result.

## Badge

The badge shows the last result's letter, `*` when provisional, and `?` when there is none. At startup it reads the last saved result from the cache; that is a file read, not a check. It also picks up results written by the optional theme-switch hook. When `theme.name` no longer matches the result's theme, the badge shows `~` before the letter and the tooltip names the previous theme.

## IPC

`open`, `close`, `toggle` and `openCategory` act on the copy on the focused monitor. `openCategory` opens that area's card, scrolls to it and outlines it, including when the panel opens before any result has loaded. A background `check` or `refresh` shows "Checking…" on every badge and hands the result to every copy. The method reference is in `docs/installation.md`.

## Overview

The panel is laid out as a report card. The fixed header holds a grade stamp (colored by grade, `*` when provisional), the title-cased theme name, the verdict, and count chips for problems, cautions, checks not measured and passes. The last check time and the Refresh icon sit at the top right; the icon spins while a check runs. Analyzer errors and paused-check notices appear as callouts under the verdict, naming what changed in the installed Omarchy build.

Below it, each area gets a card: its letter grade, a score meter (the share of its budget it kept) and a one-line summary such as "3 problems · 1 not measured · 6 passed". Problem and caution counts carry their own colors. Hovering a card's header explains what the area covers.

After the areas, the palette shows the theme's ANSI colors in two rows (normal and bright) and four role chips: Base, Text, Accent and Urgent. A theme that defines only named colors shows those instead of filling in ANSI slots. Hover or focus a swatch to identify it; click it or press Enter to copy its hex value. The palette comes from the report, not from reading the theme's files.

A "How this grade works" row at the end lists the assumptions, the grade caps for foundational checks, and the analyzer, ruleset and Omarchy versions. When the content is taller than the panel, a scrollbar shows and the bottom edge fades, so a cut-off card reads as scrollable.

## Agent actions

Once a report has loaded, a row under the verdict offers it to your default agent. The buttons use the agent's name from `~/.config/omarchy/defaults/agent`; the examples below use Pi. The main button is tinted with the theme's accent:
- **Fix all N issues with Pi** opens the agent in a new window on every problem and caution.
- With no issues but some unmeasured checks, it reads **Ask Pi about N unmeasured**.
- On a passing theme, it reads **Review with Pi**.
- With no agent chosen, it reads **Choose an agent** and opens Omarchy's picker.

Next to it:
- **Review with Pi** appears whenever the main button does something else.
- The copy icon puts the main button's prompt on the clipboard.
- The eye icon previews that prompt at the top of the panel.

Each icon's tooltip names the prompt it acts on.

A review is a read-only write-up that goes beyond the measured checks. The agent saves it as Markdown, and it appears as an **Agent review** card at the top of the body:
- closed, the card shows its byline and the start of its Summary;
- open, it shows the whole write-up as plain text (images and HTML are dropped, and links do nothing);
- while the agent works, it reads "Waiting for Pi to save the review…";
- once the theme's files change, the byline adds "theme changed since".

After the first review, the button reads **Review again with Pi**. The panel re-reads the review each time it opens.

Opening the agent (or Omarchy's agent picker) closes the panel. Copy prompt and Preview keep it open. So does a refusal, so the reason (for example "Nothing to fix") stays readable beside the buttons for a few seconds. Expanded problem and caution rows offer **Fix with Pi** and **Copy prompt** for that one check, and unmeasured rows offer **Ask Pi**. Paused checks and notes offer nothing, since changing the theme can't fix them. The actions are disabled while a check or a handoff runs, and when the result belongs to a theme you have switched away from. Prompts are built by `theme-report prompt`, never by the panel; see `docs/installation.md`.

## Exploring findings

Areas with a problem or caution start open; the rest start closed. Click an area (or Tab to it and press Enter) to open or close it. Inside, checks are listed in this order: problems, cautions, checks not measured, then notes, each in rule order. Passed checks fold into a single "N checks passed" row that opens on request. Checks paused for the same runtime reason collapse into one row that lists them.

Each check row shows a status glyph, its name and a one-line result (for example `1.37:1 contrast · target 4.5:1`). Click it for:

- text and background previews, with a contrast bar that marks the target (indicator and border checks show a border sample instead of text);
- normal and selected or focused swatches, with an OKLab separation bar;
- the four resolved core colors;
- wallpaper sample grids rebuilt from the 64 colors measured per image (coarse samples, not previews);
- context, source files and keys, and what to change. **Technical data** reveals the exact evidence JSON.

Preview hex values are rounded; grades use the unrounded measurements. Bar lengths show contrast or separation, not scores. A wallpaper deficit already charged under Text & contrast appears in Notes, not as a pass.

Open areas and expanded checks stay open when the same theme is re-checked; a different theme resets them. Arrow and Page keys scroll; Escape closes. Area headers and check rows expose accessible names with their grade, summary and open state.

## Colors

The panel follows the live popup colors, transparency and fonts. Cautions use the theme's own yellow (`yellow`, `color3` or `color11`) when it reaches 3:1 against the popup background. Otherwise they use a blend of urgent toward the text color, so a caution never looks like a pass even when accent equals text color. Evidence swatches keep their measured RGB values. A very low-contrast theme can make the panel itself harder to read; CLI and JSON output stay available.

## Data contract

The panel reads report schema v2: the optional evidence fields (`foreground`, `effectiveBackground`, `normalColor`, `stateColor`, `resolvedColors`, `paletteColors`, thumbnail `sampleColors`) and the optional top-level `runtimeContract`. Wallpapers themselves never reach the panel, only the measured colors.

## Tests

`python3 tests/panel.py` runs `tests/qml/report-smoke.qml` against a sandboxed copy of Omarchy's `Ui/` and `Commons/` modules. It covers theme colors, popup transparency, fonts, per-area checks, counts and summaries, which areas open by default, wording for failed and unmeasured checks, refresh behavior, the caution color, paused-check grouping and notice, header errors, and the agent actions (labels, signals, disabled states, preview), then lints the QML. `python3 tests/refresh.py` drives `Main.qml` with real Quickshell processes. It checks that nothing runs at startup or on file changes, how many times the analyzer runs, manual Refresh, cancellation on close, the shared cached result, and the stale badge after a theme switch.
