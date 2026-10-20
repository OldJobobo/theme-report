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

with tempfile.TemporaryDirectory(dir=WORK) as temp:
    # An update interrupted after copying leaves old and new hashes in the receipt;
    # both re-running the install and uninstalling must still work.
    import hashlib
    home = pathlib.Path(temp) / "interrupted"
    install = ["bash", "scripts/install.sh", "--root", str(home), "--binary", str(BINARY)]
    receipt = home / ".config/omarchy/plugins/oldjobobo.theme-report/.theme-report-owned"
    panel = home / ".config/omarchy/plugins/oldjobobo.theme-report/Main.qml"
    def interrupt():
        newer = panel.read_text() + "\n// newer release\n"
        digest = hashlib.sha256(newer.encode()).hexdigest()
        receipt.write_text(receipt.read_text() + digest + "\t.config/omarchy/plugins/oldjobobo.theme-report/Main.qml\n")
        panel.write_text(newer)
    command(*install)
    interrupt()
    command(*install)
    assert len(receipt.read_text().splitlines()) == 4, "Final receipt lists only installed hashes"
    interrupt()
    command("bash", "scripts/uninstall.sh", "--root", str(home))
    assert not panel.exists() and not receipt.exists()

with tempfile.TemporaryDirectory(dir=WORK) as temp:
    # The opt-in agent skill: one copy prompts cite, one per existing agent skill folder.
    home = pathlib.Path(temp) / "skill"
    (home / ".pi/agent/skills/other").mkdir(parents=True)
    (home / ".pi/agent/skills/other/SKILL.md").write_text("someone else's")
    elsewhere = pathlib.Path(temp) / "linked-skills"
    elsewhere.mkdir()
    (home / ".claude").mkdir()
    (home / ".claude/skills").symlink_to(elsewhere)
    install = ["bash", "scripts/install.sh", "--root", str(home), "--binary", str(BINARY)]
    out = command(*install, "--with-skill").stdout
    shipped = (ROOT / "skills/theme-report/SKILL.md").read_text()
    canonical = home / ".local/share/theme-report/skills/theme-report/SKILL.md"
    pi = home / ".pi/agent/skills/theme-report/SKILL.md"
    assert canonical.read_text() == shipped and pi.read_text() == shipped
    assert "Skipping linked skill folder" in out and not (elsewhere / "theme-report").exists()
    assert not (home / ".codex").exists(), "Absent agent folders are not created"
    receipt = home / ".config/omarchy/plugins/oldjobobo.theme-report/.theme-report-owned"
    assert len(receipt.read_text().splitlines()) == 6
    command(*install)  # An update without the flag keeps owning the skill.
    assert len(receipt.read_text().splitlines()) == 6 and pi.exists()
    command("bash", "scripts/uninstall.sh", "--root", str(home))
    assert not pi.parent.exists() and not (home / ".local/share/theme-report").exists()
    assert (home / ".pi/agent/skills/other/SKILL.md").read_text() == "someone else's"

with tempfile.TemporaryDirectory(dir=WORK) as temp:
    # Installed with `omarchy plugin add`: the plugin folder is a git checkout of this
    # repository. Its install.sh adds the analyzer and leaves the plugin files to git.
    import shutil
    home = pathlib.Path(temp) / "marketplace"
    checkout = home / ".config/omarchy/plugins/oldjobobo.theme-report"
    for rel in (".gitignore", "manifest.json", "Main.qml", "ReportPanel.qml", "scripts/install.sh",
                "scripts/uninstall.sh", "hooks/theme-report", "skills/theme-report/SKILL.md"):
        (checkout / rel).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / rel, checkout / rel)
    git = ["git", "-C", str(checkout), "-c", "user.name=t", "-c", "user.email=t@example.invalid"]
    command(*git, "init", "-q")
    command(*git, "add", "-A")
    command(*git, "commit", "-qm", "checkout")
    install = ["bash", str(checkout / "scripts/install.sh"), "--root", str(home), "--binary", str(BINARY), "--with-hook"]
    # With no binary to hand, the installer plans to fetch the release matching the widget.
    import os
    env = {k: v for k, v in os.environ.items() if k != "CARGO_TARGET_DIR"}
    plan = subprocess.run(["bash", str(checkout / "scripts/install.sh"), "--root", str(home), "--dry-run"],
                          cwd=ROOT, env=env, text=True, capture_output=True)
    version = json.loads((ROOT / "manifest.json").read_text())["version"]
    if f"{version}:x86_64)" in (checkout / "scripts/install.sh").read_text():
        assert plan.returncode == 0 and f"/v{version}/theme-report-{version}-linux-x86_64.tar.gz" in plan.stdout, plan
    else:
        # A release tag is built before its verified digest is pinned in source.
        assert plan.returncode != 0 and "No reviewed archive digest" in plan.stderr, plan
    assert not (home / ".local").exists(), "A dry run downloads and installs nothing"
    command(*install)
    command(*install)
    assert (home / ".local/bin/theme-report").exists() and (home / ".config/omarchy/hooks/theme-set.d/theme-report").exists()
    receipt = checkout / ".theme-report-owned"
    assert len(receipt.read_text().splitlines()) == 2, "Only the analyzer and hook are owned"
    assert command(*git, "status", "--porcelain").stdout == "", "The checkout stays clean for omarchy plugin update"
    out = command("bash", str(checkout / "scripts/uninstall.sh"), "--root", str(home)).stdout
    assert not (home / ".local/bin/theme-report").exists() and not receipt.exists()
    assert (checkout / "Main.qml").exists() and "omarchy plugin remove oldjobobo.theme-report" in out

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
with tempfile.TemporaryDirectory(dir=WORK) as temp:
    # Current audits carry the runtime contract; an unknown runtime pauses every check.
    home = pathlib.Path(temp)
    theme = home / ".local/state/omarchy/current/theme"
    theme.mkdir(parents=True)
    (theme / "colors.toml").write_text((ROOT / "fixtures/themes/minimal/colors.toml").read_text())
    current = json.loads(subprocess.run([str(BINARY), "audit", "--current", "--format", "json", "--omarchy-root", str(home), "--cache-dir", str(home / "cache")],
                                        env={"HOME": str(home), "PATH": "/usr/bin:/bin"}, text=True, capture_output=True, check=True).stdout)
    jsonschema.validate(current, schema)
    assert current["runtimeContract"]["status"] == "unsupported" and current["grade"] is None
# Golden stores scoring evidence, not absolute paths, timestamps, or request state.
golden = {k: report[k] for k in ("rulesetVersion", "score", "grade", "state", "summary", "categories", "findings", "capReasons")}
expected = json.loads((ROOT / "fixtures/reports/minimal.json").read_text())
assert golden == expected, "Deterministic minimal report changed; review rules/version before updating the golden"
main = (ROOT / "Main.qml").read_text()
assert "emergencySkin" not in main and "anchors.fill: badge" not in main, "The badge must inherit native bar styling"
assert "foreground: root.barForeground" in main
print("PASS: installer dry run/update/conflicts/uninstall; marketplace checkout; cache preservation; schema; deterministic golden; native badge styling")
