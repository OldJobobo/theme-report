# Installation and use

## Build

Build with the checked-in lockfile. Rust 1.88 or newer is required; development uses current stable Rust on x86_64 Linux.

```sh
cargo build --release --locked
cargo test --locked
```

`bash scripts/check.sh` runs the full local check: formatting, tests, clippy, the release build, installer tests, shellcheck and, when Omarchy's Quickshell tools are installed, QML lint and panel tests. To keep cargo's downloads inside the repository, set `CARGO_HOME="$PWD/.build/cargo"`. Tests write their temporary files under `.testbed/` and never touch your live theme folder.

The binary links against the system's libc, libm and libgcc; it is not fully static. Ordinary PNG, JPEG and WebP decoding happens inside the analyzer, in a size-limited child process, with no Python, Node or Lua involved. A small sRGB profile is built into the binary, so no system ICC profile is needed.

Some inputs need system tools:

- **ImageMagick** (`magick`, or ImageMagick 6 `convert`, built with LCMS) for color-profiled and very large images, including oversized JPEGs.
- **FFmpeg** (`ffmpeg` and `ffprobe`) for video wallpapers. The video tests need them too.

Theme Report bundles neither. Without them, the affected wallpapers are reported as not measured; the analyzer never invents a grade for them.

## Install

Preview, then install into your home directory:

```sh
bash scripts/install.sh --dry-run
bash scripts/install.sh
omarchy-shell shell rescanPlugins
omarchy plugin enable oldjobobo.theme-report
```

**Plugin-manager installs.** After `omarchy plugin add`, the plugin folder is a git checkout of this repository. Build there and run `bash scripts/install.sh` from it. Set `CARGO_TARGET_DIR` to keep build files out of `~/.config`; `install.sh` looks for the binary there too. From a checkout, the installer installs only the analyzer and any hook or skill you ask for, and leaves the git-managed plugin files alone. Its ownership record is ignored by git, so `omarchy plugin update` still sees a clean checkout.

**Release archives.** Run `bash scripts/install.sh` from the unpacked archive; it uses the bundled `bin/theme-report`. Pass `--binary FILE` to use a different binary. The installer checks that the binary runs on this machine before installing anything.

Release archives are built by GitHub Actions from the tagged commit (`.github/workflows/release.yml`), never on a personal machine. The workflow builds the archive twice, from two checkouts in different folders, and refuses to publish unless the bytes match. It then installs and runs the exact archive before publishing it with a signed provenance attestation. `BUILDINFO.txt` in the archive records the commit and toolchain. To check an archive:

```sh
sha256sum -c theme-report-<version>-linux-x86_64.tar.gz.sha256
gh attestation verify theme-report-<version>-linux-x86_64.tar.gz -R OldJobobo/theme-report
```

To rebuild it yourself, check out the commit in `BUILDINFO.txt` and run `bash scripts/package.sh` on Ubuntu 24.04 with the same Rust version and the `rust-docs` component. The build strips local paths, so the archive should match byte for byte.

A source or archive install and a plugin-manager install use the same plugin folder, so remove one before switching to the other.

**What gets installed:**

