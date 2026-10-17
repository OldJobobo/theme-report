# Installation and use

## Build

Use the checked-in lockfile. Tested with Rust 1.97.1 on x86_64 Linux.

```sh
cargo build --release --locked
cargo test --locked
```

Development in a repository-only environment can use `CARGO_HOME="$PWD/.build/cargo"`. Tests create their temporary data under `.testbed/`, never the live theme directory.

The binary uses the system's libc, libm, and libgcc; it is not claimed to be fully static. Ordinary bounded PNG/JPEG/WebP decoding runs in a child mode of the same executable, without Python, Node, or Lua. Color-profiled/high-resolution thumbnails require system ImageMagick with LCMS (`magick`, or ImageMagick 6 `convert`). Its decoder-assisted JPEG reduction is also used for oversized encoded JPEGs when available. A small canonical sRGB profile is embedded in the binary; no system ICC profile is required. Missing tools/resource exhaustion remain limitations, not fabricated grades. Video analysis requires system `ffmpeg` and `ffprobe`; video tests also require them. Missing video tools leave video-dependent checks unassessed. No FFmpeg libraries or executable are bundled.

## Install

Preview first, then install only when you intend to modify your own user installation:

```sh
bash scripts/install.sh --dry-run
bash scripts/install.sh
omarchy-shell shell rescanPlugins
omarchy plugin enable oldjobobo.theme-report
```

For a release archive, supply `--binary bin/theme-report`. The installer first checks that the binary runs on this machine. The default installation has no background audits. Open the panel to check your theme; click the upper-right Refresh icon or press R to force a fresh audit. Closing the panel cancels an in-flight check. `--with-hook` is an explicit opt-in to separate, non-blocking theme-set audits whose results the badge picks up; leave it off for open-only behavior.

Updating an existing install prints `omarchy restart shell`: replacing plugin QML under a running shell can crash Quickshell, so load new plugin code with a clean restart.

Install paths are `~/.local/bin/theme-report`, the project plugin directory under `~/.config/omarchy/plugins/`, and optionally its script in `~/.config/omarchy/hooks/theme-set.d/`. Installation never rewrites the general shell layout automatically. Updates reject unrelated or user-modified destination files. The ownership receipt lists both old and new file hashes until an update finishes, so an interrupted update can simply be re-run (or uninstalled). `--root DIR` stages/testing installs into an explicit root.

## CLI

```sh
theme-report audit --current
theme-report audit --current --format json
theme-report audit --current --force
theme-report audit /path/to/materialized-theme --profile quattro-shell-2026-v1
theme-report runtime
theme-report explain readability.foundation
theme-report rules
theme-report version
```

Path audits require already materialized input and never publish to the active cache. `--cache-dir DIR` isolates cache/staging; `--omarchy-root DIR` changes current-audit contract discovery, not theme files. Without that flag, the helper uses `OMARCHY_PATH` from its session environment, falling back to `/usr/share/omarchy`. If the installed Omarchy build changed a behavior the analyzer mirrors, only the dependent checks are paused; `theme-report runtime` shows which (see `docs/supported-runtime.md`).

Human output shows overall/category letters, provisional/limited states, and findings. JSON contains evidence, logical source files/keys, model assumptions, version/fingerprint metadata and, for current audits, `runtimeContract`. Successful audits exit 0 even when theme defects are found; operational failures exit 1 with a JSON error envelope in JSON mode. Argument-parser failures exit 2. An error envelope is not a scored report and does not replace the last successful cache.

## Interpreting the badge

A–F shows the last checked result; `A*` (or another letter with `*`) means Provisional, explained in the tooltip and panel. `~A` means the result belongs to the theme you had before switching; open the panel to check the new one. `?` means no graded result has been obtained. Unmeasured categories show Not checked; partial categories show Limited. All five categories retain their original scoring under restored ruleset 5. No color alone communicates status. The badge inherits Omarchy's bar styling without forcing its own background.

The widget does not audit at startup, expire results, poll, or queue background checks. Each panel instance audits only when opened or explicitly refreshed. It does read the last published result at startup and when the cache file changes, and watches `theme.name` to mark a result stale. Those are file reads, never audits. Opening may reuse a matching analyzer cache (decided from file metadata, without re-reading wallpapers); Refresh bypasses it. The tooltip labels the badge as the last result, not a continuously monitored verdict. Last successful evidence remains available during refresh/error; it is never relabeled as a new theme's report.

The report follows the live theme's popup background/text, authored transparency, accent/urgent colors, font, and native control states. Cards use theme-derived fills; color evidence keeps its measured values. There is no independent emergency palette, so a low-contrast theme may also reduce the panel's readability; CLI/JSON output remains available. The verdict and upper-right Refresh control remain visible while the body scrolls. The letter grades the checks performed, with provisional context explained beneath the verdict when measurements are missing; each area card shows a letter grade, a score meter and a one-line count summary. Problems and cautions retain their original deductions. Detailed reasons for checks that could not be measured stay in each check's row and expanded details. Analyzer errors and paused-check notices appear under the verdict.

