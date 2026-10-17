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
    implicitWidth: badge.implicitWidth
    implicitHeight: badge.implicitHeight

    property var report: null
    property string error: ""
    property string output: ""
    property string diagnostic: ""
    property string currentTheme: ""
    readonly property string home: Quickshell.env("HOME")
    readonly property string cacheHome: Quickshell.env("XDG_CACHE_HOME") || home + "/.cache"
    // The active theme is no longer the one the shown result describes.
    readonly property bool stale: !!report && currentTheme !== "" && report.theme !== currentTheme
    readonly property string status: audit.running ? "Updating" : error ? "Analyzer error" : report ? (report.grade ? report.grade + (report.state === "provisional" ? " · Provisional" : "") : "Not graded") : "Not audited"

    function accepted(data) {
        return !!data && data.state !== "error" && data.schemaVersion === 2 && data.target === "current" && Array.isArray(data.findings) && Array.isArray(data.categories)
    }

    function request(force) {
        // Only an open panel may audit. Never queue work for a closed panel.
        if (!opened || audit.running) return
        output = ""
        diagnostic = ""
        error = ""
        audit.command = [home + "/.local/bin/theme-report", "audit", "--current", "--format", "json"].concat(force ? ["--force"] : [])
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
        // "~" marks a result for a previous theme; "*" marks a provisional grade.
        text: root.report && root.report.grade ? (root.stale ? "~" : "") + root.report.grade + (root.report.state === "provisional" ? "*" : "") : "?"
        tooltipText: "Theme Report · " + (audit.running ? "Checking…" : !root.report ? "Open to check" : root.stale ? "Theme changed · open to check" : "Last check: " + root.status) + (root.report ? "\n" + (root.stale ? "Previous theme: " : "") + root.report.theme + "\n" + root.report.summary : "")
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
