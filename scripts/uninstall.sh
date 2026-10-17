#!/usr/bin/env bash
set -euo pipefail
root=$HOME
dry=false
while (($#)); do
  case "$1" in --root) root=${2:?}; shift 2;; --dry-run) dry=true; shift;; *) echo "Usage: uninstall.sh [--root DIR] [--dry-run]" >&2; exit 2;; esac
done
root=$(realpath -m -- "$root")
id=oldjobobo.theme-report
plugin="$root/.config/omarchy/plugins/$id"
receipt="$plugin/.theme-report-owned"
[[ -f $receipt && ! -L $receipt ]] || { echo "No owned install found."; exit 0; }
# Disable before removing files, so the bar never references a missing plugin.
if ! $dry && [[ $root == "$(realpath -m -- "$HOME")" ]] && command -v omarchy >/dev/null; then
  if omarchy plugin disable "$id" >/dev/null 2>&1; then echo "Disabled $id"; fi
else
  echo "Disable the widget first if it is enabled: omarchy plugin disable $id"
fi
# A receipt may list several owned hashes for one path (an interrupted update).
declare -A owned=()
order=()
while IFS=$'\t' read -r hash rel; do
  [[ -n ${owned[$rel]+set} ]] || order+=("$rel")
  owned[$rel]+="$hash "
done < "$receipt"
conflict=false
for rel in "${order[@]}"; do
  case "$rel" in
    .local/bin/theme-report|.config/omarchy/plugins/oldjobobo.theme-report/manifest.json|.config/omarchy/plugins/oldjobobo.theme-report/Main.qml|.config/omarchy/plugins/oldjobobo.theme-report/ReportPanel.qml|.config/omarchy/hooks/theme-set.d/theme-report) ;;
    .local/share/theme-report/skills/theme-report/SKILL.md|.agents/skills/theme-report/SKILL.md|.claude/skills/theme-report/SKILL.md|.codex/skills/theme-report/SKILL.md|.pi/agent/skills/theme-report/SKILL.md|.gemini/config/skills/theme-report/SKILL.md|.hermes/skills/theme-report/SKILL.md) ;;
    *) echo "Unrecognized receipt entry; refusing: $rel" >&2; conflict=true; continue;;
  esac
  target="$root/$rel"
  parent=$target
  safe=true
  while [[ $parent != "$root" && $parent != / ]]; do
    [[ ! -L $parent ]] || safe=false
    parent=$(dirname -- "$parent")
  done
  if ! $safe; then echo "Leaving symlink destination: $target" >&2; conflict=true; continue; fi
  [[ -e $target ]] || continue
  actual=$(sha256sum -- "$target"); actual=${actual%% *}
  if [[ " ${owned[$rel]}" != *" $actual "* ]]; then echo "Leaving modified file: $target" >&2; conflict=true; continue; fi
  printf 'Remove: %s\n' "$target"
  if ! $dry; then
    rm -- "$target"
    # A skill lives in its own folder; remove the folder once it is empty.
    if [[ $rel == */theme-report/SKILL.md ]]; then
      rmdir -- "$(dirname -- "$target")" 2>/dev/null || true
      if [[ $rel == .local/share/* ]]; then
        rmdir -- "$root/.local/share/theme-report/skills" "$root/.local/share/theme-report" 2>/dev/null || true
      fi
    fi
  fi
done
if $conflict; then exit 1; fi
if ! $dry; then rm -- "$receipt"; rmdir -- "$plugin" 2>/dev/null || true; fi
echo "Cache and shell layout are preserved."
# A marketplace install keeps its plugin files in a git checkout that Omarchy manages.
if [[ -d $plugin/.git ]]; then echo "Remove the plugin itself: omarchy plugin remove $id --yes"; fi
