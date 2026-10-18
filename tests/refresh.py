#!/usr/bin/env python3
"""Verify open-only audits, manual refresh, cancellation, shared cached results, stale badges and IPC."""
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
if sys.argv[1:2] == ["state"]:
    # Mirrors the real reader's output shape; reading state never audits.
    with (home / "state-log").open("a") as log:
        log.write(" ".join(sys.argv[1:]) + "\\n")
    cache = pathlib.Path(os.environ["XDG_CACHE_HOME"]) / "theme-report"
    def read(path):
        try:
            return path.read_text()
        except OSError:
            return None
    try:
        report = json.loads(read(cache / "current.json") or "null")
    except ValueError:
        report = None
    theme = sys.argv[sys.argv.index("--theme") + 1] if "--theme" in sys.argv else (report or {}).get("theme")
    name = read(home / ".local/state/omarchy/current/theme.name")
    agent = read(home / ".config/omarchy/defaults/agent")
    review = None
    if theme:
        meta = read(cache / "reviews" / (theme + ".json"))
        review = {"theme": theme, "text": read(cache / "reviews" / (theme + ".md")),
                  "meta": json.loads(meta) if meta else None, "problem": None}
    print(json.dumps({"schemaVersion": 1, "themeName": name.strip() if name else None,
                      "agent": agent.split("\\n")[0].strip() if agent else None,
                      "report": report if isinstance(report, dict) else None, "review": review}))
    sys.exit(0)
