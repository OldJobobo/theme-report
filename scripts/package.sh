#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$root"
export CARGO_HOME="${CARGO_HOME:-$root/.build/cargo}"
export PYTHONDONTWRITEBYTECODE=1
cargo build --release --locked
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml","rb"))["package"]["version"])')
name="theme-report-$version-linux-$(uname -m)"
mkdir -p dist .testbed
work=$(mktemp -d "$root/.testbed/package.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work/$name/bin" "$work/$name/scripts" "$work/$name/docs/evidence"
cp target/release/theme-report "$work/$name/bin/"
cp scripts/{install,uninstall}.sh "$work/$name/scripts/"
cp -r hooks rules schema skills "$work/$name/"
# The docs without the development evidence; the README image is copied below.
find docs -maxdepth 1 -type f -exec cp -t "$work/$name/docs/" {} +
cp manifest.json Main.qml ReportPanel.qml LICENSE README.md "$work/$name/"
host=$(rustc -vV | awk '/^host:/ { print $2 }')
cargo metadata --format-version 1 --locked --filter-platform "$host" > "$work/metadata.json"
python3 - "$work/metadata.json" "$work/$name" <<'PY'
import json, shutil, sys
from pathlib import Path
out = Path(sys.argv[2]) / 'licenses'
out.mkdir()
rows = []
for package in json.load(open(sys.argv[1]))['packages']:
    if package['name'] == 'theme-report':
        continue
    files = [p for p in Path(package['manifest_path']).parent.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE'))]
    if not files:
        raise SystemExit('Review missing license files for ' + package['name'])
    dest = out / (package['name'] + '-' + package['version'])
    dest.mkdir()
    for source in files:
        shutil.copyfile(source, dest / source.name)
    rows.append(f"{package['name']} {package['version']}: {package['license']}")
(out / 'DEPENDENCIES.txt').write_text('\n'.join(rows) + '\n')
PY
sysroot=$(rustc --print sysroot)
cp -r "$sysroot/share/doc/rust/licenses" "$work/$name/licenses/rust-toolchain"
cp "$sysroot/share/doc/rust/COPYRIGHT-library.html" "$work/$name/licenses/"
cp docs/evidence/ui-overview.png "$work/$name/docs/evidence/"
printf 'Install preview: bash scripts/install.sh --binary bin/theme-report --dry-run\nInstall with optional hook: bash scripts/install.sh --binary bin/theme-report --with-hook\nThen: omarchy-shell shell rescanPlugins; omarchy plugin enable oldjobobo.theme-report\n' > "$work/$name/RELEASE.txt"
tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner -czf "dist/$name.tar.gz" -C "$work" "$name"
sha256sum "dist/$name.tar.gz" > "dist/$name.tar.gz.sha256"
printf 'Created dist/%s.tar.gz\n' "$name"
