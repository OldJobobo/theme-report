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
    readonly property string missingAnalyzer: "The Theme Report analyzer is not installed. In a terminal, run:\nbash ~/.config/omarchy/plugins/oldjobobo.theme-report/scripts/install.sh"
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
        // Re-reading the saved state is not an audit; an agent may have written a
        // review since the panel last opened.
        if (opened) readState()
    }
    Process {
        id: audit
        stdout: StdioCollector { waitForEnd: true; onStreamFinished: root.output = text }
        stderr: StdioCollector { waitForEnd: true; onStreamFinished: root.diagnostic = text }
        onRunningChanged: if (!running) Qt.callLater(function() {
            if (root.opened && !audit.running && !root.output) {
                root.error = root.missingAnalyzer
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
            agent: root.agentId || null,
            agentMessage: root.agentMessage || null,
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
            if (!root.headlessOutput && !root.headlessError) root.headlessError = root.missingAnalyzer
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
        // Agent handoffs: "started", "busy", "no-report" or "stale". The result
        // (launched / copied / no-agent / an error) shows in the panel and in status.
        function askAgent(kind: string): string { return root.startAgent(kind, "", "window") }
        function fixRule(ruleId: string): string { return root.startAgent("fix", ruleId, "window") }
        function copyPrompt(kind: string): string { return root.startAgent(kind, "", "copy") }
    }

    // ------------------------------------------------------------------ agent
    // Prompts are built by the analyzer from the cached report; the panel never writes them.
    property string agentId: ""
    readonly property string agentName: agentLabel(agentId)
    property string agentMessage: ""
    property string agentPreview: ""
    property string agentMode: ""
    property string agentOutput: ""
    property string agentError: ""
    property int agentExit: -1
    // Process.running turns true on a later tick; this answers "busy" immediately.
    property bool agentBusy: false
    function agentLabel(id) {
        return {pi:"Pi", omp:"Oh My Pi", opencode:"OpenCode", ori:"Ori", claude:"Claude Code", codex:"Codex", grok:"Grok", openclaw:"OpenClaw", agy:"Antigravity", hermes:"Hermes", copilot:"Copilot", crush:"Crush", "cursor-agent":"Cursor", muse:"Muse", kimi:"Kimi"}[id] || (id ? id : "")
    }
    // mode: "window" opens the agent, "copy" copies the prompt, "preview" shows it in the panel.
    function startAgent(kind, rule, mode) {
        if (["fix", "measure", "explain", "review"].indexOf(kind) < 0 || ["window", "copy", "preview"].indexOf(mode) < 0) return "unknown"
        if (agentRun.running) return "busy"
        var r = latestReport()
        if (!r) return "no-report"
        if (isStale(r)) return "stale"
        var args = mode === "window" ? [analyzer, "agent", kind, "--window"] : [analyzer, "prompt", kind]
        if (rule) args = args.concat(["--rule", String(rule)])
        agentMode = mode
        agentOutput = ""
        agentError = ""
        agentExit = -1
        agentBusy = true
        agentMessage = mode === "window" ? "Opening " + (agentName || "the agent picker") + "…" : "Preparing the prompt…"
        agentRun.command = args
        agentRun.running = true
        return "started"
    }
    function finishAgent() {
        if (!agentBusy || agentRun.running) return
        agentBusy = false
        var out = agentOutput.trim()
        if (agentExit !== 0 || !out) {
            agentMessage = agentExit < 0 && !agentError ? missingAnalyzer : (agentError.trim().replace(/^Theme Report: /, "") || "Could not prepare the prompt.")
        } else if (agentMode === "copy") {
            Quickshell.clipboardText = agentOutput
            agentMessage = "Prompt copied"
        } else if (agentMode === "preview") {
            agentPreview = agentOutput
            agentMessage = ""
        } else {
            agentMessage = out === "launched" ? "Opened " + agentName : out === "no-agent" ? "Choose a default agent, then try again" : out
            // The agent window (or Omarchy's agent picker) takes over; get the panel out of its way.
            // A refusal keeps the panel open so its reason stays readable.
            if (out === "launched" || out === "no-agent") hidePanel()
        }
    }
    Process {
        id: agentRun
        stdout: StdioCollector { waitForEnd: true; onStreamFinished: root.agentOutput = text }
        stderr: StdioCollector { waitForEnd: true; onStreamFinished: root.agentError = text }
        onExited: function(exitCode, exitStatus) { root.agentExit = exitCode }
        // A process that cannot start reports only running=false, never exited.
        onRunningChanged: if (!running) Qt.callLater(root.finishAgent)
    }
    // The latest agent review for the shown theme, and the request that asked for it.
    readonly property string reviewTheme: report && /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(String(report.theme)) ? report.theme : ""
    property string reviewText: ""
    property var reviewMeta: null
    onReviewThemeChanged: readState()

    // Saved state: the last published result (restores the badge after a shell restart
    // and picks up results from the optional theme-set hook), the active theme name, the
    // default agent and the saved review. Reading it is not an audit. The panel never
    // opens these files itself: the watchers below only notice changes, and the analyzer
    // reads the files with its bounded, no-symlink, regular-file-only reader.
    property bool stateDirty: false
    property string stateOutput: ""
    function readState() {
        if (stateRun.running) { stateDirty = true; return }
        stateDirty = false
        stateOutput = ""
        stateRun.command = [analyzer, "state"].concat(reviewTheme ? ["--theme", reviewTheme] : [])
        stateRun.running = true
    }
    function applyState() {
        var s = null
        try { s = JSON.parse(stateOutput) } catch (e) {}
        if (s && s.schemaVersion === 1) {
            currentTheme = typeof s.themeName === "string" ? s.themeName : ""
            agentId = typeof s.agent === "string" ? s.agent : ""
            // Keep the same object when nothing was published since, so open cards stay put.
            var r0 = s.report
            if (!audit.running && accepted(r0) && !(report && report.auditTimestamp === r0.auditTimestamp && report.inputFingerprint === r0.inputFingerprint && report.theme === r0.theme)) report = r0
            var r = s.review
            if (r && r.theme === reviewTheme) {
                reviewText = typeof r.text === "string" ? r.text : typeof r.problem === "string" ? r.problem : ""
                reviewMeta = r.meta && typeof r.meta === "object" ? r.meta : null
            } else {
                reviewText = ""
                reviewMeta = null
            }
        }
        if (stateDirty) Qt.callLater(readState)
    }
    Process {
        id: stateRun
        // Read once at startup; afterwards only when a watched file changes or the panel opens.
        command: [root.analyzer, "state"]
        running: true
        stdout: StdioCollector { waitForEnd: true; onStreamFinished: root.stateOutput = text }
        onRunningChanged: if (!running) Qt.callLater(root.applyState)
    }
    FileView {
        path: root.cacheHome + "/theme-report/current.json"
        preload: false
        watchChanges: true
        printErrors: false
        onFileChanged: root.readState()
    }
    FileView {
        path: root.home + "/.local/state/omarchy/current/theme.name"
        preload: false
        watchChanges: true
        printErrors: false
        onFileChanged: root.readState()
    }
    FileView {
        path: root.home + "/.config/omarchy/defaults/agent"
        preload: false
        watchChanges: true
        printErrors: false
        onFileChanged: root.readState()
    }
    FileView {
        path: root.reviewTheme ? root.cacheHome + "/theme-report/reviews/" + root.reviewTheme + ".md" : ""
        preload: false
        watchChanges: true
        printErrors: false
        onFileChanged: root.readState()
    }
    FileView {
        path: root.reviewTheme ? root.cacheHome + "/theme-report/reviews/" + root.reviewTheme + ".json" : ""
        preload: false
        watchChanges: true
        printErrors: false
        onFileChanged: root.readState()
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
            agentName: root.agentName
            agentBusy: root.agentBusy
            agentMessage: root.agentMessage
            agentPreview: root.agentPreview
            reviewText: root.reviewText
            reviewMeta: root.reviewMeta
            reviewAgentName: root.reviewMeta ? root.agentLabel(String(root.reviewMeta.agent || "")) : ""
            actionsEnabled: !root.stale && !root.checking
            onRefreshRequested: root.request(true)
            onCloseRequested: root.close()
            onAgentRequested: function(kind, rule, mode) { root.startAgent(kind, rule, mode) }
            onPreviewClosed: root.agentPreview = ""
        }
    }
}
