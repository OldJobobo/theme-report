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
    readonly property string home: Quickshell.env("HOME")
    readonly property string status: audit.running ? "Updating" : error ? "Analyzer error" : report ? (report.grade ? report.grade + (report.state === "provisional" ? " · Provisional" : "") : "Not graded") : "Not audited"

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
                if (data.schemaVersion !== 2 || data.target !== "current" || !Array.isArray(data.findings) || !Array.isArray(data.categories)) throw new Error("Unsupported report format")
                root.report = data
                root.error = ""
            } catch (e) {
                root.error = String(e.message || e) + (root.diagnostic ? " · " + root.diagnostic.trim() : "")
            }
        }
    }
    WidgetButton {
        id: badge
        bar: root.bar
        foreground: root.barForeground
        text: root.report && root.report.grade ? root.report.grade + (root.report.state === "provisional" ? "*" : "") : "?"
        tooltipText: "Theme Report · " + (audit.running ? "Checking…" : root.report ? "Last check: " + root.status : "Open to check") + (root.report ? "\n" + root.report.theme + "\n" + root.report.summary : "")
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
