#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$root"
export CARGO_HOME="${CARGO_HOME:-$root/.build/cargo}"
export PYTHONDONTWRITEBYTECODE=1
cargo fmt --check
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
python3 tests/tooling.py
if command -v quickshell >/dev/null; then
  python3 tests/refresh.py
fi
shellcheck scripts/*.sh tests/vm/guest.sh hooks/theme-report
if command -v qmllint >/dev/null && [[ -d /usr/share/omarchy/shell ]]; then
  mkdir -p .testbed
  lint_root=$(mktemp -d "$root/.testbed/lint.XXXXXX")
  trap 'rm -rf -- "$lint_root"' EXIT
  mkdir -p "$lint_root/qs"
  # Fresh repository-local reference copies; never write through old symlinks.
  cp -r /usr/share/omarchy/shell/Commons "$lint_root/qs/"
  cp -r /usr/share/omarchy/shell/Ui "$lint_root/qs/"
  qmllint -I "$lint_root" Main.qml ReportPanel.qml > docs/evidence/qmllint.log 2>&1
  if command -v quickshell >/dev/null; then
    python3 tests/panel.py
  fi
else
  printf 'Native qmllint skipped: installed Omarchy/Qt tools unavailable. VM verification remains required.\n'
fi
printf 'All available checks passed.\n'