if sys.argv[1:2] in (["agent"], ["prompt"]):
    with (home / "agent-log").open("a") as log:
        log.write(" ".join(sys.argv[1:]) + "\\n")
    print("launched" if sys.argv[1] == "agent" else "PROMPT " + sys.argv[2])
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
    (home / ".config/omarchy/defaults").mkdir(parents=True)
    (home / ".config/omarchy/defaults/agent").write_text("pi\n")
    ui = base / "Ui"
    commons = base / "Commons"
    plugin = base / "Plugin"
    ui.mkdir()
    commons.mkdir()
    plugin.mkdir()
    (ui / "qmldir").write_text("module qs.Ui\nPanel 1.0 Panel.qml\nWidgetButton 1.0 WidgetButton.qml\nKeyboardPanel 1.0 KeyboardPanel.qml\n")
    (commons / "qmldir").write_text("module qs.Commons\nsingleton Style 1.0 Style.qml\nShellIpc 1.0 ShellIpc.qml\n")
    (commons / "ShellIpc.qml").write_text("import Quickshell.Io\nIpcHandler { }\n")
    (commons / "Style.qml").write_text("pragma Singleton\nimport QtQuick\nQtObject { function space(n) { return n } }\n")
    (ui / "Panel.qml").write_text('''import QtQuick
Item {
    property string moduleName
    property string ipcTarget
    property bool manageIpc: true
    property var settings: ({})
    property bool opened: false
    property var bar: null
    property color barForeground: "white"
    function open() { opened = true }
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
    property string agentName
    property bool agentBusy
    property string agentMessage
    property string agentPreview
    property bool actionsEnabled
    property string reviewText
    property var reviewMeta
    property string reviewAgentName
    signal agentRequested(string kind, string rule, string mode)
    signal previewClosed()
    objectName: "reportStub"
    property string category: ""
    function selectCategory(id) { category = id }
    signal refreshRequested()
    signal closeRequested()
}
''')
    shutil.copyfile(ROOT / "Main.qml", plugin / "Main.qml")
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
    // A bar listing both copies; its shell routes summon to the second (the "focused monitor").
    QtObject {
        id: fakeBar
        property var widgets: []
        property QtObject shell: QtObject {
            function summon(id, payload) { if (id !== "oldjobobo.theme-report") return false; second.open(); return true }
        }
        function moduleWidgets(id) { return id === "oldjobobo.theme-report" ? widgets : [] }
    }
    function namedItem(item, name) {
        if (item.objectName === name) return item
        for (var i = 0; i < item.children.length; i++) {
            var found = namedItem(item.children[i], name)
            if (found) return found
        }
        return null
    }
    Process { id: mark; command: ["sh", "-c", "echo $1 >> $HOME/audit-count", "sh", "ipc"] }
    Process { id: hideHelper; command: ["sh", "-c", "mv $HOME/.local/bin/theme-report $HOME/.local/bin/theme-report.off"] }
    Process { id: switchTheme; command: ["sh", "-c", "printf other > $HOME/.local/state/omarchy/current/theme.name"] }
    Process { id: switchBack; command: ["sh", "-c", "printf fixture > $HOME/.local/state/omarchy/current/theme.name"] }
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
                // The second widget shows the published result without running its own audit.
                if (!verify(first.report && first.status === "A" && !first.error && second.status === "A", "opening audits only its own widget; others read the shared result")) return
                retained = first.report
                mutation.running = true
                stage++
            } else if (stage === 3) {
                if (!verify(first.report === retained && first.status === "A", "input and cache writes do not trigger audits")) return
                first.request(true)
                stage++
            } else if (stage === 4) {
                if (!verify(first.report !== retained && first.status === "A" && !first.error, "manual Refresh updates the report")) return
                retained = first.report
                first.close()
                first.request(true)
                stage++
            } else if (stage === 5) {
                if (!verify(first.report === retained && first.status === "A", "closing stops background activity")) return
                first.opened = true
                stage++
            } else if (stage === 6) {
                if (!verify(first.report !== retained && first.status === "A", "reopening rechecks")) return
                retained = first.report
                first.request(true)
                cancellation.start()
                stage++
            } else if (stage === 7) {
                if (!verify(!first.opened && first.report === retained && first.status === "A" && !first.error, "closing cancels an in-flight check")) return
                if (!verify(!first.stale && !second.stale, "matching theme is not stale")) return
                switchTheme.running = true
                stage++
            } else if (stage === 8) {
                if (!verify(first.stale && second.stale && first.report === retained, "theme switch marks the badge stale without auditing")) return
                if (!verify(!first.ipcOwner && !second.ipcOwner, "no IPC owner before the bar places the widget")) return
                fakeBar.widgets = [first, second]
                first.bar = fakeBar
                second.bar = fakeBar
                mark.running = true
                stage++
            } else if (stage === 9) {
                if (!verify(first.ipcOwner && !second.ipcOwner, "exactly one copy owns the IPC target")) return
                var st = JSON.parse(first.statusJson())
                if (!verify(st.grade === "A" && st.badge === "~A" && st.stale === true && st.open === false && st.checking === false && st.theme === "fixture" && st.activeTheme === "other", "status JSON")) return
                if (!verify(first.badgeFor(first.latestReport()) === "~A", "grade answer")) return
                var firstCheck = first.startCheck(false), secondCheck = first.startCheck(true)
                if (!verify(firstCheck === "started" && secondCheck === "busy", "background check and busy answer")) return
                if (!verify(second.sharedChecking && second.checking, "every badge shows a background check")) return
                stage++
            } else if (stage === 10) {
                if (!verify(!first.checking && !second.checking && first.report !== retained && second.report === first.report && !first.opened && !second.opened, "background check updates every badge without opening")) return
                mark.command = ["sh", "-c", "echo $1 >> $HOME/audit-count", "sh", "ipc-panel"]
                mark.running = true
                if (!verify(first.togglePanel() === "opened" && second.opened && !first.opened, "toggle opens on the focused copy")) return
                if (!verify(JSON.parse(first.statusJson()).open === true, "status reports the open panel")) return
                if (!verify(first.togglePanel() === "closed" && !second.opened, "toggle closes wherever the panel is open")) return
                if (!verify(first.openCategory("bogus") === "unknown" && !second.opened, "unknown category is rejected")) return
                if (!verify(first.openCategory("shell") === "ok" && second.opened && namedItem(second, "reportStub").category === "shell", "openCategory opens the focused copy on that category")) return
                first.hidePanel()
                if (!verify(!second.opened, "close acts on the open copy")) return
                fakeBar.widgets = [second, first]
                first.electAll()
                if (!verify(second.ipcOwner && !first.ipcOwner, "ownership follows the bar's order")) return
                if (!verify(second.startAgent("fix", "", "window") === "stale", "agent handoff refuses a result for another theme")) return
                if (!verify(second.startAgent("bogus", "", "window") === "unknown" && second.startAgent("fix", "", "email") === "unknown", "unknown handoff kind or mode")) return
                if (!verify(second.agentName === "Pi", "default agent name")) return
                switchBack.running = true
                stage = 20
            } else if (stage === 20) {
                if (!verify(!second.stale, "theme restored")) return
                second.open()
                if (!verify(second.startAgent("fix", "readability.bar", "window") === "started", "fixRule handoff starts")) return
                if (!verify(second.agentBusy && second.startAgent("fix", "", "copy") === "busy", "a running handoff answers busy at once")) return
                stage++
            } else if (stage === 21) {
                if (!verify(!second.agentBusy && second.agentMessage === "Opened Pi", "launch result: " + second.agentMessage)) return
                if (!verify(!second.opened && !first.opened, "opening the agent closes the panel")) return
                second.open()
                if (!verify(second.startAgent("explain", "", "preview") === "started", "preview starts")) return
                stage++
            } else if (stage === 22) {
                if (!verify(second.agentPreview === "PROMPT explain\\n" && second.agentMessage === "", "preview shows the analyzer's prompt")) return
                if (!verify(JSON.parse(second.statusJson()).agent === "pi", "status names the default agent")) return
                if (!verify(second.startAgent("fix", "", "copy") === "started", "copy starts")) return
                stage++
            } else if (stage === 23) {
                if (!verify(second.agentMessage === "Prompt copied", "copy result: " + second.agentMessage)) return
                if (!verify(second.opened, "preview and copy keep the panel open")) return
                if (!verify(second.reviewText.indexOf("Saved review") === 0 && second.reviewMeta && second.reviewMeta.agent === "pi" && namedItem(second, "reportStub").reviewAgentName === "Pi", "a saved agent review reaches the panel")) return
                if (!verify(second.startAgent("review", "", "window") === "started", "review handoff starts")) return
                stage = 24
            } else if (stage === 24) {
                if (!verify(second.agentMessage === "Opened Pi" && !second.opened, "review opens the agent and closes the panel")) return
                hideHelper.running = true
                stage = 11
            } else if (stage === 11) {
                if (!verify(second.startCheck(false) === "started", "check without the analyzer starts")) return
                stage++
            } else if (stage === 12) {
                var failed = JSON.parse(second.statusJson())
                if (!verify(!second.checking && !first.checking && failed.checking === false && String(failed.error).indexOf("Could not start") === 0, "missing analyzer clears busy and reports an error")) return
                console.log("PASS: startup idle, open-only audits, explicit Refresh, shared cached result, stale badge, no queued work, close cancellation, IPC ownership/routing/status/check/agent")
                Qt.quit()
            }
        }
    }
    Timer { id: cancellation; interval: 100; onTriggered: first.close() }
}
''')
    cache = base / "cache/theme-report"
    cache.mkdir(parents=True)
    (cache / "reviews").mkdir()
    (cache / "reviews/fixture.md").write_text("Saved review")
    (cache / "reviews/fixture.json").write_text('{"agent": "pi", "requestedAt": 1}')
    (cache / "current.json").write_text("{}")
    runtime = base / "runtime"
    runtime.mkdir(mode=0o700)
    env = dict(os.environ, HOME=str(home), XDG_CACHE_HOME=str(base / "cache"),
               XDG_RUNTIME_DIR=str(runtime), QT_QPA_PLATFORM="offscreen")
    result = subprocess.run(["quickshell", "-p", str(base)], env=env, text=True,
                            capture_output=True, timeout=20)
    output = result.stdout + result.stderr
    assert result.returncode == 0 and "PASS: startup" in output and "FAIL:" not in output, output
    counts = (home / "audit-count").read_text().splitlines()
    # Exactly four panel audits before IPC, then exactly one background check (the busy call starts nothing).
    assert counts[:counts.index("ipc")] == ["audit"] * 4, output
    assert counts[counts.index("ipc") + 1:counts.index("ipc-panel")] == ["audit"], counts
    # Each handoff ran the analyzer once with exact arguments; none of them audited.
    assert (home / "agent-log").read_text().splitlines() == ["agent fix --window --rule readability.bar", "prompt explain", "prompt fix", "agent review --window"], (home / "agent-log").read_text()
    main = (ROOT / "Main.qml").read_text()
    assert not any(token in main for token in
                   ["Component.onCompleted", "Timer", "queued", "freshnessExpiry"]), "Automatic audits must remain absent"
    # File views only watch: they never read a file themselves and never audit. Every
    # read goes through the analyzer's read-only `state` command.
    for block in main.split("FileView {")[1:]:
        body = block.split("\n    }")[0]
        assert "preload: false" in body and "onFileChanged: root.readState()" in body, body
        assert "request(" not in body and "running = true" not in body and "text()" not in body, body
    assert "text()" not in main and ".reload()" not in main, "The panel must not read files directly"
    states = (home / "state-log").read_text().splitlines()
    assert states and all(line.split()[0] == "state" for line in states), states
    print("PASS: open-only audit regression and IPC (real Process, two widgets, exact invocation counts, cancellation, single IPC owner, focused routing, agent handoffs)")