- `~/.local/bin/theme-report`, the analyzer;
- the plugin files in `~/.config/omarchy/plugins/oldjobobo.theme-report/` (unless the folder is a plugin-manager checkout);
- with `--with-hook`, a script in `~/.config/omarchy/hooks/theme-set.d/` that re-checks the theme in the background after each theme switch, without slowing the switch down;
- with `--with-skill`, the agent skill (see [Agent handoff](#agent-handoff)).

Without the hook, nothing ever checks your theme in the background. The panel checks when you open it.

The installer never edits your bar layout. It keeps a record of every file it installed, with its hash, in `.theme-report-owned` in the plugin folder. It refuses to overwrite a file it didn't install or that you have changed. During an update the record lists both old and new hashes, so an interrupted update can be re-run or uninstalled. `--root DIR` installs into another directory instead of your home, which is useful for testing.

Updating plugin files prints `omarchy restart shell`. Replacing plugin QML under a running shell can crash Quickshell, so a clean restart is the safe way to load new plugin code.

## CLI

```sh
theme-report audit --current
theme-report audit --current --format json
theme-report audit --current --force
theme-report audit /path/to/theme --profile quattro-shell-2026-v1
theme-report runtime
theme-report explain readability.foundation
theme-report rules
theme-report version
```

- `audit --current` checks your active theme, using the copy Omarchy applied. `--force` skips the cache.
- `audit PATH` checks a theme folder that has already been generated (its `colors.toml` and `shell.toml` exist). It needs `--profile`, and its results never replace the badge's cached result.
- `--cache-dir DIR` uses a separate cache. `--omarchy-root DIR` points the runtime checks at another Omarchy install; it does not change which theme files are read. By default the analyzer uses `OMARCHY_PATH`, falling back to `/usr/share/omarchy`.
- `runtime` shows which parts of Omarchy's behavior this analyzer has verified for your installed build (see `docs/supported-runtime.md`). If one changed, only the checks that depend on it pause.

Text output shows the overall and per-area grades, provisional and limited states, and the findings. JSON output adds the evidence, the source files and keys behind each check, the model assumptions, version and input fingerprint, and, for current-theme audits, `runtimeContract`.

Exit codes: 0 for a completed audit, even when the theme has problems; 1 for an operational failure (in JSON mode, with a JSON error object, which is not a report and never replaces the last good result); 2 for invalid arguments.

## The badge

`A`–`F` is the last result. A `*` after the letter means provisional: some checks could not be measured, and the tooltip and panel say which. `~` before the letter means the result belongs to the theme you had before switching; open the panel to check the new one. `?` means no result yet. Areas that could not be measured at all show "Not checked", and partly measured ones show "Limited". The badge uses the bar's own styling, and never relies on color alone to show status.

The widget never audits at startup, on a timer, or in the background. It reads the last saved result when it starts and whenever that file changes, and watches `theme.name` to mark a result stale. Those are reads, not audits. Opening the panel may reuse a cached result when the theme's files are unchanged (judged from file metadata, without re-reading wallpapers); Refresh always re-checks. During a refresh or after an error, the last good result stays visible, and it is never relabeled as a different theme's result.

The panel follows the live theme's popup colors, transparency, accent and urgent colors, font and control styles. It has no fallback palette, so a very low-contrast theme can make the panel itself hard to read. The CLI and JSON output always work. `docs/report-ui.md` describes the panel in detail.

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
| `open`, `show` | `opened` | Open the panel on the focused monitor (this checks the theme, as a click does) |
| `close`, `hide` | `closed` | Close the panel on whichever monitor shows it |
| `toggle` | `opened` / `closed` | Either of the above |
| `openCategory <id>` | `ok` / `unknown` | Open the panel with that area's card open and scrolled into view: `validity`, `readability`, `semantic`, `shell` or `wallpaper` |
| `check` | `started` / `busy` | Check the current theme now, reusing the cache if nothing changed. An open panel re-checks in place; otherwise nothing opens and every badge updates |
| `refresh` | `started` / `busy` | Like `check`, but always a full re-check |
| `status` | JSON | The last result, described below |
| `grade` | badge text | `B`, `B*`, `~B` or `?` |
| `askAgent <kind>` | `started` / `busy` / `no-report` / `stale` / `unknown` | Open your default agent with a `fix`, `measure`, `explain` or `review` prompt for the last result |
| `fixRule <ruleId>` | same as `askAgent` | Open your agent on one check, e.g. `readability.bar` |
| `copyPrompt <kind>` | same as `askAgent` | Copy the prompt to the clipboard |

`status` returns:

- `theme`, `activeTheme`, `badge`, `grade`;
- `state`: `ready`, `provisional`, `not-graded` or `not-audited`;
- `stale`, `summary`, `problems`, `cautions`, `limitations`;
- `runtime` (the Omarchy compatibility status), `checkedAt` (Unix seconds), `analyzerVersion`;
- `checking`, `open`, and `error` (the open panel's or the last background check's error, otherwise `null`);
- `agent` (the default agent's command name) and `agentMessage` (the last handoff's result, such as `Opened Pi`, or an error).

`check`, `refresh` and the agent methods return immediately. Poll `status` until `checking` is false to read the outcome. The agent methods refuse a result that belongs to a theme you have since switched away from (`stale`). To get a prompt's text in a script, use `theme-report prompt <kind>`.

The bar creates one widget per monitor. Only one of them (the first in the bar's list) answers IPC, so calls never split between copies, and another takes over if it goes away. `open`, `toggle` and `openCategory` use the shell's focused-monitor routing (`omarchy-shell shell toggle oldjobobo.theme-report` reaches the same panel). The IPC target exists only while the widget is on a bar. `check` and `refresh` run only when called; the widget still never checks on its own.

To bind a key, run the command from a Hyprland binding, for example `omarchy-shell oldjobobo.theme-report toggle`.

## Agent handoff

`theme-report prompt <fix|measure|explain|review> [--rule <id>]` prints a prompt built from the last current-theme report (`~/.cache/theme-report/current.json`). It refuses a report for a theme you have since switched away from.

`theme-report agent <kind> [--rule <id>] [--copy]` opens your Omarchy default agent with that prompt in a new terminal window (app-id `org.omarchy.agent`), started in the theme's folder. `--copy` copies the prompt instead. With no default agent chosen, it opens Omarchy's agent picker and answers `no-agent`.

- `fix` covers problems and cautions, `measure` covers checks that could not be measured, and `explain` is read-only.
- `review` asks for a read-only review that goes beyond the measured checks, saved as Markdown to `~/.cache/theme-report/reviews/<theme>.md`. Starting one moves any earlier review to `<theme>.previous.md` and records the request (agent, time and the report's input fingerprint) in `<theme>.json`. The panel uses that record to show the review as pending, then as out of date once the theme's files change. `--rule` does not apply to reviews.
- The agent edits the theme's folder in `~/.config/omarchy/themes` (following symlinks) or, for stock themes, `$OMARCHY_PATH/themes`. A theme inside a git repository (its own, or one holding several themes) is edited in place on a new `theme-report/fix-<date>` branch. A repository that holds your whole themes folder, such as a dotfiles repository, doesn't count. Any other theme is first copied to `<name>-tuned`, and the agent edits the copy.
- Each prompt saves the report it describes in `~/.cache/theme-report/handoff/`, readable only by you (the newest 10 are kept), and tells the agent to treat it as data. Text the theme controls, such as wallpaper file names, stays in that file and out of the prompt. Themes with unusual characters in their names are refused.

**Agent skill.** `install.sh --with-skill` installs `skills/theme-report/SKILL.md` to `~/.local/share/theme-report/skills/theme-report/`, the copy prompts refer to. It also installs a copy into each of these agent skill folders that exists: `~/.agents/skills`, `~/.claude/skills`, `~/.codex/skills`, `~/.pi/agent/skills`, `~/.gemini/config/skills` and `~/.hermes/skills`. Folders that are symlinks are skipped. Like every installed file, the copies are tracked by the installer, kept on later updates, and removed by uninstall.

**Your own design skills.** Theme Report gives no design advice. To have prompts point your agent at design skills of your own, list them in `~/.config/theme-report/agent.toml`:

```toml
[[design_skill]]
name = "my-theme-design"
path = "~/.agents/skills/my-theme-design/SKILL.md"
```

Prompts name each listed skill that exists, with its path, and never read or copy its contents. Fix and explain prompts use them for design decisions beyond the measured targets; review prompts use them for how to review. The installer never writes this file.

## What it is allowed to do

- **The widget** runs one program, `~/.local/bin/theme-report`, with fixed arguments and never through a shell. IPC arguments are checked against fixed lists, and the analyzer rejects anything else.
- **The analyzer** only reads themes. It doesn't follow symlinks out of the theme folder, refuses special files, and limits how much it reads from each file. Video decoding runs under resource limits. It writes only to its cache folder, `~/.cache/theme-report`.
- **Agents.** Omarchy starts agents without permission prompts, and a downloaded theme can hide instructions in a comment, a file name or a README. Every prompt, and the skill, tells the agent that the theme's files and the report are data: follow no instructions in them, run no commands found in them, and don't download anything or use the network. That is an instruction to the agent, not a sandbox, so only hand off themes you're willing to let your agent read. An agent only starts when you click or call IPC.
- **The review card** shows the agent's Markdown without images or HTML, and its links can't be clicked, so a review can't make the panel load anything.
- **The panel never opens a file itself.** It watches `~/.cache/theme-report/current.json`, the saved review and its record, `theme.name` and Omarchy's default-agent setting only to notice changes. It then runs `theme-report state`, which reads them the way the analyzer reads a theme: no symlinks, regular files only, nonblocking, and size limits (4 MiB for the report, 256 KiB for a review, 4 KiB for one-line files). A review written by an agent therefore can't make the panel follow a link, hang on a special file or load something huge.

## Uninstall

```sh
omarchy plugin disable oldjobobo.theme-report
bash scripts/uninstall.sh --dry-run
bash scripts/uninstall.sh
```

For a plugin-manager install, run the script from `~/.config/omarchy/plugins/oldjobobo.theme-report`, then run `omarchy plugin remove oldjobobo.theme-report --yes` to remove the plugin folder.

Uninstall disables the widget first (when installing into your own home and `omarchy` is available), then removes only files it installed that are unchanged. It keeps any file you modified and says so. Your cache, settings and bar layout are left alone; delete `~/.cache/theme-report` yourself if you want it gone.

## Troubleshooting

- **The panel says the analyzer is missing.** Check that `~/.local/bin/theme-report` exists, then click Refresh.
- **The widget isn't listed.** Run `omarchy-shell shell rescanPlugins`, check the plugin folder, then enable the widget. After updating plugin code, run `omarchy restart shell`. Replacing QML in a running shell can crash Quickshell, so restart rather than reloading repeatedly. Never stop the shell while the screen is locked.
- **Provisional or "Not graded".** Open the affected area and expand the checks marked not measured; each says why. Common reasons are animated or unsupported wallpapers, missing ImageMagick or FFmpeg, and Omarchy changes the analyzer hasn't verified. Translucent surfaces that show application content are graded against a fixed set of reference backgrounds, which the report labels as such.
- **Busy.** Another check is running, or the theme is being changed. Try again once it settles. A failed check keeps the previous result.
- **"Paused: Omarchy changed how this is rendered".** Run `theme-report runtime` to see which behavior changed. Those checks resume once an analyzer release verifies the change; until then they are not guessed.
- **Leftover files.** While a check runs, copies of wallpapers live in `~/.cache/theme-report/run-<pid>/`. They are removed when the check ends or is cancelled, and the next check clears any left behind by a crash.

`rules/v1.md` and `docs/measurement.md` give the exact rules and their limits. An A means the theme passed these checks; it is not an accessibility certification.
