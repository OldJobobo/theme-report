# Supported runtime

Profile: `quattro-shell-2026-v1`.

## Runtime verification

Current-theme audits copy five pieces of Omarchy's behavior. The analyzer verifies each one separately against your installed Omarchy, by hashing an extract of only the code and defaults it depends on: whole named functions and the relevant declaration lines, with whitespace and comments ignored.

| Behavior | Source | Extract |
| --- | --- | --- |
| Palette loading | `Color.qml` / `ShellColor.qml` | `loadColors` |
| Surface defaults | `Color.qml` / `ShellColor.qml` | `pick`, `pickAlpha`, `flatColor`, `composed` and every `root.composed(...)` / `root.pick(...)` declaration |
| Control states | `Style.qml` | style token helpers, `*StateColor`, `*FillFor`, and `styleAlpha/String/Num("…")` declarations |
| Template defaults | `default/themed/shell.toml.tpl` | literal values (template expressions excluded) |
| Selection fallbacks | `bin/omarchy-theme-color` | `[[ … ]] ||` fallback assignments |

Rule groups and what they need:

- **Palette** (`validity.*`, `readability.foundation`, `semantic.accent`, `semantic.error-indicator`): palette loading.
- **Selection** (`readability.selection`, `semantic.selection`): palette loading and selection fallbacks.
- **Shell** (all other scored rules): palette loading, surface defaults, control states and template defaults.

The status is:

- `reviewed` when the files exactly match a reviewed set of files (the whole-file hashes in `src/contract.rs`);
- `compatible` when every extract matches a reviewed extract;
- `partial` when palette loading is verified but something else is not;
- `unsupported` otherwise.

Checks in an unverified group are unassessed, with the reason "Paused: Omarchy changed how this is rendered". Nothing is guessed. If palette loading is unverified, every check pauses, so a grade never rests on the file-parsing checks alone.

Reviewed builds: Omarchy `eeff52bc` (the Qt 6.12 `ShellColor` rename) and `omarchy-dev 4.0.0.r6866.g77bfaca` are recorded as exact file sets and extracts. `4.0.0.r7017.g84cfb93` verifies as `compatible`: its `Style.qml` change adds bar icon tokens outside every extract.

`theme-report runtime [--omarchy-root DIR]` prints the status, which behaviors are verified, and the extract hashes. To review a new Omarchy build: inspect the changed extract's diff against the last reviewed revision, confirm the analyzer's model still matches, then add the printed hash to the matching `KNOWN_*` list in `src/contract.rs`.

The profile was written from these Omarchy sources: `shell/Commons/Color.qml`, `Style.qml`, `BorderGeometry.js`; `default/themed/shell.toml.tpl`; `shell/Ui/Panel.qml`, `KeyboardPanel.qml`, `WidgetButton.qml`; first-party plugin manifests; the theme-set and hook runner commands. `BorderGeometry.js` is fingerprinted for caching but not gated.

## Resolution

- The Omarchy install is `--omarchy-root` if given, then a nonempty `OMARCHY_PATH`, then `/usr/share/omarchy`. This way a session running a development checkout of Omarchy isn't checked against the packaged copy.
- The active theme and its name come from `$HOME/.local/state/omarchy/current`, as in the shell itself, which does not use `XDG_STATE_HOME`.
- The palette is the applied theme's `colors.toml`. The core roles and the `color0`, `color7` and `color4` aliases are recognized. Missing core colors are not filled in from whatever the shell still holds from the previous theme.
- The theme's `shell.toml` is parsed the way the shell parses it, line by line. `$HOME/.config/omarchy/shell.toml` is layered on top. Missing shell keys take the shell's own fallbacks from `Color.qml` and `Style.qml`, **not** the values in the generated template.
- Quoted values, numbers, role references and lists of numeric widths are recognized. Unknown sections and keys, and lines the shell ignores, are reported as information; they don't fail validity by themselves. Out-of-range alpha and size values, and unresolved template expressions, fail the validity check that owns them.
- Graded solid colors must be opaque six-digit hex or role aliases that resolve. Popup-border gradients resolve aliases and sample 2–8 supported stops with an optional finite angle. RGB, RGBA and hex border stops keep their alpha when composited and sampled. Alpha embedded in other solid surface colors, arbitrary Qt color names, reference cycles and unsupported surface values leave the dependent measurements unassessed.
- Path audits need `--profile` and a theme folder that has already been generated. They don't render templates or apply the stock and user overlays. Machine-level overrides apply only to current-theme audits.

## Plugin and hook

The plugin uses `manifest.json`, `kinds: ["bar-widget"]`, and `entryPoints.barWidget`. It sets `manageIpc: false` and declares its own standard Quickshell `IpcHandler`, enabled only on the first copy listed by `bar.moduleWidgets()` (the same list on every monitor). On newer shells it also registers and unregisters that handler with the optional `qs.Commons.IpcRegistry` for socket-based IPC. It does not require the newer `ShellIpc` type; stable Omarchy 4.0.2 uses native Quickshell IPC. Panel commands go through the plugin shell facade's `summon`, which uses the bar's focused-monitor `findPanelWidget`. IPC methods return strings, because this Quickshell's qmllint aborts on `: void` annotations inside an `IpcHandler`. Its entry component extends `qs.Ui.Panel`, owns a `WidgetButton` and a `KeyboardPanel`, and receives `bar`, `moduleName`, and `settings`. The plugin host supplies the shared imports. The badge uses the bar's foreground, background and transparency, with no fallback styling; the report panel follows the live popup colors and transparency and the shared font and control styles. The analyzer's report schema is v2; the plugin manifest schema is v1.

The hook is a script in `~/.config/omarchy/hooks/theme-set.d/`, receiving the theme slug as `$1`. It starts the analyzer in the background, so the theme switch doesn't wait for the check. The badge picks up the result when it is saved.

Omarchy has no reliable marker that a theme switch has finished. To catch a theme changing mid-check, the analyzer records each input file's size, times, inode and link target before reading, and compares them again after reading and after analysis. This catches most changes but is not an atomic snapshot. The badge marks a result whose theme name differs from `theme.name` as stale (`~`); it never checks on its own.

## Not measured

Theme Report does not run Lua or certify its syntax, and does not evaluate QML: Omarchy's behavior is verified from source extracts, never executed. It does not capture application content, simulate blur, analyze animation or model display transforms. Lua syntax gets an advisory note that costs no points. Wallpaper results use a fixed, unblurred model and say so; they are not captures of the rendered lock screen or launcher.

The ruleset is still young. Its perceptual thresholds are documented engineering choices, tested at their boundaries with synthetic themes. They are not scientifically validated, and calibration against more real themes is ongoing.
