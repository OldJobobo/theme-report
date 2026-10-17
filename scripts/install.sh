#!/usr/bin/env bash
set -euo pipefail
root=$HOME
binary=""
dry=false
hook=false
skill=false
while (($#)); do
  case "$1" in
    --root) root=${2:?}; shift 2;;
    --binary) binary=${2:?}; shift 2;;
    --with-hook) hook=true; shift;;
    --with-skill) skill=true; shift;;
    --dry-run) dry=true; shift;;
    *) echo "Usage: install.sh [--root DIR] [--binary FILE] [--with-hook] [--with-skill] [--dry-run]" >&2; exit 2;;
  esac
done
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
binary=${binary:-$repo/target/release/theme-report}
[[ -f $binary ]] || { echo "Build with cargo build --release --locked first." >&2; exit 1; }
root=$(realpath -m -- "$root")
[[ $root != *$'\n'* && $root != *$'\t'* ]] || { echo "Unsupported root name" >&2; exit 1; }
plugin=.config/omarchy/plugins/oldjobobo.theme-report
receipt="$root/$plugin/.theme-report-owned"
paths=(.local/bin/theme-report "$plugin/manifest.json" "$plugin/Main.qml" "$plugin/ReportPanel.qml")
sources=("$binary" "$repo/plugin/manifest.json" "$repo/plugin/Main.qml" "$repo/plugin/ReportPanel.qml")
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
  if $dry; then printf 'Would install: %s\n' "$target"; else printf 'Install target: %s\n' "$target"; fi
done
"$binary" version >/dev/null 2>&1 || { echo "Cannot run $binary on this system (wrong architecture or damaged build)." >&2; exit 1; }
$dry && exit 0
updating=false
plugin_changed=false
[[ -f $receipt ]] && updating=true
mkdir -p -- "$(dirname -- "$receipt")"
tmp=$(mktemp -- "${receipt}.XXXXXX")
trap 'rm -f -- "$tmp"' EXIT
# Own both the previous and the new content before copying anything, so an interrupted
# install can be re-run or uninstalled instead of reporting its own files as unowned.
if $updating; then cat -- "$receipt" > "$tmp"; fi
for i in "${!paths[@]}"; do
  h=$(sha256sum -- "${sources[$i]}"); printf '%s\t%s\n' "${h%% *}" "${paths[$i]}" >> "$tmp"
done
mv -- "$tmp" "$receipt"
tmp=$(mktemp -- "${receipt}.XXXXXX")
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
if $updating && ! $plugin_changed; then
  echo "Updated project-owned files. Plugin code is unchanged; no shell restart needed."
elif $updating; then
  echo "Updated project-owned files."
  # Replacing QML under a running shell can crash Quickshell; load it with a clean restart.
  echo "Load the new plugin code: omarchy restart shell"
else
  echo "Installed project-owned files. Rescan: omarchy-shell shell rescanPlugins"
  echo "Then enable: omarchy plugin enable oldjobobo.theme-report"
fi
