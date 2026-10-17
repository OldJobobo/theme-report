# Installation and use

## Build

Use the checked-in lockfile. Tested with Rust 1.97.1 on x86_64 Linux.

```sh
cargo build --release --locked
cargo test --locked
```

Development in a repository-only environment can use `CARGO_HOME="$PWD/.build/cargo"`. Tests create their temporary data under `.testbed/`, never the live theme directory.

The binary uses the system's libc, libm, and libgcc; it is not claimed to be fully static. Ordinary bounded PNG/JPEG decoding runs in a child mode of the same executable, without Python, Node, or Lua. Color-profiled/high-resolution thumbnails require system ImageMagick with LCMS (`magick`, or ImageMagick 6 `convert`). Its decoder-assisted JPEG reduction is also used for oversized encoded JPEGs when available. A small canonical sRGB profile is embedded in the binary; no system ICC profile is required. Missing tools/resource exhaustion remain limitations, not fabricated grades. Video analysis requires system `ffmpeg` and `ffprobe`; video tests also require them. Missing video tools leave video-dependent checks unassessed. No FFmpeg libraries or executable are bundled.

## Install

Preview first, then install only when you intend to modify your own user installation:

```sh
bash scripts/install.sh --dry-run
bash scripts/install.sh
omarchy-shell shell rescanPlugins
omarchy plugin enable oldjobobo.theme-report
```

For a release archive, supply `--binary bin/theme-report`. The default installation has no background audits. Open the panel to check your theme; click the upper-right Refresh icon or press R to force a fresh audit. Closing the panel cancels an in-flight check. `--with-hook` is an explicit opt-in to separate, non-blocking theme-set audits; leave it off for open-only behavior.

Install paths are `~/.local/bin/theme-report`, the project plugin directory under `~/.config/omarchy/plugins/`, and optionally its script in `~/.config/omarchy/hooks/theme-set.d/`. Installation never rewrites the general shell layout automatically. Updates reject unrelated or user-modified destination files. `--root DIR` stages/testing installs into an explicit root.

## CLI

```sh
theme-report audit --current
theme-report audit --current --format json
theme-report audit --current --force
theme-report audit /path/to/materialized-theme --profile quattro-shell-2026-v1
theme-report explain readability.foundation
theme-report rules
theme-report version
```

Path audits require already materialized input and never publish to the active cache. `--cache-dir DIR` isolates cache/staging; `--omarchy-root DIR` changes current-audit contract discovery, not theme files. Without that flag, the helper uses `OMARCHY_PATH` from its session environment, falling back to `/usr/share/omarchy`. Unknown runtime contract hashes produce incomplete results.

Human output shows overall/category letters, provisional/limited states, and findings. JSON contains evidence, logical source files/keys, model assumptions, and version/fingerprint metadata. Successful audits exit 0 even when theme defects are found; operational failures exit 1 with a JSON error envelope in JSON mode. Argument-parser failures exit 2. An error envelope is not a scored report and does not replace the last successful cache.

## Interpreting the badge

A–F shows the last checked result; `A*` (or another letter with `*`) means Provisional, explained in the tooltip and panel. `?` means no graded result has been obtained. Unmeasured categories show Not checked; partial categories show Limited. All five categories retain their original scoring under restored ruleset 5. No color alone communicates status. The badge inherits Omarchy's bar styling without forcing its own background.

The widget does not audit at startup, watch theme/cache files, expire results, poll, or queue background checks. Each panel instance audits only when opened or explicitly refreshed. Opening may reuse a matching analyzer cache; Refresh bypasses it. The last checked letter remains on the badge until the next check; its tooltip labels it as the last result, not a continuously monitored verdict. Last successful evidence remains available during refresh/error; it is never relabeled as a new theme's report.

The report follows the live theme's popup background/text, authored transparency, accent/urgent colors, font, and native control states. Cards use theme-derived fills; color evidence keeps its measured values. There is no independent emergency palette, so a low-contrast theme may also reduce the panel's readability; CLI/JSON output remains available. The verdict and upper-right Refresh control remain visible while the body scrolls. The letter grades the checks performed, with provisional context explained beneath the verdict when measurements are missing; category rows show letter badges and short Limited / Not checked tags rather than paragraphs or percentages. Problems and cautions retain their original deductions. Detailed limitation reasons stay in the Limitations view and expandable findings.

Click a category to filter findings. Issues, Limitations, and Notes are separate views. Expand a finding with click or keyboard Tab + Enter for remediation, source references, and measurement evidence. Method & limits contains assumptions and foundational cap details. Arrow/Page keys scroll; Escape closes. Refresh is the only report-level action; palette color copying and per-finding technical evidence remain available.

## Uninstall

```sh
omarchy plugin disable oldjobobo.theme-report
bash scripts/uninstall.sh --dry-run
bash scripts/uninstall.sh
```

Uninstall removes only unchanged, receipt-owned files. Modified files are preserved with a diagnostic. Cache, user settings, and unrelated shell layout remain intact. You may explicitly remove your cache separately if desired.

## Troubleshooting

- Missing helper: verify `~/.local/bin/theme-report`, then Refresh.
- Widget not found: rescan plugins, validate the plugin folder, then enable it. Live QML replacement can trigger native Quickshell teardown crashes on the tested runtime. For plugin-code updates in an unlocked session, stop the matching shell with `quickshell kill -p "$OMARCHY_PATH/shell" --any-display`, install all files, then use `omarchy restart shell` instead of replacing live QML repeatedly. Never stop an active lock client. A normal `omarchy restart shell` clears retained compiled components.
- Provisional / Not graded: inspect category states and expanded limitation reasons. Animated/unsupported media, unavailable color-managed decoding, and unknown runtime contracts remain honest limitations. Translucent application-backed surfaces are graded using explicitly labeled background stress scenarios, not silently treated as opaque or live-pixel measurements.
- Busy/unstable: another audit or theme mutation is in progress; retry after it settles. Failed audits retain previous cache evidence.
- Unsupported runtime: compare the installed contracts with `docs/supported-runtime.md`; do not force an unknown current profile into a guessed grade.

See `rules/v1.md` and `docs/measurement.md` for exact policy and limitations. The ruleset is experimental; A is not a universal accessibility certification.
