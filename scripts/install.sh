#!/usr/bin/env bash
# Installs the Theme Report analyzer (and, outside a plugin-manager checkout, the widget).
# With no options it uses, in order: the binary bundled in a release archive, a matching
# local cargo build, or the matching release from GitHub, checksum-verified.
set -euo pipefail
root=$HOME
binary=""
build=false
dry=false
hook=false
skill=false
usage() {
  cat <<'USAGE'
Usage: install.sh [options]

  --with-hook     re-check the theme in the background after every theme switch
  --with-skill    install the theme-report agent skill for your coding agents
  --build         build the analyzer from source with Rust instead of downloading it
  --binary FILE   install this analyzer binary
  --dry-run       show what would be installed, change nothing
  --root DIR      install into DIR instead of your home folder (for testing)
USAGE
}
while (($#)); do
  case "$1" in
    --root) root=${2:?}; shift 2;;
    --binary) binary=${2:?}; shift 2;;
    --build) build=true; shift;;
    --with-hook) hook=true; shift;;
    --with-skill) skill=true; shift;;
    --dry-run) dry=true; shift;;
    -h|--help) usage; exit 0;;
    *) usage >&2; exit 2;;
  esac
done
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
version=$(sed -n 's/^ *"version": *"\([^"]*\)".*/\1/p' "$repo/manifest.json" | head -n 1)
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Cannot read the plugin version from $repo/manifest.json" >&2; exit 1; }
cleanup=()
trap 'rm -rf -- "${cleanup[@]}"' EXIT
matches() { [[ $("$1" version 2>/dev/null) == "theme-report $version "* ]]; }
download=false
if [[ -n $binary ]]; then
  :
elif $build; then
  command -v cargo >/dev/null || { echo "--build needs Rust. Install the rust package, then run this again." >&2; exit 1; }
  # Build outside the plugin folder so build files stay out of ~/.config.
  export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/.cache/theme-report/build}
  if $dry; then
    echo "Would build theme-report $version with cargo into $CARGO_TARGET_DIR"
  else
    echo "Building theme-report $version (this takes a few minutes)..."
    cargo build --release --locked --manifest-path "$repo/Cargo.toml"
  fi
  binary=$CARGO_TARGET_DIR/release/theme-report
elif [[ -f $repo/bin/theme-report ]]; then
  binary=$repo/bin/theme-report
elif local_build=${CARGO_TARGET_DIR:-$repo/target}/release/theme-report && [[ -f $local_build ]] && matches "$local_build"; then
  binary=$local_build
else
  download=true
fi
if $download; then
  arch=$(uname -m)
  [[ $arch == x86_64 ]] || { echo "There is no prebuilt analyzer for $arch. Install Rust and run: bash $repo/scripts/install.sh --build" >&2; exit 1; }
  command -v curl >/dev/null || { echo "Downloading the analyzer needs curl." >&2; exit 1; }
  name=theme-report-$version-linux-x86_64.tar.gz
  url=https://github.com/OldJobobo/theme-report/releases/download/v$version
  if $dry; then
    echo "Would download $url/$name and check its checksum"
  else
    work=$(mktemp -d); cleanup+=("$work")
    echo "Downloading theme-report $version..."
    for file in "$name" "$name.sha256"; do
      curl -fsSL --proto '=https' --tlsv1.2 -o "$work/$file" "$url/$file" ||
        { echo "Could not download $url/$file" >&2; echo "To build it yourself instead: bash $repo/scripts/install.sh --build" >&2; exit 1; }
    done
    (cd -- "$work" && sha256sum --quiet -c -- "$name.sha256") || { echo "Checksum mismatch for $name; nothing was installed." >&2; exit 1; }
    if command -v gh >/dev/null && gh auth status >/dev/null 2>&1; then
      gh attestation verify "$work/$name" -R OldJobobo/theme-report >/dev/null ||
        { echo "GitHub could not confirm $name was built by this project's release workflow; nothing was installed." >&2; exit 1; }
      echo "Verified: checksum matches, and GitHub confirms the release workflow built it."
    else
      echo "Verified: checksum matches. (With the gh CLI logged in, the installer also checks who built it.)"
    fi
    mkdir -- "$work/x"
    tar -xzf "$work/$name" -C "$work/x"
    binary=$work/x/theme-report-$version-linux-x86_64/bin/theme-report
  fi
fi
$dry || [[ -f $binary ]] || { echo "No analyzer binary at $binary" >&2; exit 1; }
root=$(realpath -m -- "$root")
[[ $root != *$'\n'* && $root != *$'\t'* ]] || { echo "Unsupported root name" >&2; exit 1; }
plugin=.config/omarchy/plugins/oldjobobo.theme-report
receipt="$root/$plugin/.theme-report-owned"
# Run from the plugin checkout that `omarchy plugin add` made, the plugin files are already
# in place and belong to git; install only the analyzer (and the opt-in extras).
checkout=false
[[ $(realpath -m -- "$root/$plugin") == "$repo" ]] && checkout=true
paths=(.local/bin/theme-report)
sources=("$binary")
if ! $checkout; then
  paths+=("$plugin/manifest.json" "$plugin/Main.qml" "$plugin/ReportPanel.qml")
  sources+=("$repo/manifest.json" "$repo/Main.qml" "$repo/ReportPanel.qml")
fi
hookrel=.config/omarchy/hooks/theme-set.d/theme-report
if $hook; then paths+=("$hookrel"); sources+=("$repo/hooks/theme-report"); fi
# The agent skill: one copy that prompts point at, plus one per agent skill folder
# that already exists (the folders Omarchy links its own skills into).
skillrels=(.local/share/theme-report/skills/theme-report/SKILL.md)
for dir in .agents/skills .claude/skills .codex/skills .pi/agent/skills .gemini/config/skills .hermes/skills; do
  skillrels+=("$dir/theme-report/SKILL.md")
