#!/usr/bin/env python3
"""Verify open-only audits, manual refresh, cancellation and idle widget isolation."""
import os
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORK = ROOT / ".testbed/refresh"
WORK.mkdir(parents=True, exist_ok=True)

with tempfile.TemporaryDirectory(dir=WORK) as temp:
    base = pathlib.Path(temp)
    home = base / "home"
    helper = home / ".local/bin/theme-report"
    helper.parent.mkdir(parents=True)
    helper.write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys, time
home = pathlib.Path(os.environ["HOME"])
if "change-inputs" in sys.argv:
    (home / ".local/state/omarchy/current/theme/colors.toml").write_text("changed")
    sys.exit(0)
with (home / "audit-count").open("a") as count:
    count.write("audit\\n")
cache = pathlib.Path(os.environ["XDG_CACHE_HOME"]) / "theme-report"
cache.mkdir(parents=True, exist_ok=True)
snapshot = cache / (str(os.getpid()) + ".snapshot")
snapshot.write_text("media")
time.sleep(0.25)
snapshot.unlink()
report = {"schemaVersion": 2, "target": "current", "state": "ready", "grade": "A", "theme": "fixture", "summary": "Healthy", "findings": [], "categories": [], "watchedPaths": [], "auditTimestamp": time.time()}
tmp = cache / (str(os.getpid()) + ".json")
tmp.write_text(json.dumps(report))
tmp.replace(cache / "current.json")
print(json.dumps(report))
''')
    helper.chmod(0o755)
    current = home / ".local/state/omarchy/current"
    (current / "theme").mkdir(parents=True)
    for name in ["theme.name", "theme/colors.toml", "theme/shell.toml", "background"]:
        (current / name).write_text("fixture")
    (home / ".config/omarchy").mkdir(parents=True)
    (home / ".config/omarchy/shell.toml").write_text("fixture")
    ui = base / "Ui"
    commons = base / "Commons"
    plugin = base / "Plugin"
    ui.mkdir()
    commons.mkdir()
    plugin.mkdir()
    (ui / "qmldir").write_text("module qs.Ui\nPanel 1.0 Panel.qml\nWidgetButton 1.0 WidgetButton.qml\nKeyboardPanel 1.0 KeyboardPanel.qml\n")
    (commons / "qmldir").write_text("module qs.Commons\nsingleton Style 1.0 Style.qml\n")
    (commons / "Style.qml").write_text("pragma Singleton\nimport QtQuick\nQtObject { function space(n) { return n } }\n")
    (ui / "Panel.qml").write_text('''import QtQuick
Item {
    property string moduleName
    property string ipcTarget
    property bool opened: false
    property var bar: null
    property color barForeground: "white"
    function toggle() { opened = !opened }
    function close() { opened = false }
}
''')
    (ui / "WidgetButton.qml").write_text('''import QtQuick
Item {
    property var bar
    property color foreground
    property string text
    property string tooltipText
    signal pressed(int button)
}
''')
    (ui / "KeyboardPanel.qml").write_text('''import QtQuick
Item {
    property var owner
    property var anchorItem
    property var bar
    property bool open
    property var focusTarget
    property real contentWidth
    property real contentHeight
    function fittedContentWidth(n) { return n }
    function cappedContentHeight(n) { return n }
}
''')
    (plugin / "ReportPanel.qml").write_text('''import QtQuick
Item {
    property var report
    property string status
    property string errorText
    property bool refreshing
    signal refreshRequested()
    signal closeRequested()
}
''')
    shutil.copyfile(ROOT / "plugin/Main.qml", plugin / "Main.qml")
    (base / "shell.qml").write_text('''import QtQuick
import Quickshell
import Quickshell.Io
import "Plugin" as Plugin
ShellRoot {
    Plugin.Main { id: first }
    Plugin.Main { id: second }
    property int stage: 0
    property var retained: null
    function verify(condition, message) {
        if (!condition) { console.error("FAIL:", message, first.status, first.error, second.status, second.error); Qt.quit() }
        return condition
    }
    Process { id: mutation; command: [Quickshell.env("HOME") + "/.local/bin/theme-report", "change-inputs"] }
    Timer {
        interval: 700; running: true; repeat: true
        onTriggered: {
            if (stage === 0) {
                if (!verify(!first.report && !second.report && first.status === "Not audited" && second.status === "Not audited", "no startup audits")) return
                first.request(true); second.request(true)
                stage++
            } else if (stage === 1) {
                if (!verify(!first.report && !second.report, "closed refresh is ignored")) return
                first.opened = true
                first.request(true) // Busy refresh must not queue a second process.
                stage++
            } else if (stage === 2) {
                if (!verify(first.report && first.status === "A" && !first.error && !second.report, "opening audits only its own widget")) return
                retained = first.report
                mutation.running = true
                stage++
            } else if (stage === 3) {
                if (!verify(first.report === retained && first.status === "A" && !second.report, "input and cache writes do not trigger audits")) return
                first.request(true)
                stage++
            } else if (stage === 4) {
                if (!verify(first.report !== retained && first.status === "A" && !first.error, "manual Refresh updates the report")) return
                retained = first.report
                first.close()
                first.request(true)
                stage++
            } else if (stage === 5) {
                if (!verify(first.report === retained && first.status === "A" && !second.report, "closing stops background activity")) return
                first.opened = true
                stage++
            } else if (stage === 6) {
                if (!verify(first.report !== retained && first.status === "A", "reopening rechecks")) return
                retained = first.report
                first.request(true)
                cancellation.start()
                stage++
            } else {
                if (!verify(!first.opened && first.report === retained && first.status === "A" && !first.error && !second.report, "closing cancels an in-flight check")) return
                console.log("PASS: startup idle, open-only audits, explicit Refresh, no watchers or queued work, close cancellation")
                Qt.quit()
            }
        }
    }
    Timer { id: cancellation; interval: 100; onTriggered: first.close() }
}
''')
    cache = base / "cache/theme-report"
    cache.mkdir(parents=True)
    (cache / "current.json").write_text("{}")
    runtime = base / "runtime"
    runtime.mkdir(mode=0o700)
    env = dict(os.environ, HOME=str(home), XDG_CACHE_HOME=str(base / "cache"),
               XDG_RUNTIME_DIR=str(runtime), QT_QPA_PLATFORM="offscreen")
    result = subprocess.run(["quickshell", "-p", str(base)], env=env, text=True,
                            capture_output=True, timeout=10)
    output = result.stdout + result.stderr
    assert result.returncode == 0 and "PASS: startup" in output and "FAIL:" not in output, output
    assert (home / "audit-count").read_text().splitlines() == ["audit"] * 4, output
    assert not any(token in (ROOT / "plugin/Main.qml").read_text() for token in
                   ["Component.onCompleted", "FileView", "Timer", "queued", "freshnessExpiry"]), "Automatic audits must remain absent"
    print("PASS: open-only audit regression (real Process, two widgets, exact invocation count, cancellation)")
