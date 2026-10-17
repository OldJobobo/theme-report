# Changelog

## 0.8.0

Hand the report to your Omarchy default agent.

- **Panel.** "Fix N issues with Pi" (using your chosen agent) opens the agent in a new window with a prompt built from the measurements. Expanded checks can fix just that check, unmeasured checks can be asked about, and "Copy prompt" and "Preview" show the exact text instead of launching. With no agent chosen, the button opens Omarchy's picker. The panel closes once the agent window or picker opens.
- **CLI.** `theme-report prompt <fix|measure|explain> [--rule <id>]` prints the prompt; `theme-report agent <kind> [--rule <id>] [--copy]` launches or copies it.
- **IPC.** `askAgent <kind>`, `fixRule <ruleId>` and `copyPrompt <kind>`; `status` reports `agent` and `agentMessage`.
- **Limits in every prompt.** The agent edits only the theme source: in place on a new branch when the theme is in a git repository, otherwise in a `<name>-tuned` copy, and stock themes are always copied. It never touches wallpapers or the applied theme, states its plan first, and asks before applying. Theme-controlled text stays in a private handoff file the prompt marks as data.
- **Agent skill.** `install.sh --with-skill` installs a `theme-report` skill into the agent skill folders that exist.
- **Agent review.** "Review with Pi" (offered on every theme, including passing ones) asks for a thorough read-only review beyond the measured checks. The agent saves a Markdown write-up, and the panel shows it as an "Agent review" card, pending while the agent works and marked out of date once the theme changes. `theme-report prompt|agent review` and IPC `askAgent review` do the same.
- **Header.** The agent row is always shown. Its main button is tinted with the theme's accent and reads "Fix all N issues", "Ask about N unmeasured" or "Review". Copy and Preview are icons whose tooltips name the prompt they act on.
- **Personal design skills.** List your own skills in `~/.config/theme-report/agent.toml` and prompts point the agent at them by name and path. Theme Report itself ships no design guidance.

## 0.7.0

Full IPC support on `oldjobobo.theme-report`.

- One widget copy owns the target instead of every monitor's copy claiming it. That removes the duplicate-handler warnings, and calls always reach a live copy.
- `open`/`show`, `close`/`hide` and `toggle` act on the focused monitor through the shell's own routing, and close the panel wherever it is shown.
- New: `openCategory <id>`, `check` and `refresh` (a background re-check that updates every badge without opening the panel), `status` (JSON) and `grade` (badge text).
- Every method answers with a string, and a busy check answers `busy` immediately.

Also in 0.7.0, a report card redesign of the panel. No change to the report data or grading.

- Each area is a card with its letter grade, a score meter and a one-line summary. Its checks are listed inline, so clicking an area no longer filters a list below the fold. Areas with problems or cautions open on their own, and passed checks fold into one row.
- The Issues / Passed / Limitations / Notes tabs are gone. "Limitations" now reads "not measured".
- The header has a grade stamp colored by grade, count chips, the last check time and a spinning Refresh icon. Errors and paused-check notices show as callouts.
- The palette and "How this grade works" move below the grades. A scrollbar and a faded bottom edge show when there is more to scroll.
- Fix: a failing integrity check (such as missing core colors) no longer shows the passing description. It names what is missing instead.

## 0.6.0

Real-world coverage and reliability. Schema v2 and ruleset 5 scoring are unchanged; caches from older versions are ignored.

- **Runtime verification per behavior.** Omarchy runtime files are no longer pinned by whole-file hashes. Palette loading, surface defaults, control states, template defaults and selection fallbacks are each verified from a normalized extract, so unrelated Omarchy edits (such as r7017's bar-icon tokens) no longer disable every check. Unverified behaviors pause only the checks that depend on them. Reports carry `runtimeContract`; `theme-report runtime` explains the status.
- **No more aborted audits from theme layout.** Subfolders in `backgrounds/` (such as stock `intros/`), notes like `README.md`, and dotfiles are ignored. Up to 32 wallpapers are sampled, and extra ones become a limitation. A wallpaper symlink that leaves the theme folder is reported, not read, instead of failing the audit.
- **WebP wallpapers** are decoded (static; animated WebP is a limitation).
- **No leaked scratch files.** Media copies live in a per-run directory that is removed on exit and on termination (closing the panel). Decoders die with the analyzer. Files leaked by older versions are swept.
- **Faster.** Cache hits are decided from file metadata without reading wallpapers. Inputs are read once per audit instead of two or three times, decoder version probes run once per process, and retries back off while a theme switch settles.
- **Panel.** Cautions get their own readable hue, quick refreshes no longer flash "Previous result", errors and paused-check notices sit under the verdict, identical paused checks collapse into one card, theme names are title-cased, and rows have accessible names.
- **Badge.** After a shell restart it shows the last result instead of `?`. It marks results for a previous theme as `~B`, and it picks up results from the optional theme-switch hook.
- **Findings.** Passing checks no longer carry fix-it advice, sources are not repeated, and absent `shell.toml` files are not cited. Invalid values in your own `~/.config/omarchy/shell.toml` become an `advisory.user-shell` note instead of failing the theme.
- **Install.** Interrupted updates can be re-run or uninstalled. Updates recommend a clean shell restart. Uninstall disables the widget before removing files, and the installer checks that the binary runs on this machine.

## 0.5.2

Recognized the exact runtime files of `omarchy-dev 4.0.0.r6866` (image-picker scrim removal, bar notch height).

## 0.5.1

Restored ruleset 5 scoring and category letters after the rejected 0.5.0 experiment that made semantic/shell/wallpaper categories unscored guidance.

## 0.4.x

Open-only audits with an upper-right Refresh control; no startup audit, timers or watchers (0.4.5). Overview color palette with copy-to-clipboard (0.4.4). Qt 6.12 `ShellColor` runtime variants (0.4.3). Presentation evidence for passing checks: color previews, threshold tracks, wallpaper sample grids (0.4.1).

## 0.1–0.3

Initial analyzer, native bar widget and report panel; color-managed thumbnails, video wallpaper sampling, and modeled translucent-surface stress tests.
