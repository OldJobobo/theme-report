import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import qs.Ui
import qs.Commons

Panel {
    id: root
    moduleName: "oldjobobo.theme-report"
    ipcTarget: "oldjobobo.theme-report"
    // The bar creates one copy of this widget per monitor. The base handler would make
    // every copy claim the same target; the plugin-wide handler below has one owner.
    manageIpc: false
    implicitWidth: badge.implicitWidth
    implicitHeight: badge.implicitHeight

    property var report: null
    property string error: ""
    property string output: ""
    property string diagnostic: ""
    property string currentTheme: ""
    property bool ipcOwner: false
    // A check started over IPC by another copy; shown on every badge.
    property bool sharedChecking: false
    property string headlessOutput: ""
    property string headlessError: ""
    // Process.running only turns true on a later tick; this answers "busy" immediately.
    property bool headlessActive: false
    readonly property bool checking: audit.running || headlessActive || sharedChecking
    readonly property string home: Quickshell.env("HOME")
    readonly property string cacheHome: Quickshell.env("XDG_CACHE_HOME") || home + "/.cache"
    readonly property string analyzer: home + "/.local/bin/theme-report"
    // The active theme is no longer the one the shown result describes.
    readonly property bool stale: isStale(report)
    readonly property string status: audit.running ? "Updating" : error ? "Analyzer error" : report ? (report.grade ? report.grade + (report.state === "provisional" ? " · Provisional" : "") : "Not graded") : "Not audited"
    readonly property var categories: ["validity", "readability", "semantic", "shell", "wallpaper"]

    function accepted(data) {
        return !!data && data.state !== "error" && data.schemaVersion === 2 && data.target === "current" && Array.isArray(data.findings) && Array.isArray(data.categories)
    }
    function isStale(r) {
        return !!r && currentTheme !== "" && r.theme !== currentTheme
    }
    // "~" marks a result for a previous theme; "*" marks a provisional grade.
    function badgeFor(r) {
        return r && r.grade ? (isStale(r) ? "~" : "") + r.grade + (r.state === "provisional" ? "*" : "") : "?"
    }
    function auditCommand(force) {
        return [analyzer, "audit", "--current", "--format", "json"].concat(force ? ["--force"] : [])
    }

    function request(force) {
        // Only an open panel audits on its own. Never queue work for a closed panel.
        if (!opened || audit.running) return
        output = ""
        diagnostic = ""
        error = ""
        audit.command = auditCommand(force)
        audit.running = true
    }
    onOpenedChanged: {
        if (opened) request(false)
        else audit.running = false
    }
    Process {
        id: audit
        stdout: StdioCollector { waitForEnd: true; onStreamFinished: root.output = text }
        stderr: StdioCollector { waitForEnd: true; onStreamFinished: root.diagnostic = text }
        onRunningChanged: if (!running) Qt.callLater(function() {
            if (root.opened && !audit.running && !root.output) {
                root.error = "Could not start the analyzer. Check ~/.local/bin/theme-report and refresh."
            }
        })
        onExited: function(exitCode, exitStatus) {
            if (!root.opened) return
            try {
                var data = JSON.parse(root.output)
                if (exitCode !== 0 || data.state === "error") throw new Error(data.error || "Audit process failed")
                if (!root.accepted(data)) throw new Error("Unsupported report format")
                root.report = data
                root.error = ""
            } catch (e) {
                root.error = String(e.message || e) + (root.diagnostic ? " · " + root.diagnostic.trim() : "")
            }
        }
    }

    // ------------------------------------------------------------------ IPC
    // Every live copy of this widget, in the bar's own order (null if the host cannot list them).
    function peers() {
        var list = root.bar && typeof root.bar.moduleWidgets === "function" ? root.bar.moduleWidgets(root.moduleName) : null
        if (!Array.isArray(list)) return null
        return list.filter(function(peer) {
            try { return !!peer && peer.ipcTarget === root.ipcTarget } catch (e) { return false }
        })
    }
    function everyPeer() {
        var list = peers()
        return list && list.length ? list : [root]
    }
    // The first listed copy owns the IPC target; all copies compute the same answer.
    function electIpc() {
        if (!root.bar) { ipcOwner = false; return }
        var list = peers()
        ipcOwner = !list || list.length === 0 || list[0] === root
    }
    function electAll() {
        everyPeer().forEach(function(peer) { if (typeof peer.electIpc === "function") peer.electIpc() })
    }
    onBarChanged: Qt.callLater(root.electAll)
    onModuleNameChanged: Qt.callLater(root.electAll)
    Component.onDestruction: {
        var others = (peers() || []).filter(function(peer) { return peer !== root })
        ipcOwner = false
        others.forEach(function(peer) {
            Qt.callLater(function() { try { peer.electAll() } catch (e) {} })
        })
    }

    function openedPeer() {
        var list = everyPeer()
        for (var i = 0; i < list.length; i++) if (list[i].opened) return list[i]
        return null
    }
    function latestReport() {
        var best = null
        everyPeer().forEach(function(peer) {
            var r = peer.report
            if (r && (!best || (r.auditTimestamp || 0) > (best.auditTimestamp || 0))) best = r
        })
        return best
    }
    // Open on the focused monitor through the shell's own routing; returns the open copy.
    function summonPanel() {
        var existing = openedPeer()
        if (existing) return existing
        var api = root.bar ? root.bar.shell : null
        var routed = false
        try { routed = !!api && typeof api.summon === "function" && api.summon(root.moduleName, "{}") === true } catch (e) {}
        if (!routed) root.open()
        return openedPeer() || root
    }
    function hidePanel() {
        everyPeer().forEach(function(peer) { if (peer.opened) peer.close() })
    }
    function togglePanel() {
        if (openedPeer()) {
            hidePanel()
            return "closed"
        }
        summonPanel()
        return "opened"
    }
    function openCategory(category) {
        var id = String(category || "")
        if (categories.indexOf(id) < 0) return "unknown"
        summonPanel().showCategory(id)
        return "ok"
    }
    function showCategory(id) {
        content.selectCategory(id)
    }
    // Explicit checks: an open panel re-checks in place; otherwise check without opening.
    function startCheck(force) {
        var shown = openedPeer()
        if (shown) {
            if (shown.checking) return "busy"
            shown.request(force)
            return "started"
        }
        if (everyPeer().some(function(peer) { return peer.checking })) return "busy"
        headlessOutput = ""
        headlessError = ""
        setHeadlessActive(true)
        headless.command = auditCommand(force)
        headless.running = true
        return "started"
    }
    function statusJson() {
        var r = latestReport()
        var shown = openedPeer()
        function count(test) { return r ? r.findings.filter(test).length : 0 }
        return JSON.stringify({
            theme: r ? r.theme : null,
            activeTheme: currentTheme || null,
            badge: badgeFor(r),
            grade: r && r.grade ? r.grade : null,
            state: r ? r.state : "not-audited",
            stale: isStale(r),
            summary: r ? r.summary : "",
            problems: count(function(f) { return !f.policySuppressed && f.severity === "failure" }),
            cautions: count(function(f) { return !f.policySuppressed && f.severity === "caution" }),
            limitations: count(function(f) { return f.policySuppressed || f.severity === "unassessed" }),
            runtime: r && r.runtimeContract ? r.runtimeContract.status : null,
            checkedAt: r ? r.auditTimestamp : null,
            analyzerVersion: r ? r.analyzerVersion : null,
            checking: everyPeer().some(function(peer) { return peer.checking }),
            open: !!shown,
            error: (shown && shown.error) || headlessError || null
        })
    }
    function setHeadlessActive(active) {
        headlessActive = active
        everyPeer().forEach(function(peer) { if (peer !== root) peer.sharedChecking = active })
    }
    Process {
        id: headless
        stdout: StdioCollector { waitForEnd: true; onStreamFinished: root.headlessOutput = text }
        // A process that cannot start reports only running=false, never exited.
        onRunningChanged: if (!running) Qt.callLater(function() {
            if (!root.headlessActive || headless.running) return
            if (!root.headlessOutput && !root.headlessError) root.headlessError = "Could not start the analyzer. Check ~/.local/bin/theme-report."
            root.setHeadlessActive(false)
        })
        onExited: function(exitCode, exitStatus) {
            try {
                var data = JSON.parse(root.headlessOutput)
                if (exitCode !== 0 || data.state === "error") throw new Error(data.error || "Audit process failed")
                if (!root.accepted(data)) throw new Error("Unsupported report format")
                everyPeer().forEach(function(peer) { peer.report = data })
            } catch (e) {
                root.headlessError = String(e.message || e) || "Could not run the analyzer"
            }
        }
    }
    ShellIpc {
        enabled: root.ipcOwner
        target: root.ipcTarget

        // Every method answers with a string (qmllint also crashes on `: void` here).
        function open(): string { root.summonPanel(); return "opened" }
        function show(): string { root.summonPanel(); return "opened" }
        function close(): string { root.hidePanel(); return "closed" }
        function hide(): string { root.hidePanel(); return "closed" }
        function toggle(): string { return root.togglePanel() }
        function openCategory(category: string): string { return root.openCategory(category) }
        function check(): string { return root.startCheck(false) }
        function refresh(): string { return root.startCheck(true) }
        function status(): string { return root.statusJson() }
        function grade(): string { return root.badgeFor(root.latestReport()) }
    }

    // Reading the last published result is not an audit: it restores the badge after a
    // shell restart and picks up results from the optional theme-set hook.
    FileView {
        path: root.cacheHome + "/theme-report/current.json"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            if (audit.running) return
            try {
                var data = JSON.parse(text())
                if (root.accepted(data)) root.report = data
            } catch (e) {}
        }
    }
    FileView {
        path: root.home + "/.local/state/omarchy/current/theme.name"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: root.currentTheme = text().trim()
    }
    WidgetButton {
        id: badge
        bar: root.bar
        foreground: root.barForeground
        text: root.badgeFor(root.report)
        tooltipText: "Theme Report · " + (root.checking ? "Checking…" : !root.report ? "Open to check" : root.stale ? "Theme changed · open to check" : "Last check: " + root.status) + (root.report ? "\n" + (root.stale ? "Previous theme: " : "") + root.report.theme + "\n" + root.report.summary : "")
        onPressed: root.toggle()
    }
    KeyboardPanel {
        id: panel
        owner: root
        anchorItem: badge
        bar: root.bar
        open: root.opened
        contentWidth: panel.fittedContentWidth(Style.space(560))
        contentHeight: panel.cappedContentHeight(Style.space(680))
        focusTarget: content
        ReportPanel {
            id: content
            anchors.fill: parent
            report: root.report
            status: root.status
            errorText: root.error
            refreshing: audit.running
            onRefreshRequested: root.request(true)
            onCloseRequested: root.close()
        }
    }
}
