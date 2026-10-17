# Supported runtime

Profile: `quattro-shell-2026-v1`. This is an explicit, conservative contract, not support for all Omarchy 4 builds.

Read-only inspection covered host `4.0.0.r6720.g8e02fc8-1` and VM `4.0.4-1`. Current audits match exact palette singleton (`ShellColor.qml` when present, otherwise `Color.qml`), `Style.qml`, shell-template, and palette-fallback hashes in `src/inputs.rs`; unknown combinations produce unassessed checks. v0.4.3 adds the reviewed Qt 6.12 variants: `Commons.Color`-qualified references in the installed runtime and the upstream `ShellColor` rename at commit `eeff52bc`. Inspection confirmed only namespace/reference/comment changes to Style, with unchanged color/state defaults; the palette singleton content, template, palette fallback script, and geometry are unchanged. The profile is pinned to these inspected contracts. Geometry source is fingerprinted; native gradients are sampled across stops/segments, never scored as a flattened color; unsupported formats remain limitations.

v0.5.2 additionally recognizes the exact installed contract in `omarchy-dev 4.0.0.r6866.g77bfaca-2`. Read-only comparison against `eeff52bc` and the package sources confirmed:

- `Color.qml` removes the image-picker scrim; foundational palette loading and all assessed surface defaults are unchanged. Image-picker scrim contrast is not a scored measurement.
- `Style.qml` uses `Commons.Color` references and adds `[bar] notch-height`; assessed control-state defaults are unchanged.
- `shell.toml.tpl` removes the image-picker scrim keys. Current audits consume materialized theme files, not template rendering.
- Palette fallback script and border geometry are unchanged.

The new palette/style/template/fallback combination is pinned as one complete tuple, not independent allowlists. Unknown files and old/new mixed tuples remain unsupported. The analyzer/plugin version advances to 0.5.2 so cached unsupported results are invalidated; schema v2 and ruleset 5 remain unchanged. Building this source does not update an installed helper or plugin.

Sources: installed `/usr/share/omarchy/shell/Commons/Color.qml`, `Style.qml`, `BorderGeometry.js`; `/usr/share/omarchy/default/themed/shell.toml.tpl`; `/usr/share/omarchy/shell/Ui/Panel.qml`, `KeyboardPanel.qml`, `WidgetButton.qml`; first-party monitor/tailscale plugin manifests and panel implementations; the theme-set and hook runner commands.

## Resolution

- Runtime-root selection is explicit `--omarchy-root`, then nonempty `OMARCHY_PATH`, then `/usr/share/omarchy`. This keeps development-linked sessions from validating the unrelated packaged runtime.
- Active theme and name come from `$HOME/.local/state/omarchy/current`, matching the inspected shell rather than assuming it honors `XDG_STATE_HOME`.
- Palette is materialized `colors.toml`. Fundamental explicit roles and `color0/color7/color4` aliases are recognized. Missing foundational context is not guessed from the shell's retained previous-theme state.
- Theme `shell.toml` is parsed using the runtime's line-oriented subset. `$HOME/.config/omarchy/shell.toml` is layered above it. Absent shell keys use the inspected Color/Style runtime fallbacks, **not** the generated template values.
- Quoted scalar values, numeric values, role references, and numeric width lists are recognized. Unknown sections/keys and ignored lines are exposed as unsupported information; they do not automatically fail validity. Known invalid alpha/dimension domains and unresolved template expressions fail their owning validity check.
- Scored solid colors are opaque six-digit hex or resolvable role aliases. Native popup-border gradients resolve aliases and sample 2–8 supported stops with an optional finite angle. Native RGB/RGBA/hex border stops retain their alpha and are composited/sampled under restored ruleset 5. Embedded alpha in other solid surface roles, arbitrary Qt names, reference cycles, and unsupported surface values leave dependent measurements unassessed.
- Path audits require the explicit profile and an already materialized directory. They neither render templates nor apply stock/user overlays. Machine-level overrides apply only to current audits.

## Plugin and hook

The verified plugin uses `manifest.json`, `kinds: ["bar-widget"]`, and `entryPoints.barWidget`. Its entry component extends `qs.Ui.Panel`, owns a `WidgetButton` and a `KeyboardPanel`, and receives `bar`, `moduleName`, and `settings`. Shared imports are supplied by the native plugin host. The badge inherits the bar foreground/background/transparency without an emergency skin; the report panel follows live popup colors/alpha and shared font/control tokens. The current analyzer report schema is v2 (the native plugin manifest schema remains v1).

The hook is a script in `~/.config/omarchy/hooks/theme-set.d/`, receiving the theme slug as `$1`. It launches the same analyzer asynchronously. Theme application is not coupled to report completion.

No durable completed-generation token was established for all supported builds. Capture consistency uses content manifests around capture and before publication. This is best-effort detection, not an atomic compositor snapshot. The UI invalidates on watched changes and revalidates through the helper; it never claims freshness from the slug alone.

## Not implemented as runtime measurements

No Lua execution or syntax certification; no QML evaluation; no application-content capture; no gradient/blur simulation; no animation analysis; no display transforms. Lua syntax gets a zero-budget advisory. Wallpaper output explicitly identifies a fixed unblurred stress model, not the rendered lock/launcher.

The initial ruleset is experimental. Its perceptual thresholds are transparent engineering policy with synthetic boundary tests, not scientifically validated universal thresholds. Broader real-theme calibration is a release gate, not implied by successful compilation.
