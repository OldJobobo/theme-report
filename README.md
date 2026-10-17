# Theme Report

A read-only Omarchy Quattro theme analyzer with a native Quickshell bar widget and report panel.

It reports validity, readability, functional color health, shell cues, and modeled wallpaper robustness. Every scored finding includes reproducible evidence. It does **not** grade aesthetics, legacy compatibility, optional-file counts, or publishing polish.

## Status

**Implemented experimental v0.5.2 (ruleset 5; reviewed r6866 runtime support).** One Rust helper, one native plugin, and an optional non-blocking theme-set hook. The compact report card shows A–F overall and category grades, clear fixes, and plain-language limitations—not competing score/coverage percentages. Bounded color-managed thumbnails cover large/profiled wallpapers; explicitly labeled reference-background stress tests grade translucent shell surfaces and native border gradients without claiming live-pixel certification.

Checks performed determine the grade. Missing measurements make it **Provisional**; partial categories are **Limited**, and unmeasured categories are **Not checked**. The minimal native fixture earns a provisional A without authored overrides. An A is not a universal accessibility guarantee.

SDR video wallpapers (MP4/M4V/MOV, WebM/MKV, AVI) use bounded, deterministic sampling through system FFmpeg. Known wallpaper-backed lock surfaces reuse these samples for contrast; variable application-backed transparency, unsupported color profiles, and Lua certification remain explicitly bounded models or limitations. The badge inherits native bar styling; the report panel follows live popup colors, transparency, typography, and control states.

![Native report panel](evidence/ui-overview.png)

Healthy checks are inspectable too: select a category or **Passed**, then expand a check for color previews, threshold markers, state comparisons and sampled wallpaper grids. Exact evidence stays behind **Technical data**. Checks run only when you open the panel or use its upper-right Refresh icon—no startup audit, stale timer, or file watchers. See `docs/report-ui.md`.

## Build and run

```sh
cargo build --release --locked
cargo test --locked
./target/release/theme-report audit --current
./target/release/theme-report audit --current --format json
```

Video analysis and video tests require `ffmpeg` and `ffprobe` on PATH. Static PNG/JPEG analysis has no external decoder dependency. Missing video tools create limitations rather than inventing a failure.

To keep host inspection/cache writes repository-local:

```sh
./target/release/theme-report audit --current --cache-dir "$PWD/.testbed/cache"
```

Preview installation with `bash scripts/install.sh --dry-run`. Installation is opt-in; building or auditing never changes theme files or enables the widget.

## Documentation

- `docs/installation.md`: installation, CLI, panel controls, and uninstall.
- `docs/supported-runtime.md`: verified runtime profiles and resolution.
- `rules/v1.md`: budgets, thresholds, provisional grading, and deduction ownership.
- `docs/measurement.md`: formulas, sampling, bounds, and determinism.
- `schema/report-v2.schema.json`: current machine-readable report contract (schema v1 is historical).
- `docs/verification.md`: test results, VM workflow, visual evidence, and remaining release gates.
- `PROPOSAL.md` and `PLAN.md`: product rationale and build sequence.

MIT licensed. Synthetic fixtures are project-owned; native shell components are consumed from the user's Omarchy installation, not vendored.
