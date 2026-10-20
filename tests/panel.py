#!/usr/bin/env python3
"""Run the actual report panel against a read-only copy of a native shell runtime."""
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
SHELL = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else os.environ.get("OMARCHY_PATH", "/usr/share/omarchy")) / "shell"
WORK = ROOT / ".testbed/panel"
WORK.mkdir(parents=True, exist_ok=True)

with tempfile.TemporaryDirectory(dir=WORK) as temp:
    base = pathlib.Path(temp)
    (base / "qs").mkdir()
    for module in ("Commons", "Ui"):
        shutil.copytree(SHELL / module, base / module)
        (base / "qs" / module).symlink_to("../" + module)
    (base / "plugin").mkdir()
    for name in ("manifest.json", "Main.qml", "ReportPanel.qml"):
        shutil.copyfile(ROOT / name, base / "plugin" / name)
    smoke = (ROOT / "tests/qml/report-smoke.qml").read_text().replace('import "../.." as Plugin', 'import "plugin" as Plugin')
    (base / "shell.qml").write_text(smoke)
    runtime = base / "run"
    runtime.mkdir(mode=0o700)
    home = base / "home"
    home.mkdir()
    env = dict(os.environ, HOME=str(home), XDG_CACHE_HOME=str(base / "cache"),
               XDG_RUNTIME_DIR=str(runtime), QT_QPA_PLATFORM="offscreen")
    result = subprocess.run(["quickshell", "-p", str(base)], env=env, text=True,
                            capture_output=True, timeout=15)
    output = result.stdout + result.stderr
    assert result.returncode == 0 and "PASS: live theme" in output and "UI assertion failed" not in output, output
    assert "TypeError:" not in output and "ReferenceError:" not in output, output
    lint = subprocess.run(["qmllint", "-I", str(base), str(base / "plugin/Main.qml"),
                           str(base / "plugin/ReportPanel.qml")], text=True, capture_output=True)
    assert lint.returncode == 0, lint.stdout + lint.stderr
    print("PASS: native panel theme/presentation smoke and QML lint · " + str(SHELL))