Areas with problems or cautions open on their own, listing their checks inline; click an area to open or close it. Expand a check with click or keyboard Tab + Enter for remediation, source references, and measurement evidence. Passed checks fold into one row per area. How this grade works contains assumptions and foundational cap details. Arrow/Page keys scroll; Escape closes. Refresh is the only report-level action; palette color copying and per-finding technical evidence remain available.

## IPC

The widget answers on the IPC target `oldjobobo.theme-report`, through `omarchy-shell` (or `qs ipc call`):

```sh
omarchy-shell oldjobobo.theme-report toggle
omarchy-shell oldjobobo.theme-report openCategory readability
omarchy-shell oldjobobo.theme-report check
omarchy-shell oldjobobo.theme-report status | jq .
```

| Method | Answer | Effect |
| --- | --- | --- |
| `open`, `show` | `opened` | Open the panel on the focused monitor (opening checks the theme, as a click does) |
| `close`, `hide` | `closed` | Close the panel on whichever monitor shows it |
| `toggle` | `opened` / `closed` | Either of the above |
| `openCategory <id>` | `ok` / `unknown` | Open with that area's card opened and scrolled into view: `validity`, `readability`, `semantic`, `shell` or `wallpaper` |
| `check` | `started` / `busy` | Check the current theme now; unchanged inputs reuse the cache. An open panel re-checks in place; otherwise nothing opens and every badge updates |
| `refresh` | `started` / `busy` | Like `check`, but always a full re-audit |
| `status` | JSON | Last result, described below |
| `grade` | badge text | `B`, `B*`, `~B` or `?` |

`status` returns `theme`, `activeTheme`, `badge`, `grade`, `state` (`ready`, `provisional`, `not-graded` or `not-audited`), `stale`, `summary`, `problems`, `cautions`, `limitations`, `runtime` (the runtime contract status), `checkedAt` (Unix seconds), `analyzerVersion`, `checking`, `open`, and `error` (the open panel's or the last background check's error, otherwise `null`). `check` and `refresh` return immediately; poll `status` until `checking` is false to read the outcome.

The bar creates one widget copy per monitor. Exactly one copy (the first the bar lists) owns the target, so calls are never split between copies, and ownership moves if that copy goes away. `open`, `toggle` and `openCategory` use the shell's own focused-monitor routing (`omarchy-shell shell toggle oldjobobo.theme-report` reaches the same place). The target exists only while the widget is on a bar. `check` and `refresh` are explicit requests: the widget still never audits by itself in the background.

To bind a key, run the command from your Hyprland binding, for example `omarchy-shell oldjobobo.theme-report toggle`.

## Uninstall

```sh
omarchy plugin disable oldjobobo.theme-report
bash scripts/uninstall.sh --dry-run
bash scripts/uninstall.sh
```

Uninstall disables the widget first (for your own home directory, when `omarchy` is available), then removes only unchanged, receipt-owned files. Modified files are preserved with a diagnostic. Cache, user settings, and unrelated shell layout remain intact. You may explicitly remove your cache separately if desired.

## Troubleshooting

- Missing helper: verify `~/.local/bin/theme-report`, then Refresh.
- Widget not found: rescan plugins, validate the plugin folder, then enable it. Live QML replacement can trigger native Quickshell teardown crashes on the tested runtime. For plugin-code updates in an unlocked session, stop the matching shell with `quickshell kill -p "$OMARCHY_PATH/shell" --any-display`, install all files, then use `omarchy restart shell` instead of replacing live QML repeatedly. Never stop an active lock client. A normal `omarchy restart shell` clears retained compiled components.
- Provisional / Not graded: inspect category states and expanded limitation reasons. Animated/unsupported media, unavailable color-managed decoding, and unknown runtime contracts remain honest limitations. Translucent application-backed surfaces are graded using explicitly labeled background stress scenarios, not silently treated as opaque or live-pixel measurements.
- Busy/unstable: another audit or theme mutation is in progress; retry after it settles. Failed audits retain previous cache evidence.
- "Paused: Omarchy changed how this is rendered": run `theme-report runtime` to see which runtime behavior changed. Those checks resume with an analyzer release that has reviewed the change; nothing is guessed meanwhile.
- Scratch space: media copies live in `~/.cache/theme-report/run-<pid>/` only while an audit runs. They are removed on exit or cancellation, and any left by a crash are swept by the next audit.

See `rules/v1.md` and `docs/measurement.md` for exact policy and limitations. The ruleset is experimental; A is not a universal accessibility certification.