done
if $skill; then
  for rel in "${skillrels[@]}"; do
    dir=${rel%/theme-report/SKILL.md}
    if [[ $rel != .local/share/* ]]; then
      [[ -d $root/$dir ]] || continue
      linked=false
      parent=$root/$dir
      while [[ $parent != "$root" && $parent != / ]]; do [[ ! -L $parent ]] || linked=true; parent=$(dirname -- "$parent"); done
      if $linked; then echo "Skipping linked skill folder: $root/$dir"; continue; fi
    fi
    paths+=("$rel"); sources+=("$repo/skills/theme-report/SKILL.md")
  done
fi
# Check every target and its parent chain before making any changes.
for rel in "${paths[@]}" "$plugin/.theme-report-owned"; do
  target="$root/$rel"
  parent=$target
  while [[ $parent != "$root" && $parent != / ]]; do
    [[ ! -L $parent ]] || { echo "Refusing symlink destination: $parent" >&2; exit 1; }
    parent=$(dirname -- "$parent")
  done
  if [[ -e $target && $target != "$receipt" ]]; then
    [[ -f $receipt ]] || { echo "Unowned destination: $target" >&2; exit 1; }
    actual=$(sha256sum -- "$target"); actual=${actual%% *}
    grep -Fqx -- "$actual"$'\t'"$rel" "$receipt" || { echo "Modified or unowned destination: $target" >&2; exit 1; }
  fi
  if $dry; then printf 'Would install: %s\n' "${target/#"$HOME"/\~}"; fi
done
if [[ -f $binary ]]; then
  "$binary" version >/dev/null 2>&1 || { echo "Cannot run $binary on this system (wrong architecture or damaged build)." >&2; exit 1; }
  matches "$binary" || echo "Note: $binary is not version $version; the widget may not match it."
fi
$dry && exit 0
updating=false
plugin_changed=false
[[ -f $receipt ]] && updating=true
mkdir -p -- "$(dirname -- "$receipt")"
tmp=$(mktemp -- "${receipt}.XXXXXX")
cleanup+=("$tmp")
# Own both the previous and the new content before copying anything, so an interrupted
# install can be re-run or uninstalled instead of reporting its own files as unowned.
if $updating; then cat -- "$receipt" > "$tmp"; fi
for i in "${!paths[@]}"; do
  h=$(sha256sum -- "${sources[$i]}"); printf '%s\t%s\n' "${h%% *}" "${paths[$i]}" >> "$tmp"
done
mv -- "$tmp" "$receipt"
tmp=$(mktemp -- "${receipt}.XXXXXX")
cleanup+=("$tmp")
# Keep ownership of installed optional files (hook, skill) during an update without their flag.
for old in "$hookrel" "${skillrels[@]}"; do
  printf '%s\n' "${paths[@]}" | grep -Fqx -- "$old" && continue
  [[ -f $root/$old ]] || continue
  h=$(sha256sum -- "$root/$old"); h=${h%% *}
  if grep -Fqx -- "$h"$'\t'"$old" "$receipt"; then printf '%s\t%s\n' "$h" "$old" >> "$tmp"; fi
done
for i in "${!paths[@]}"; do
  target="$root/${paths[$i]}"
  mkdir -p -- "$(dirname -- "$target")"
  mode=644
  [[ ${paths[$i]} == .local/bin/* || ${paths[$i]} == .config/omarchy/hooks/* ]] && mode=755
  # Identical files are left alone: replacing QML under a running shell can crash it.
  if cmp -s -- "${sources[$i]}" "$target"; then
    chmod "$mode" -- "$target"
  else
    install -m "$mode" -- "${sources[$i]}" "$target"
    [[ ${paths[$i]} == *.qml || ${paths[$i]} == */manifest.json ]] && plugin_changed=true
  fi
  h=$(sha256sum -- "$target"); h=${h%% *}
  printf '%s\t%s\n' "$h" "${paths[$i]}" >> "$tmp"
done
mv -- "$tmp" "$receipt"
echo "Installed theme-report $("$root/.local/bin/theme-report" version 2>/dev/null | cut -d' ' -f2)."
id=oldjobobo.theme-report
ask() {
  [[ -t 0 && -t 1 ]] || return 1
  local reply
  read -r -p "$1 [Y/n] " reply || return 1
  [[ -z $reply || $reply == [Yy]* ]]
}
# Only touch the running desktop when installing into this user's own home.
if [[ $root != "$(realpath -m -- "$HOME")" ]] || ! command -v omarchy >/dev/null; then
  if ! $checkout; then echo "Then: omarchy-shell shell rescanPlugins && omarchy plugin enable $id"; fi
  exit 0
fi
if $updating && $plugin_changed; then
  # Replacing QML under a running shell can crash Quickshell; load it with a clean restart.
  if ask "The widget changed. Restart the Omarchy shell now to load it?"; then
    omarchy restart shell
  else
    echo "Load the new widget later with: omarchy restart shell"
  fi
elif ! $checkout && ! $updating; then
  omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true
fi
enabled=$(omarchy plugin list --json 2>/dev/null | jq -r --arg id "$id" '.[] | select(.id == $id) | .enabled' 2>/dev/null || true)
if [[ $enabled != true ]]; then
  if ask "Add the Theme Report badge to your bar now?"; then
    omarchy plugin enable "$id"
  else
    echo "Add the badge to your bar later with: omarchy plugin enable $id"
    exit 0
  fi
fi
echo "Done. Click the Theme Report badge in your bar to see your theme's report."
