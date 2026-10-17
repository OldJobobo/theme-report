# Supported runtime

Profile: `quattro-shell-2026-v1`.

## Runtime verification

Current audits mirror five pieces of Omarchy runtime behavior. Each is verified separately from a normalized extract of only the code and defaults it depends on. Whitespace and comments are ignored; whole named functions and the relevant declaration lines are kept.

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

Status is `reviewed` when the files exactly match a reviewed tuple (the original whole-file pins in `src/contract.rs`), `compatible` when every extract matches a reviewed extract, `partial` when palette loading is verified but something else is not, and `unsupported` otherwise. Checks whose group is unverified are unassessed with the reason "Paused: Omarchy changed how this is rendered". Nothing is guessed. If palette loading is unverified, every check pauses so a grade never rests on parse/decode checks alone.

Reviewed runtimes: Omarchy `eeff52bc` (Qt 6.12 `ShellColor` rename) and `omarchy-dev 4.0.0.r6866.g77bfaca` are recorded as exact tuples and extracts. `4.0.0.r7017.g84cfb93` verifies as `compatible`: its `Style.qml` change adds bar icon tokens outside every extract.

`theme-report runtime [--omarchy-root DIR]` prints the status, which behaviors are verified, and the extract hashes. To review a new Omarchy build: inspect the changed extract's diff against the last reviewed revision, confirm the analyzer's model still matches, then add the printed hash to the matching `KNOWN_*` list in `src/contract.rs`.

Sources consulted for the profile: `shell/Commons/Color.qml`, `Style.qml`, `BorderGeometry.js`; `default/themed/shell.toml.tpl`; `shell/Ui/Panel.qml`, `KeyboardPanel.qml`, `WidgetButton.qml`; first-party plugin manifests; the theme-set and hook runner commands. `BorderGeometry.js` is fingerprinted for caching but not gated.

## Resolution

- Runtime-root selection is explicit `--omarchy-root`, then nonempty `OMARCHY_PATH`, then `/usr/share/omarchy`. This keeps development-linked sessions from validating the unrelated packaged runtime.
- Active theme and name come from `$HOME/.local/state/omarchy/current`, matching the inspected shell rather than assuming it honors `XDG_STATE_HOME`.
- Palette is materialized `colors.toml`. Fundamental explicit roles and `color0/color7/color4` aliases are recognized. Missing foundational context is not guessed from the shell's retained previous-theme state.
- Theme `shell.toml` is parsed using the runtime's line-oriented subset. `$HOME/.config/omarchy/shell.toml` is layered above it. Absent shell keys use the inspected Color/Style runtime fallbacks, **not** the generated template values.
- Quoted scalar values, numeric values, role references, and numeric width lists are recognized. Unknown sections/keys and ignored lines are exposed as unsupported information; they do not automatically fail validity. Known invalid alpha/dimension domains and unresolved template expressions fail their owning validity check.
- Scored solid colors are opaque six-digit hex or resolvable role aliases. Native popup-border gradients resolve aliases and sample 2–8 supported stops with an optional finite angle. Native RGB/RGBA/hex border stops retain their alpha and are composited/sampled under restored ruleset 5. Embedded alpha in other solid surface roles, arbitrary Qt names, reference cycles, and unsupported surface values leave dependent measurements unassessed.
- Path audits require the explicit profile and an already materialized directory. They neither render templates nor apply stock/user overlays. Machine-level overrides apply only to current audits.

## Plugin and hook

The verified plugin uses `manifest.json`, `kinds: ["bar-widget"]`, and `entryPoints.barWidget`. It sets `manageIpc: false` and declares its own `ShellIpc`, enabled only on the first copy listed by `bar.moduleWidgets()` (the same list on every monitor). Panel commands go through the plugin shell facade's `summon`, which uses the bar's focused-monitor `findPanelWidget`. IPC methods return strings: this runtime's qmllint aborts on `: void` annotations inside an `IpcHandler`. Its entry component extends `qs.Ui.Panel`, owns a `WidgetButton` and a `KeyboardPanel`, and receives `bar`, `moduleName`, and `settings`. Shared imports are supplied by the native plugin host. The badge inherits the bar foreground/background/transparency without an emergency skin; the report panel follows live popup colors/alpha and shared font/control tokens. The current analyzer report schema is v2 (the native plugin manifest schema remains v1).

The hook is a script in `~/.config/omarchy/hooks/theme-set.d/`, receiving the theme slug as `$1`. It launches the same analyzer asynchronously. Theme application is not coupled to report completion. The badge picks up the hook's published result.

No durable completed-generation token was established for all supported builds. Capture consistency uses metadata manifests (size, times, inode, link target) taken before reading and re-checked after capture and after analysis. This is best-effort detection, not an atomic compositor snapshot. The badge marks a result whose theme name differs from `theme.name` as stale (`~`); it never audits on its own.

## Not implemented as runtime measurements

No Lua execution or syntax certification; no QML evaluation (runtime behavior is verified from source extracts, never executed); no application-content capture; no gradient/blur simulation; no animation analysis; no display transforms. Lua syntax gets a zero-budget advisory. Wallpaper output explicitly identifies a fixed unblurred stress model, not the rendered lock/launcher.

The initial ruleset is experimental. Its perceptual thresholds are transparent engineering policy with synthetic boundary tests, not scientifically validated universal thresholds. Broader real-theme calibration is a release gate, not implied by successful compilation.
