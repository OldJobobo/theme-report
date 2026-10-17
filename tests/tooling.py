#!/usr/bin/env python3
"""Repository-local installer, schema, and golden checks (jsonschema is a dev dependency)."""
import json
import pathlib
import subprocess
import tempfile
import jsonschema

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/release/theme-report"
WORK = ROOT / ".testbed/tooling"
WORK.mkdir(parents=True, exist_ok=True)

def command(*args, success=True):
    result = subprocess.run(args, cwd=ROOT, text=True, capture_output=True)
    assert (result.returncode == 0) == success, (args, result.stdout, result.stderr)
    return result

with tempfile.TemporaryDirectory(dir=WORK) as temp:
    home = pathlib.Path(temp) / "home with spaces"
    install = ["bash", "scripts/install.sh", "--root", str(home), "--binary", str(BINARY), "--with-hook"]
    command(*install, "--dry-run")
    assert not home.exists(), "Dry run mutated destination"
    command(*install)
    command(*install)
    binary = home / ".local/bin/theme-report"
    command(str(binary), "version")
    receipt = home / ".config/omarchy/plugins/oldjobobo.theme-report/.theme-report-owned"
    assert len(receipt.read_text().splitlines()) == 5
    panel = home / ".config/omarchy/plugins/oldjobobo.theme-report/Main.qml"
    panel.write_text(panel.read_text() + "\n// user edit\n")
    command(*install, success=False)
    command("bash", "scripts/uninstall.sh", "--root", str(home), success=False)
    assert panel.exists(), "Uninstall deleted a user-modified file"
    assert not binary.exists()

with tempfile.TemporaryDirectory(dir=WORK) as temp:
    home = pathlib.Path(temp) / "clean"
    command("bash", "scripts/install.sh", "--root", str(home), "--binary", str(BINARY), "--with-hook")
    cache = home / ".cache/theme-report"
    cache.mkdir(parents=True)
    (cache / "current.json").write_text("keep")
    command("bash", "scripts/uninstall.sh", "--root", str(home), "--dry-run")
    assert (home / ".local/bin/theme-report").exists()
    command("bash", "scripts/uninstall.sh", "--root", str(home))
    command("bash", "scripts/uninstall.sh", "--root", str(home))
    assert not (home / ".local/bin/theme-report").exists()
    assert (cache / "current.json").read_text() == "keep"

schema = json.loads((ROOT / "schema/report-v2.schema.json").read_text())
jsonschema.Draft202012Validator.check_schema(schema)
result = command(str(BINARY), "audit", "fixtures/themes/minimal", "--profile", "quattro-shell-2026-v1", "--format", "json", "--cache-dir", str(WORK / "cache"))
report = json.loads(result.stdout)
jsonschema.validate(report, schema)
assert report["grade"] == "A" and report["state"] == "ready"
assert "coverage" not in report and "eligibilityReasons" not in report
for finding in report["findings"]:
    assert 0 <= finding["pointsLost"] <= finding["assessedBudget"] <= finding["budget"]
for category in report["categories"]:
    assert 0 <= category["lost"] <= category["assessed"] <= category["expected"] <= category["weight"]
# Golden stores scoring evidence, not absolute paths, timestamps, or request state.
golden = {k: report[k] for k in ("rulesetVersion", "score", "grade", "state", "summary", "categories", "findings", "capReasons")}
expected = json.loads((ROOT / "fixtures/reports/minimal.json").read_text())
assert golden == expected, "Deterministic minimal report changed; review rules/version before updating the golden"
main = (ROOT / "plugin/Main.qml").read_text()
assert "emergencySkin" not in main and "anchors.fill: badge" not in main, "The badge must inherit native bar styling"
assert "foreground: root.barForeground" in main
print("PASS: installer dry run/update/conflicts/uninstall; cache preservation; schema; deterministic golden; native badge styling")
