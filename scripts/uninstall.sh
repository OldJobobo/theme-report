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
  omarchy plugin disable "$id" >/dev/null 2>&1 && echo "Disabled $id" || true
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
  $dry || rm -- "$target"
done
if $conflict; then exit 1; fi
if ! $dry; then rm -- "$receipt"; rmdir -- "$plugin" 2>/dev/null || true; fi
echo "Cache and shell layout are preserved."
