import QtQuick
import QtQuick.Controls
import Quickshell
import qs.Ui as Ui
import qs.Commons
import qs.Commons as Theme

// A report card: the overall grade up top, then one card per subject with its
// checks listed inline. Subjects that need work open on their own, so problems
// are on screen without hunting for them.
FocusScope {
    id: root
    property var report: null
    property string status: "Not audited"
    property string errorText: ""
    property bool refreshing: false
    // The subject named over IPC (openCategory); drawn with a highlight.
    property string selectedCategory: ""
    property bool showMethod: false
    // Explicit open/closed choices per subject; unset subjects open when they have issues.
    property var openCategories: ({})
    property var passedOpen: ({})
    property var expandedRules: ({})
    property var technicalRules: ({})
    property string viewedTarget: ""
    property string copiedPaletteColor: ""
    property string focusedPaletteColor: ""
    property string pendingCategory: ""
    // Agent handoff state, owned by Main.qml.
    property string agentName: ""
    property bool agentBusy: false
    property string agentMessage: ""
    property string agentPreview: ""
    property bool actionsEnabled: true
    // The latest saved agent review for this theme (Markdown) and its request record.
    property string reviewText: ""
    property var reviewMeta: null
    property string reviewAgentName: ""
    property bool reviewOpen: false
    readonly property bool reviewPending: !!reviewMeta && reviewText === ""
    readonly property bool reviewOutdated: !!reviewMeta && !!report && !!reviewMeta.inputFingerprint && reviewMeta.inputFingerprint !== report.inputFingerprint
    // The handoff result shown under the actions; it fades after a few seconds.
    property string agentNote: ""
    onAgentMessageChanged: {
        agentNote = agentMessage
        if (agentMessage && !agentMessage.endsWith("…")) agentNoteExpiry.restart()
        else agentNoteExpiry.stop()
    }
    Timer { id: agentNoteExpiry; interval: 4500; onTriggered: root.agentNote = "" }
    readonly property var paletteEvidence: report && Array.isArray(report.findings) ? report.findings.find(function(f) { return f.ruleId === "validity.roles" }) : null
    readonly property var paletteColors: paletteEvidence && paletteEvidence.evidence ? paletteEvidence.evidence.paletteColors || paletteEvidence.evidence.resolvedColors || ({}) : ({})
    readonly property var paletteBanks: paletteRows()
    readonly property var paletteRoles: [{key:"background",label:"Base"},{key:"foreground",label:"Text"},{key:"accent",label:"Accent"},{key:"urgent",label:"Urgent"}].filter(function(role) { return root.validPaletteColor(root.paletteColors[role.key]) })
    // ShellColor is the current name; Color supports the earlier Quattro runtime.
    readonly property var themeColors: typeof Theme.ShellColor !== "undefined" ? Theme.ShellColor : Theme.Color
    readonly property color ink: themeColors.popups.text
    readonly property color muted: Util.alpha(ink, 0.7)
    readonly property color faint: Util.alpha(ink, 0.45)
    readonly property color surface: Style.normalFillFor(ink, accent)
    readonly property color background: themeColors.popups.background
    readonly property color accent: themeColors.accent
    // Cautions need their own hue: many themes (and Omarchy's default) use one color for
    // accent and text. Prefer the theme's own yellow when it is readable on the popup.
    readonly property color warning: {
        var best = null, bestContrast = 3
        ;["yellow", "color3", "color11"].forEach(function(key) {
            if (!root.validPaletteColor(root.paletteColors[key])) return
            var candidate = root.hexColor(root.paletteColors[key])
            var c = root.contrastOf(candidate, root.background)
            if (c >= bestContrast) { best = candidate; bestContrast = c }
        })
        return best || Qt.tint(themeColors.urgent, Qt.rgba(ink.r, ink.g, ink.b, 0.35))
    }
    readonly property color failure: themeColors.urgent
    readonly property string bodyFont: Style.font.family
    // A quick cache-backed check should not flash "Previous result" on every open.
    property bool slowRefresh: false
    readonly property bool previous: (refreshing && slowRefresh) || errorText !== ""
    readonly property var contract: report && report.runtimeContract ? report.runtimeContract : null
    readonly property bool runtimePaused: !!contract && (contract.status === "partial" || contract.status === "unsupported")
    // Nerd Font glyphs, the same family the shell's own icons use.
    readonly property var glyphs: ({failure:"", caution:"", unassessed:"", note:"", pass:"", open:"", closed:"", agent:"\uf0d0", copy:"\uf0c5", preview:"\uf06e", review:"\uf0f6"})
    onRefreshingChanged: {
        slowRefresh = false
        if (refreshing) slowRefreshTimer.restart()
        else slowRefreshTimer.stop()
    }
    Timer { id: slowRefreshTimer; interval: 350; onTriggered: root.slowRefresh = true }
    signal refreshRequested()
    signal closeRequested()
    signal agentRequested(string kind, string rule, string mode)
    signal previewClosed()

    component EvidenceVisual: Column {
        id: visual
        required property var finding
        readonly property var evidence: finding.evidence || ({})
        readonly property bool contrast: typeof evidence.contrast === "number" && isFinite(evidence.contrast)
        readonly property bool distance: typeof evidence.oklabDistance === "number" && isFinite(evidence.oklabDistance)
        readonly property real measured: contrast ? evidence.contrast : distance ? evidence.oklabDistance : 0
        readonly property real target: typeof evidence.required === "number" && isFinite(evidence.required) ? evidence.required : 0
        readonly property real minimum: contrast ? 1 : 0
        readonly property real maximum: contrast ? 21 : Math.max(0.1, measured, evidence.required || 0)
        readonly property color cue: root.severityColor(finding)
        spacing: Style.space(10)
        Row {
            width: parent.width; spacing: Style.space(10)
            visible: !!visual.evidence.foreground && !!visual.evidence.effectiveBackground
            Rectangle {
                width: Style.space(96); height: Style.space(54); radius: Style.space(5)
                color: root.safeColor(visual.evidence.effectiveBackground)
                border.width: 1; border.color: Util.alpha(root.ink, 0.12)
                Text { anchors.centerIn: parent; visible: visual.finding.ruleId.indexOf("readability.") === 0 || visual.finding.ruleId.indexOf("wallpaper.") === 0; text: "Aa"; color: root.safeColor(visual.evidence.foreground); font.family: root.bodyFont; font.pixelSize: Style.font.heading * 1.4; font.weight: Font.DemiBold }
                Rectangle { anchors.centerIn: parent; visible: !(visual.finding.ruleId.indexOf("readability.") === 0 || visual.finding.ruleId.indexOf("wallpaper.") === 0); width: Style.space(32); height: width; radius: Style.space(4); color: "transparent"; border.width: Style.space(3); border.color: root.safeColor(visual.evidence.foreground) }
            }
            Text { width: parent.width - Style.space(106); anchors.verticalCenter: parent.verticalCenter; text: "Foreground " + visual.evidence.foreground + "\nBackground " + visual.evidence.effectiveBackground + "\nRounded composite preview"; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
        }
        Row {
            width: parent.width; spacing: Style.space(8); visible: !!visual.evidence.normalColor && !!visual.evidence.stateColor
            Repeater {
                model: [{name:"Normal",color:visual.evidence.normalColor},{name:visual.finding.ruleId === "shell.focus-fill" ? "Focused" : "Selected",color:visual.evidence.stateColor}]
                delegate: Column {
                    required property var modelData
                    width: (visual.width - Style.space(8)) / 2; spacing: Style.space(5)
                    Rectangle { width: parent.width; height: Style.space(32); radius: Style.space(4); color: root.safeColor(modelData.color); border.width: 1; border.color: Util.alpha(root.ink, 0.12) }
                    Text { width: parent.width; text: modelData.name + " · " + modelData.color; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                }
            }
        }
        Column {
            width: parent.width; spacing: Style.space(6); visible: visual.contrast || visual.distance
            Item {
                width: parent.width; height: measuredLabel.implicitHeight
                Text { id: measuredLabel; text: visual.contrast ? Number(visual.measured).toFixed(2) + ":1 contrast" : Number(visual.measured).toFixed(3) + " separation"; color: visual.cue; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; font.weight: Font.DemiBold }
                Text { anchors.right: parent.right; text: "target " + visual.evidence.required + (visual.contrast ? ":1" : ""); color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
            }
            Item {
                width: parent.width; height: Style.space(16)
                Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width; height: Style.space(6); radius: height / 2; color: Util.alpha(root.ink, 0.1) }
                Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width * Math.max(0, Math.min(1, (visual.measured - visual.minimum) / (visual.maximum - visual.minimum))); height: Style.space(6); radius: height / 2; color: visual.cue }
                Rectangle { x: Math.max(0, Math.min(parent.width - width, parent.width * (visual.target - visual.minimum) / (visual.maximum - visual.minimum))); width: Style.space(2); height: parent.height; color: root.ink }
            }
            Text { width: parent.width; text: visual.contrast ? "Scale 1:1 to 21:1 · the marker is the target" : "Scale 0 to " + Number(visual.maximum).toFixed(3) + " · the marker is the target"; color: root.faint; font.family: root.bodyFont; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap }
        }
        Flow {
            width: parent.width; spacing: Style.space(12); visible: !!visual.evidence.resolvedColors
            Repeater {
                model: Object.keys(visual.evidence.resolvedColors || ({}))
                delegate: Row {
                    required property string modelData
                    spacing: Style.space(6)
                    Rectangle { width: Style.space(20); height: width; radius: Style.space(3); color: root.safeColor(visual.evidence.resolvedColors[modelData]); border.width: 1; border.color: root.muted }
                    Text { anchors.verticalCenter: parent.verticalCenter; text: modelData; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
                }
            }
        }
        Column {
            width: parent.width; spacing: Style.space(8); visible: !!visual.evidence.thumbnails && Object.keys(visual.evidence.thumbnails).length > 0
            Text { width: parent.width; text: "Sampled wallpaper colors · 8×8 grids, not full previews"; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
            Flow {
                width: parent.width; spacing: Style.space(12)
                Repeater {
                    model: Object.keys(visual.evidence.thumbnails || ({})).slice(0,16)
                    delegate: Column {
                        id: tile
                        required property string modelData
                        readonly property var samples: visual.evidence.thumbnails[modelData].sampleColors || []
                        width: Math.min(Style.space(130), (visual.width - Style.space(12)) / 2); spacing: Style.space(5)
                        Grid {
                            width: tile.width; height: width * 9 / 16; columns: 8
                            Repeater { model: tile.samples.slice(0,64); delegate: Rectangle { required property string modelData; width: tile.width / 8; height: width * 9 / 16; color: root.safeColor(modelData) } }
                        }
                        Text { width: parent.width; text: tile.modelData.replace(/^backgrounds\//, ""); textFormat: Text.PlainText; elide: Text.ElideMiddle; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
                    }
                }
            }
        }
    }

    // One check inside a subject: a two-line row that opens into its evidence.
    component CheckRow: Column {
        id: check
        required property var finding
        readonly property bool expanded: root.expandedRules[finding.ruleId] === true
        readonly property bool technical: root.technicalRules[finding.ruleId] === true
        readonly property color cue: root.severityColor(finding)
        spacing: 0
        Ui.Button {
            id: checkButton
            width: parent.width; height: checkLines.implicitHeight + Style.space(14)
            text: ""; foreground: root.ink; accent: root.accent; focusable: true; leftAlign: true
            Accessible.name: root.title(check.finding.ruleId) + ", " + root.severityLabel(check.finding) + (check.expanded ? ", expanded" : ", collapsed")
            onActiveFocusChanged: if (activeFocus) root.reveal(check)
            onClicked: root.toggleDetail(check.finding.ruleId, false)
            Text {
                id: checkGlyph
                x: Style.space(10); y: Style.space(7)
                width: Style.space(18)
                text: root.glyphs[root.severityKey(check.finding)]
                color: check.cue; font.family: root.bodyFont; font.pixelSize: Style.font.body
            }
            Column {
                id: checkLines
                x: checkGlyph.x + checkGlyph.width + Style.space(6); y: Style.space(7)
                width: parent.width - x - Style.space(30)
                spacing: Style.space(3)
                Text { width: parent.width; text: root.title(check.finding.ruleId); textFormat: Text.PlainText; color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.body; font.weight: Font.DemiBold; wrapMode: Text.Wrap }
                Text { width: parent.width; text: root.shortReason(check.finding); textFormat: Text.PlainText; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
            }
            Text { anchors.right: parent.right; anchors.rightMargin: Style.space(10); y: Style.space(7); text: check.expanded ? root.glyphs.open : root.glyphs.closed; color: root.faint; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
        }
        // Details hang under the title, behind a rule in the check's own color.
        Item {
            visible: check.expanded
            width: parent.width
            height: visible ? detailColumn.implicitHeight + Style.space(16) : 0
            Rectangle { x: Style.space(18); y: Style.space(2); width: Style.space(2); height: parent.height - Style.space(12); radius: 1; color: Util.alpha(check.cue, 0.5) }
            Column {
                id: detailColumn
                x: Style.space(34); y: Style.space(4); width: parent.width - x - Style.space(12)
                spacing: Style.space(10)
                EvidenceVisual { width: parent.width; finding: check.finding }
                Text { width: parent.width; text: root.context(check.finding); color: root.faint; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                Column {
                    width: parent.width; spacing: Style.space(4)
                    Text { width: parent.width; text: check.finding.evidence && check.finding.evidence.referenceDefectOwnedByReadability ? "SCORING NOTE" : check.finding.severity === "pass" ? "WHY IT PASSED" : "WHAT TO DO"; color: check.cue; font.family: root.bodyFont; font.pixelSize: Style.font.caption; font.weight: Font.DemiBold; font.letterSpacing: 1.1 }
                    Text { width: parent.width; text: check.finding.severity === "pass" ? root.detail(check.finding) : check.finding.remediation; textFormat: Text.PlainText; color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                }
                Text { width: parent.width; visible: text !== ""; text: (check.finding.sources || []).map(function(s) { return s.file + (s.section ? "  [" + s.section + "]" : "") + (s.key ? "  " + s.key : "") }).join("\n"); textFormat: Text.PlainText; color: root.muted; font.family: Style.font.family; font.pixelSize: Style.font.bodySmall; wrapMode: Text.WrapAnywhere }
                Flow {
                    width: parent.width; spacing: Style.space(6)
                    visible: root.handoffKind(check.finding) !== ""
                    Ui.Button {
                        objectName: "checkAgentButton"
                        text: !root.agentName ? "Choose an agent" : root.handoffKind(check.finding) === "fix" ? "Fix with " + root.agentName : "Ask " + root.agentName
                        iconText: root.glyphs.agent
                        bordered: true; foreground: root.ink; accent: root.accent; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall
                        enabled: root.actionsEnabled && !root.agentBusy
                        tooltipText: root.handoffKind(check.finding) === "fix" ? "Open your agent with a prompt to fix only this check" : "Open your agent to find out what this check needs"
                        onActiveFocusChanged: if (activeFocus) root.reveal(this)
                        onClicked: root.agentRequested(root.handoffKind(check.finding), check.finding.ruleId, "window")
                    }
                    Ui.Button {
                        text: "Copy prompt"; iconText: root.glyphs.copy
                        foreground: root.muted; accent: root.accent; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall
                        enabled: root.actionsEnabled && !root.agentBusy
                        onActiveFocusChanged: if (activeFocus) root.reveal(this)
                        onClicked: root.agentRequested(root.handoffKind(check.finding), check.finding.ruleId, "copy")
                    }
                }
                Ui.Button { text: check.technical ? "Hide technical data" : "Technical data"; foreground: root.muted; accent: root.accent; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall; onActiveFocusChanged: if (activeFocus) root.reveal(this); onClicked: root.toggleDetail(check.finding.ruleId, true) }
                Text { visible: check.technical; width: parent.width; text: "MEASUREMENT · " + check.finding.ruleId; color: root.accent; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                Text { visible: check.technical; width: parent.width; text: JSON.stringify(check.finding.evidence, null, 2); textFormat: Text.PlainText; color: root.muted; font.family: Style.font.family; font.pixelSize: Style.font.bodySmall; wrapMode: Text.WrapAnywhere }
            }
        }
    }

    // A small count tag for the header summary: glyph, number, label.
    component CountChip: Rectangle {
        id: chip
        required property string glyph
        required property string label
        required property color tint
        implicitWidth: chipRow.implicitWidth + Style.space(16)
        implicitHeight: chipRow.implicitHeight + Style.space(8)
        radius: height / 2
        color: Util.alpha(tint, 0.14)
        border.width: 1; border.color: Util.alpha(tint, 0.3)
        Row {
            id: chipRow
            anchors.centerIn: parent; spacing: Style.space(5)
            Text { text: chip.glyph; color: chip.tint; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; anchors.verticalCenter: parent.verticalCenter }
            Text { text: chip.label; color: chip.tint; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; font.weight: Font.DemiBold; anchors.verticalCenter: parent.verticalCenter }
        }
    }

    // KeyboardPanel owns the themed popup background and its authored alpha.
    function counts() {
        if (!report) return ""
        var problems = number("problems"), cautions = number("cautions"), limits = number("unassessed")
        return problems + (problems === 1 ? " problem" : " problems") + " · " + cautions + (cautions === 1 ? " caution" : " cautions") + " · " + limits + " not measured"
    }
    function summaryChips() {
        if (!report) return []
        var list = [], n
        if ((n = number("problems"))) list.push({glyph:glyphs.failure, label:n + (n === 1 ? " problem" : " problems"), tint:failure})
        if ((n = number("cautions"))) list.push({glyph:glyphs.caution, label:n + (n === 1 ? " caution" : " cautions"), tint:warning})
        if ((n = number("unassessed"))) list.push({glyph:glyphs.unassessed, label:n + " not measured", tint:muted})
        list.push({glyph:glyphs.pass, label:number("passed") + " passed", tint:accent})
        return list
    }
    function categoryStatus(c) {
        var modeled = report && report.findings.some(function(f) { return f.category === c.id && f.method === "modeled-appearance" && f.assessedBudget > 0 })
        return c.state === "not-checked" ? "Not checked" : c.state === "not-applicable" ? "N/A" : c.state === "limited" ? "Limited" : c.lost > 0 ? "Needs attention" : modeled ? "Modeled" : "Passed"
    }
    // One line under a subject's name: what is wrong first, then what passed.
    // With `styled`, the problem and caution counts carry their own color (StyledText).
    function categorySummary(c, styled) {
        var problems = number("problems", c.id), cautions = number("cautions", c.id), limits = number("unassessed", c.id), notes = number("notes", c.id), passed = number("passed", c.id)
        if (c.state === "not-applicable") return "Does not apply to this theme"
        function tint(text, color) { return styled ? "<font color=\"" + root.hexOf(color) + "\">" + text + "</font>" : text }
        var parts = []
        if (problems) parts.push(tint(problems + (problems === 1 ? " problem" : " problems"), failure))
        if (cautions) parts.push(tint(cautions + (cautions === 1 ? " caution" : " cautions"), warning))
        if (limits) parts.push(limits + " not measured")
        if (notes) parts.push(notes + (notes === 1 ? " note" : " notes"))
        if (!parts.length) return passed ? (passed === 1 ? "Its check passed" : "All " + passed + " checks passed") : c.state === "not-checked" ? "Not checked" : "No checks"
        if (passed) parts.push(passed + " passed")
        return parts.join(" · ")
    }
    function hexOf(c) {
        function two(v) { return ("0" + Math.round(Math.max(0, Math.min(1, v)) * 255).toString(16)).slice(-2) }
        return "#" + two(c.r) + two(c.g) + two(c.b)
    }
    function categoryHint(id) {
        return {validity:"colors.toml parses, core colors resolve, shell values are in range, wallpapers decode", readability:"Text against its background across the terminal, bar, popups, menus, launcher and lock screen", semantic:"Accent, error and selection colors stand apart from the base", shell:"Selected and focused fills and popup borders are visible", wallpaper:"Lock and launcher text over your theme's own wallpapers"}[id] || ""
    }
    function gradeColor(g) {
        if (!g) return muted
        var letter = String(g).charAt(0)
        return letter === "A" || letter === "B" ? accent : letter === "C" || letter === "D" ? warning : failure
    }
    function number(group, category) {
        if (!report) return 0
        return report.findings.filter(function(f) { return belongs(f, group) && (!category || f.category === category) }).length
    }
    function belongs(f, group) {
        var shared = f.evidence && f.evidence.referenceDefectOwnedByReadability
        if (group === "problems") return !f.policySuppressed && f.severity === "failure"
        if (group === "cautions") return !f.policySuppressed && f.severity === "caution"
        return group === "issues" ? !f.policySuppressed && (f.severity === "failure" || f.severity === "caution") : group === "passed" ? !f.policySuppressed && f.severity === "pass" && !shared : group === "unassessed" ? f.severity === "unassessed" || f.policySuppressed : f.severity === "advisory" || shared
    }
    function severityKey(f) {
        if (f.policySuppressed || f.severity === "unassessed") return "unassessed"
        if (f.severity === "failure" || f.severity === "caution") return f.severity
        return f.severity === "pass" && !(f.evidence && f.evidence.referenceDefectOwnedByReadability) ? "pass" : "note"
    }
    function severityColor(f) {
        var key = severityKey(f)
        return key === "failure" ? failure : key === "caution" || key === "note" && f.evidence && f.evidence.referenceDefectOwnedByReadability ? warning : key === "pass" ? accent : muted
    }
    // Which handoff a check offers: fix its problem, or find out why it was not measured.
    function handoffKind(f) {
        if (!f || f.ruleId === "runtime.paused" || (f.evidence && f.evidence.runtimeContract)) return ""
        if (String(f.ruleId).indexOf("advisory.") === 0) return ""
        return belongs(f, "issues") ? "fix" : belongs(f, "unassessed") ? "measure" : ""
    }
    // The header's main handoff: fix every issue, else ask about unmeasured checks.
    // The header's main handoff: fix every issue, else ask about unmeasured checks,
    // else (a passing theme) a full review.
    function primaryHandoff() {
        if (!report || previous) return null
        var issues = number("issues"), limits = report.findings.filter(function(f) { return root.handoffKind(f) === "measure" }).length
        if (issues) return {kind:"fix", label:(agentName ? (issues === 1 ? "Fix 1 issue" : "Fix all " + issues + " issues") + " with " + agentName : "Choose an agent to fix " + issues + (issues === 1 ? " issue" : " issues"))}
        if (limits) return {kind:"measure", label:(agentName ? "Ask " + agentName + " about " + limits + " unmeasured" : "Choose an agent")}
        return {kind:"review", label:reviewLabel()}
    }
    function reviewLabel() {
        return !agentName ? "Choose an agent for a review" : reviewText || reviewPending ? "Review again with " + agentName : "Review with " + agentName
    }
    function promptNoun(kind) {
        return {fix:"fix", measure:"measuring", explain:"explain", review:"review"}[kind] || ""
    }
    // Agent reviews are Markdown written by the agent: show text only, no images or HTML.
    function reviewMarkdown(text) {
        return String(text).replace(/^#\s[^\n]*\n+/, "").replace(/!\[[^\]]*\]\([^)]*\)/g, "").replace(/<[^>]*>/g, "")
    }
    function reviewSummary(text) {
        var m = String(text).match(/##\s*Summary[^\n]*\n+([\s\S]*?)(\n#|$)/)
        var body = m ? m[1] : String(text).replace(/^#[^\n]*\n/, "")
        return body.replace(/[*_`>#\[\]]/g, "").replace(/\s+/g, " ").trim().slice(0, 360)
    }
    function reviewByline() {
        var who = reviewAgentName || "your agent"
        if (reviewPending) return "Waiting for " + who + " to save the review…"
        var parts = []
        if (reviewMeta) {
            parts.push("By " + who)
            var when = new Date((reviewMeta.requestedAt || 0) * 1000)
            if (reviewMeta.requestedAt) parts.push(when.toDateString() === new Date().toDateString() ? Qt.formatTime(when, "hh:mm") : Qt.formatDate(when, "d MMM"))
        }
        if (reviewOutdated) parts.push("theme changed since")
        return parts.join(" · ")
    }
    function categoryOpen(id) {
        var choice = openCategories[id]
        return choice !== undefined ? choice === true : number("issues", id) > 0
    }
    function toggleCategory(id) {
        var copy = Object.assign({}, openCategories)
        copy[id] = !categoryOpen(id)
        openCategories = copy
    }
    function togglePassed(id) {
        var copy = Object.assign({}, passedOpen)
        copy[id] = !copy[id]
        passedOpen = copy
    }
    // Open a subject and bring it into view (openCategory over IPC).
    function selectCategory(id) {
        selectedCategory = id || ""
        showMethod = false
        // Requested before any report arrived: apply it once one does.
        pendingCategory = report ? "" : selectedCategory
        if (!selectedCategory || !report) return
        var copy = Object.assign({}, openCategories)
        copy[selectedCategory] = true
        openCategories = copy
        Qt.callLater(root.revealCategory, selectedCategory)
    }
    function revealCategory(id) {
        for (var i = 0; i < subjects.count; i++) {
            var card = subjects.itemAt(i)
            if (card && card.modelData && card.modelData.id === id) {
                var p = card.mapToItem(findingsColumn, 0, 0)
                scroll.contentY = Math.max(0, Math.min(Math.max(0, scroll.contentHeight - scroll.height), p.y))
                return
            }
        }
    }
    function toggleDetail(ruleId, technical) {
        var copy = Object.assign({}, technical ? technicalRules : expandedRules)
        copy[ruleId] = !copy[ruleId]
        if (technical) technicalRules = copy
        else expandedRules = copy
    }
    function validPaletteColor(value) { return /^#[0-9a-f]{6}$/i.test(String(value)) }
    function hexColor(value) {
        var n = parseInt(String(value).slice(1), 16)
        return Qt.rgba(((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255, 1)
    }
    function luminance(c) {
        function channel(v) { return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4) }
        return 0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
    }
    function contrastOf(a, b) {
        var x = luminance(a), y = luminance(b)
        return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)
    }
    function displayName(slug) {
        return String(slug).replace(/[-_]+/g, " ").replace(/(^|\s)\S/g, function(c) { return c.toUpperCase() })
    }
    function severityLabel(f) {
        return f.severity === "failure" ? "Needs fixing" : f.severity === "caution" ? "Worth checking" : f.severity === "unassessed" ? "Not measured" : f.severity === "pass" && !(f.evidence && f.evidence.referenceDefectOwnedByReadability) ? "Passed" : "Note"
    }
    function safeColor(value) { return validPaletteColor(value) ? value : root.surface }
    function paletteRows() {
        var normal = [], bright = []
        for (var i = 0; i < 16; i++) {
            var key = "color" + i
            if (validPaletteColor(paletteColors[key])) (i < 8 ? normal : bright).push({key:key,color:paletteColors[key]})
        }
        // Semantic-only themes have no ANSI slots; show their actual colors,
        // rather than inventing a normal/bright terminal palette.
        if (!normal.length && !bright.length) {
            ["red","green","yellow","blue","magenta","cyan"].forEach(function(key) {
                if (root.validPaletteColor(root.paletteColors[key])) normal.push({key:key,color:root.paletteColors[key]})
            })
        }
        return [normal,bright].filter(function(bank) { return bank.length > 0 })
    }
    function copyPaletteColor(value) {
        if (!validPaletteColor(value)) return
        Quickshell.clipboardText = value
        copiedPaletteColor = value
        paletteCopyExpiry.restart()
    }
    Timer { id: paletteCopyExpiry; interval: 2200; onTriggered: root.copiedPaletteColor = "" }
    function context(f) {
        var e = f.evidence || {}
        return e.backgroundModel === "application-gray17-v4" ? "Reference backgrounds · " + e.backgroundSamples + " samples" : e.sampleModel ? "Wallpaper stress model · worst sampled pair" : f.method === "modeled-appearance" ? "Modeled appearance" : "Resolved theme values"
    }
    function detail(f) {
        var e = f.evidence || {}
        if (e.referenceDefectOwnedByReadability) return root.proof(f)
        if (e.contrast !== undefined) return f.method === "modeled-appearance" ? "The lowest checked contrast meets its " + e.required + ":1 target in this sampled model." : "This resolved color pair meets its " + e.required + ":1 contrast target."
        if (e.oklabDistance !== undefined) return "The checked states exceed the required color-separation threshold. Other control cues are not certified by this measurement."
        return {"validity.palette":"colors.toml passed TOML parsing without syntax errors.","validity.roles":"All four required roles resolve to supported RGB colors.","validity.shell":"No invalid values were found in known shell domains; unspecified fields use runtime defaults.","validity.backgrounds":"Every supported wallpaper decoded within bounds. Image grids show 64 sampled points each, after orientation and any required color conversion."}[f.ruleId] || f.summary
    }
    function proof(f) {
        var e = f.evidence || {}
        if (e.referenceDefectOwnedByReadability) return "Measured here; the deduction belongs to Text & contrast."
        if (e.contrast !== undefined) return Number(e.contrast).toFixed(2) + ":1 contrast · target " + e.required + ":1"
        if (e.oklabDistance !== undefined) return Number(e.oklabDistance).toFixed(3) + " color separation · target " + e.required
        // Repeater delegates see arrays as list types, so test for length, not Array.isArray.
        if (e.missingOrUnsupported && e.missingOrUnsupported.length) return "Missing or unsupported: " + Array.prototype.slice.call(e.missingOrUnsupported).join(", ") + "."
        // The canned lines below describe a pass; anything else speaks for itself.
        if (f.severity !== "pass") return f.summary
        if (f.ruleId === "validity.palette") return "Palette parsed successfully."
        if (f.ruleId === "validity.roles") return "Background, text, accent and urgent colors resolved."
        if (f.ruleId === "validity.shell") return "Known shell values are valid."
        if (f.ruleId === "validity.backgrounds" && e.decoded !== undefined) return e.decoded + (e.decoded === 1 ? " unique wallpaper sampled successfully." : " unique wallpapers sampled successfully.")
        return f.summary
    }
    // A subject's checks that are not plain passes: problems, cautions, gaps, then notes.
    function checksFor(category) {
        if (!report) return []
        var rank = {failure:0, caution:1, unassessed:2, note:3}
        var list = report.findings.filter(function(f) { return f.category === category && !belongs(f, "passed") }).sort(function(a,b) { return rank[root.severityKey(a)] - rank[root.severityKey(b)] || root.report.findings.indexOf(a) - root.report.findings.indexOf(b) })
        // Checks paused for the same runtime reason read as one row, not a wall of copies.
        var paused = list.filter(function(f) { return f.evidence && f.evidence.runtimeContract })
        if (paused.length < 2) return list
        var merged = {
            ruleId: "runtime.paused", category: category, severity: "unassessed", method: "resolved-token", policySuppressed: false,
            summary: paused[0].summary,
            evidence: { reason: paused[0].evidence.reason, pausedChecks: paused.map(function(f) { return f.ruleId }) },
            sources: [],
            remediation: paused[0].remediation + " Paused: " + paused.map(function(f) { return root.title(f.ruleId) }).join(", ") + "."
        }
        var at = list.indexOf(paused[0])
        var rest = list.filter(function(f) { return paused.indexOf(f) < 0 })
        rest.splice(at, 0, merged)
        return rest
    }
    function passedFor(category) {
        if (!report) return []
        return report.findings.filter(function(f) { return f.category === category && belongs(f, "passed") })
    }
    function categoryName(id) {
        return {validity:"Theme integrity",readability:"Text & contrast",semantic:"State distinction",shell:"Shell surfaces",wallpaper:"Wallpaper resilience"}[id] || id
    }
    function title(id) {
        return {"validity.palette":"Palette file", "validity.roles":"Required colors", "validity.shell":"Shell configuration", "validity.backgrounds":"Wallpaper media", "readability.foundation":"Primary text", "readability.selection":"Terminal selection", "readability.bar":"Status bar", "readability.popups":"Popup text", "readability.notifications":"Notifications", "readability.menu.selected":"Menu selection", "readability.launcher.selected":"Launcher selection", "readability.polkit":"Authentication prompt", "readability.lock":"Lock screen text", "readability.tooltip":"Tooltip text", "semantic.accent":"Accent distinction", "semantic.error-indicator":"Error indicator", "semantic.selection":"Selection colors", "shell.selected-fill":"Selected surface", "shell.focus-fill":"Focus surface", "shell.popup-border":"Popup boundaries", "wallpaper.lock-model":"Lock screen over wallpaper", "wallpaper.launcher-model":"Launcher over wallpaper", "advisory.lua-syntax":"Lua validation", "advisory.user-shell":"Your shell overrides", "runtime.paused":"Checks paused for this Omarchy build"}[id] || id
    }
    function shortReason(f) {
        var reason = f.evidence.reason || ""
        if (f.severity === "unassessed") {
            if (f.evidence.unsupported && f.evidence.unsupported.length) {
                if (f.evidence.unsupported.some(function(s) { return String(s).indexOf("snapshot byte limit") >= 0 })) return "At least one wallpaper exceeds the analyzer’s input limit. It stays untouched; dependent checks are not graded."
                if (f.evidence.unsupported.some(function(s) { return String(s).indexOf("wallpapers in the set") >= 0 })) return "This theme has more wallpapers than one check samples. Expand to see which were skipped."
                if (f.evidence.unsupported.some(function(s) { return String(s).indexOf("outside the theme") >= 0 })) return "A wallpaper links outside the theme folder, so it was not read."
                return "Some wallpapers could not be sampled. Expand for format, decoder, or size limits."
            }
            return reason || "The current model cannot establish this check. It does not count as a pass or a failure."
        }
        return root.proof(f)
    }
    function verdict() {
        if (!report) return refreshing ? "Reading your theme" : "No report yet"
        if (previous) return "Previous result"
        return report.grade ? (report.capReasons.length ? "Fix the foundation first" : number("issues") ? "Room to improve" : "Your theme checks out") : "Not graded"
    }
    function explanation() {
        if (!report) return "The analyzer reads your theme without changing it."
        if (previous) return "This result is retained for reference, not a current verdict."
        return report.state === "provisional" ? "Grade based on measured checks" : report.grade ? "Includes modeled background checks" : "No supported measurements"
    }
    function checkedAt() {
        if (!report || !report.auditTimestamp) return ""
        var when = new Date(report.auditTimestamp * 1000), now = new Date()
        return "Checked " + (when.toDateString() === now.toDateString() ? Qt.formatTime(when, "hh:mm") : Qt.formatDate(when, "d MMM"))
    }
    function reveal(item) {
        var p = item.mapToItem(findingsColumn, 0, 0)
        if (p.y < scroll.contentY) scroll.contentY = p.y
        else if (p.y + item.height > scroll.contentY + scroll.height) scroll.contentY = Math.min(Math.max(0, scroll.contentHeight - scroll.height), p.y + item.height - scroll.height)
    }
    onReportChanged: {
        var key = report ? report.themePath + "\n" + report.theme : ""
        if (key !== viewedTarget) {
            viewedTarget = key
            selectedCategory = ""
            showMethod = false
            openCategories = ({})
            passedOpen = ({})
            expandedRules = ({})
            technicalRules = ({})
            scroll.contentY = 0
            copiedPaletteColor = ""
            focusedPaletteColor = ""
            paletteCopyExpiry.stop()
            if (agentPreview) previewClosed()
            reviewOpen = false
        }
        if (report && pendingCategory) selectCategory(pendingCategory)
    }
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) { root.closeRequested(); event.accepted = true }
        else if (event.key === Qt.Key_PageDown || event.key === Qt.Key_Down) { scroll.contentY = Math.min(Math.max(0, scroll.contentHeight - scroll.height), scroll.contentY + Style.space(60)); event.accepted = true }
        else if (event.key === Qt.Key_PageUp || event.key === Qt.Key_Up) { scroll.contentY = Math.max(0, scroll.contentY - Style.space(60)); event.accepted = true }
        else if (event.key === Qt.Key_R && !(event.modifiers & Qt.ControlModifier)) { root.refreshRequested(); event.accepted = true }
    }

    // ------------------------------------------------------------ header
    Column {
        id: header
        width: parent.width
        spacing: Style.space(12)
        Item {
            width: parent.width; height: refreshButton.height
            Text { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "THEME REPORT"; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.caption; font.weight: Font.DemiBold; font.letterSpacing: 1.6 }
            Text { anchors.right: refreshButton.left; anchors.rightMargin: Style.space(6); anchors.verticalCenter: parent.verticalCenter; text: root.refreshing ? "Checking…" : root.checkedAt(); color: root.faint; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
            Ui.Button {
                id: refreshButton
                objectName: "refreshButton"
                anchors.right: parent.right
                iconText: ""
                iconSpinning: root.refreshing
                foreground: root.muted; accent: root.accent
                focusable: true; enabled: !root.refreshing
                tooltipText: root.refreshing ? "Checking theme…" : "Refresh"
                Accessible.name: "Refresh theme report"
                onClicked: root.refreshRequested()
            }
        }
        // The report card's masthead: a grade stamp beside the theme, verdict and tally.
        Row {
            width: parent.width
            spacing: Style.space(16)
            Rectangle {
                id: stamp
                readonly property string letter: !root.previous && root.report && root.report.grade ? root.report.grade : "—"
                readonly property color tone: root.previous ? root.muted : root.gradeColor(root.report ? root.report.grade : "")
                width: Style.space(84); height: width; radius: Style.space(14)
                color: Util.alpha(tone, 0.12)
                border.width: Style.space(2); border.color: Util.alpha(tone, 0.55)
                Text { anchors.centerIn: parent; text: stamp.letter; color: stamp.tone; font.family: root.bodyFont; font.pixelSize: Style.font.displayLarge * 1.6; font.weight: Font.Bold }
                Text { anchors.right: parent.right; anchors.top: parent.top; anchors.margins: Style.space(6); visible: !root.previous && !!root.report && root.report.state === "provisional"; text: "*"; color: stamp.tone; font.family: root.bodyFont; font.pixelSize: Style.font.heading; font.weight: Font.Bold }
            }
            Column {
                width: parent.width - stamp.width - parent.spacing
                anchors.verticalCenter: stamp.verticalCenter
                spacing: Style.space(6)
                Text { width: parent.width; text: root.report ? root.displayName(root.report.theme) : "Your theme, examined"; textFormat: Text.PlainText; color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.heading * 1.25; font.weight: Font.DemiBold; elide: Text.ElideRight }
                Column {
                    width: parent.width; spacing: Style.space(2)
                    Text { width: parent.width; text: root.verdict(); color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.body; font.weight: Font.DemiBold; wrapMode: Text.Wrap }
                    Text { width: parent.width; text: root.explanation(); color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                }
                Flow {
                    width: parent.width; spacing: Style.space(6)
                    visible: !!root.report && !root.previous
                    Repeater {
                        model: root.summaryChips()
                        delegate: CountChip { required property var modelData; glyph: modelData.glyph; label: modelData.label; tint: modelData.tint }
                    }
                }
            }
        }
        // Hand the report to the default agent: the main action, a copy, and a preview.
        Flow {
            id: agentActions
            objectName: "agentActions"
            width: parent.width; spacing: Style.space(6)
            visible: root.primaryHandoff() !== null
            // The main action reads as one: tinted with the theme's accent.
            Ui.Button {
                id: agentFixButton
                objectName: "agentFixButton"
                text: root.primaryHandoff() ? root.primaryHandoff().label : ""
                iconText: root.primaryHandoff() && root.primaryHandoff().kind === "review" ? root.glyphs.review : root.glyphs.agent
                background: Util.alpha(root.accent, 0.22)
                bordered: true; foreground: root.ink; accent: root.accent; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall
                enabled: root.actionsEnabled && !root.agentBusy
                tooltipText: !root.actionsEnabled ? "Check the current theme first" : root.primaryHandoff() && root.primaryHandoff().kind === "review" ? "Opens your agent for a thorough read-only review; it saves the write-up here" : "Opens your default agent in a new window with a prompt built from this report"
                onClicked: root.agentRequested(root.primaryHandoff().kind, "", "window")
            }
            Ui.Button {
                objectName: "agentReviewButton"
                visible: !!root.primaryHandoff() && root.primaryHandoff().kind !== "review"
                text: root.reviewLabel()
                iconText: root.glyphs.review
                bordered: true; foreground: root.ink; accent: root.accent; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall
                enabled: root.actionsEnabled && !root.agentBusy
                tooltipText: "A thorough read-only review beyond the measured checks; it saves the write-up here"
                onClicked: root.agentRequested("review", "", "window")
            }
            Ui.Button {
                objectName: "agentCopyButton"
                iconText: root.glyphs.copy
                Accessible.name: tooltipText
                foreground: root.muted; accent: root.accent; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall
                enabled: root.actionsEnabled && !root.agentBusy
                tooltipText: root.primaryHandoff() ? "Copy the " + root.promptNoun(root.primaryHandoff().kind) + " prompt" : ""
                onClicked: root.agentRequested(root.primaryHandoff().kind, "", "copy")
            }
            Ui.Button {
                objectName: "agentPreviewButton"
                iconText: root.glyphs.preview
                selected: root.agentPreview !== ""
                Accessible.name: tooltipText
                foreground: root.muted; accent: root.accent; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall
                enabled: (root.actionsEnabled && !root.agentBusy) || root.agentPreview !== ""
                tooltipText: root.agentPreview ? "Hide the prompt" : root.primaryHandoff() ? "Preview the exact " + root.promptNoun(root.primaryHandoff().kind) + " prompt the agent receives" : ""
                onClicked: {
                    if (root.agentPreview) root.previewClosed()
                    else root.agentRequested(root.primaryHandoff().kind, "", "preview")
                }
            }
            Text {
                objectName: "agentMessage"
                height: agentFixButton.height
                verticalAlignment: Text.AlignVCenter
                visible: text !== ""
                text: root.agentNote
                textFormat: Text.PlainText; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall
            }
        }
        // Errors and paused checks belong next to the verdict, not below the fold.
        Rectangle {
            width: parent.width; visible: root.errorText !== ""
            height: visible ? errorLabel.implicitHeight + Style.space(16) : 0
            radius: Style.space(6); color: Util.alpha(root.warning, 0.12)
            Rectangle { width: Style.space(3); height: parent.height; radius: 1; color: root.warning }
            Text { id: errorLabel; objectName: "errorText"; x: Style.space(14); y: Style.space(8); width: parent.width - Style.space(24); text: root.errorText; textFormat: Text.PlainText; color: root.warning; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
        }
        Rectangle {
            width: parent.width; visible: root.runtimePaused && !root.previous
            height: visible ? runtimeLabel.implicitHeight + Style.space(16) : 0
            radius: Style.space(6); color: Util.alpha(root.warning, 0.12)
            Rectangle { width: Style.space(3); height: parent.height; radius: 1; color: root.warning }
            Text {
                id: runtimeLabel
                objectName: "runtimeBanner"
                x: Style.space(14); y: Style.space(8); width: parent.width - Style.space(24)
                text: root.contract ? (root.contract.palette ? "Some checks are paused: " : "Checks are paused: ") + "this Omarchy build changed " + (root.contract.unverified || []).join(", ") + ". Update theme-report to measure them; nothing is guessed meanwhile." : ""
                textFormat: Text.PlainText; color: root.warning; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap
            }
        }
    }

    // ------------------------------------------------------------ body
    Flickable {
        id: scroll
        anchors.top: header.bottom; anchors.topMargin: Style.space(16)
        anchors.bottom: parent.bottom
        width: parent.width; clip: true
        contentWidth: width; contentHeight: findingsColumn.implicitHeight + Style.space(8)
        boundsBehavior: Flickable.StopAtBounds
        readonly property bool overflowing: contentHeight > height + 1
        ScrollBar.vertical: ScrollBar {
            id: vbar
            policy: scroll.overflowing ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
            contentItem: Rectangle { implicitWidth: Style.space(4); radius: width / 2; color: Util.alpha(root.ink, vbar.pressed || vbar.hovered ? 0.5 : 0.22) }
            background: Item {}
        }
        Column {
            id: findingsColumn
            width: scroll.width - (scroll.overflowing ? Style.space(12) : 0)
            spacing: Style.space(8)

            // The exact prompt, so nothing reaches the agent unseen.
            Rectangle {
                objectName: "agentPreviewBox"
                width: parent.width; visible: root.agentPreview !== ""
                height: visible ? previewText.implicitHeight + Style.space(20) : 0
                radius: Style.space(8); color: "transparent"
                border.width: 1; border.color: Util.alpha(root.accent, 0.4)
                Text {
                    id: previewText
                    x: Style.space(12); y: Style.space(10); width: parent.width - Style.space(24)
                    text: root.agentPreview
                    textFormat: Text.PlainText; color: root.muted; font.family: Style.font.family; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap
                }
            }
            // The agent's own write-up, when one was saved for this theme.
            Rectangle {
                id: reviewCard
                objectName: "agentReviewCard"
                width: parent.width
                visible: !!root.report && (root.reviewText !== "" || root.reviewPending)
                height: visible ? reviewColumn.implicitHeight : 0
                radius: Style.space(8); color: root.surface
                border.width: 1; border.color: Util.alpha(root.accent, root.reviewOutdated ? 0.2 : 0.45)
                Column {
                    id: reviewColumn
                    width: parent.width
                    Ui.Button {
                        width: parent.width; height: reviewHead.implicitHeight + Style.space(20)
                        text: ""; foreground: root.ink; accent: root.accent; focusable: true; leftAlign: true
                        radius: Style.space(8)
                        enabled: root.reviewText !== ""
                        Accessible.name: "Agent review" + (root.reviewOpen ? ", expanded" : ", collapsed")
                        onActiveFocusChanged: if (activeFocus) root.reveal(reviewCard)
                        onClicked: root.reviewOpen = !root.reviewOpen
                        Text { id: reviewGlyph; x: Style.space(14); y: Style.space(10); width: Style.space(18); text: root.glyphs.review; color: root.accent; font.family: root.bodyFont; font.pixelSize: Style.font.body }
                        Column {
                            id: reviewHead
                            x: reviewGlyph.x + reviewGlyph.width + Style.space(6); y: Style.space(10)
                            width: parent.width - x - Style.space(34)
                            spacing: Style.space(3)
                            Text { width: parent.width; text: "Agent review"; color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.body; font.weight: Font.DemiBold }
                            Text { objectName: "reviewByline"; width: parent.width; visible: text !== ""; text: root.reviewByline(); color: root.reviewOutdated ? root.warning : root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                            Text { width: parent.width; visible: !root.reviewOpen && root.reviewText !== ""; text: root.reviewSummary(root.reviewText); textFormat: Text.PlainText; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap; maximumLineCount: 3; elide: Text.ElideRight }
                        }
                        Text { anchors.right: parent.right; anchors.rightMargin: Style.space(14); y: Style.space(10); visible: root.reviewText !== ""; text: root.reviewOpen ? root.glyphs.open : root.glyphs.closed; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
                    }
                    Text {
                        objectName: "reviewBody"
                        visible: root.reviewOpen && root.reviewText !== ""
                        x: Style.space(14); width: parent.width - Style.space(28)
                        bottomPadding: Style.space(14)
                        text: visible ? root.reviewMarkdown(root.reviewText) : ""
                        textFormat: Text.MarkdownText
                        color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap
                    }
                }
            }
            Text {
                width: parent.width; visible: !root.report
                text: root.refreshing ? "Measuring your theme…" : "Press Refresh (or R) to check your theme."
                color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.body; wrapMode: Text.Wrap
            }

            // One card per subject. Its checks open inline, right under its grade.
            Repeater {
                id: subjects
                model: root.report ? root.report.categories : []
                delegate: Rectangle {
                    id: subject
                    required property var modelData
                    readonly property bool open: root.categoryOpen(modelData.id)
                    readonly property color tone: root.gradeColor(modelData.grade)
                    readonly property var checks: root.checksFor(modelData.id)
                    readonly property var passes: root.passedFor(modelData.id)
                    readonly property bool showPasses: root.passedOpen[modelData.id] === true
                    width: findingsColumn.width
                    height: subjectColumn.implicitHeight
                    radius: Style.space(8)
                    color: root.surface
                    // A faint edge keeps cards distinct on light popups, where the fill nearly vanishes.
                    border.width: 1
                    border.color: root.selectedCategory === modelData.id ? Util.alpha(root.accent, 0.7) : Util.alpha(root.ink, 0.07)
                    Column {
                        id: subjectColumn
                        width: parent.width
                        Ui.Button {
                            id: subjectButton
                            width: parent.width; height: Math.max(Style.space(58), subjectText.implicitHeight + Style.space(20))
                            text: ""; foreground: root.ink; accent: root.accent; focusable: true; leftAlign: true
                            radius: Style.space(8)
                            tooltipText: root.categoryHint(subject.modelData.id)
                            Accessible.name: root.categoryName(subject.modelData.id) + ", grade " + (subject.modelData.grade || "not checked") + ", " + root.categorySummary(subject.modelData) + (subject.open ? ", expanded" : ", collapsed")
                            onActiveFocusChanged: if (activeFocus) root.reveal(subject)
                            onClicked: root.toggleCategory(subject.modelData.id)
                            Rectangle {
                                id: gradeTile
                                x: Style.space(10); anchors.verticalCenter: parent.verticalCenter
                                width: Style.space(38); height: width; radius: Style.space(7)
                                color: Util.alpha(subject.tone, 0.14)
                                Text { anchors.centerIn: parent; text: subject.modelData.grade || "—"; color: subject.tone; font.family: root.bodyFont; font.pixelSize: Style.font.heading * 1.15; font.weight: Font.Bold }
                            }
                            Column {
                                id: subjectText
                                x: gradeTile.x + gradeTile.width + Style.space(12)
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - x - Style.space(34)
                                spacing: Style.space(4)
                                Item {
                                    width: parent.width; height: subjectName.implicitHeight
                                    Text { id: subjectName; width: parent.width - scoreMeter.width - Style.space(12); text: root.categoryName(subject.modelData.id); color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.body; font.weight: Font.DemiBold; elide: Text.ElideRight }
                                    // Score meter: how much of the subject's budget it kept.
                                    Item {
                                        id: scoreMeter
                                        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                                        width: Style.space(72); height: Style.space(4)
                                        visible: typeof subject.modelData.score === "number" && isFinite(subject.modelData.score)
                                        Rectangle { width: parent.width; height: parent.height; radius: height / 2; color: Util.alpha(root.ink, 0.1) }
                                        Rectangle { width: parent.width * Math.max(0, Math.min(1, (subject.modelData.score || 0) / 100)); height: parent.height; radius: height / 2; color: subject.tone }
                                    }
                                }
                                Text { width: parent.width; text: root.categorySummary(subject.modelData, true); textFormat: Text.StyledText; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                            }
                            Text { anchors.right: parent.right; anchors.rightMargin: Style.space(14); anchors.verticalCenter: parent.verticalCenter; text: subject.open ? root.glyphs.open : root.glyphs.closed; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
                        }
                        Column {
                            width: parent.width
                            visible: subject.open
                            Rectangle { x: Style.space(10); width: parent.width - Style.space(20); height: 1; color: Util.alpha(root.ink, 0.08) }
                            Item { width: 1; height: Style.space(4) }
                            Repeater {
                                model: subject.checks
                                delegate: CheckRow { required property var modelData; x: Style.space(4); width: subject.width - Style.space(8); finding: modelData }
                            }
                            // Passed checks fold into one row so the problems stay on top.
                            Ui.Button {
                                id: passToggle
                                x: Style.space(4); width: subject.width - Style.space(8)
                                height: passLabel.implicitHeight + Style.space(14)
                                visible: subject.passes.length > 0
                                text: ""; foreground: root.ink; accent: root.accent; focusable: true; leftAlign: true
                                Accessible.name: subject.passes.length + " passed checks" + (subject.showPasses ? ", expanded" : ", collapsed")
                                onActiveFocusChanged: if (activeFocus) root.reveal(this)
                                onClicked: root.togglePassed(subject.modelData.id)
                                Text { id: passGlyph; x: Style.space(10); anchors.verticalCenter: parent.verticalCenter; width: Style.space(18); text: root.glyphs.pass; color: root.accent; font.family: root.bodyFont; font.pixelSize: Style.font.body }
                                Text { id: passLabel; x: passGlyph.x + passGlyph.width + Style.space(6); anchors.verticalCenter: parent.verticalCenter; text: (subject.passes.length === 1 ? "1 check passed" : subject.passes.length + " checks passed") + (subject.showPasses ? "" : " · show"); color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
                                Text { anchors.right: parent.right; anchors.rightMargin: Style.space(10); anchors.verticalCenter: parent.verticalCenter; text: subject.showPasses ? root.glyphs.open : root.glyphs.closed; color: root.faint; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
                            }
                            Repeater {
                                model: subject.showPasses ? subject.passes : []
                                delegate: CheckRow { required property var modelData; x: Style.space(4); width: subject.width - Style.space(8); finding: modelData }
                            }
                            Item { width: 1; height: Style.space(6) }
                        }
                    }
                }
            }

            // ---- palette: reference material, after the grades
            Column {
                id: paletteOverview
                objectName: "paletteOverview"
                width: parent.width
                spacing: Style.space(7)
                topPadding: Style.space(10)
                visible: root.paletteBanks.length > 0 || root.paletteRoles.length > 0
                Item {
                    width: parent.width; height: paletteTitle.implicitHeight
                    Text { id: paletteTitle; text: "PALETTE"; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.caption; font.weight: Font.DemiBold; font.letterSpacing: 1.6 }
                    Text { anchors.right: parent.right; width: parent.width * 0.6; text: root.copiedPaletteColor ? "Copied " + root.copiedPaletteColor : root.focusedPaletteColor ? root.focusedPaletteColor + " · Enter to copy" : "Click a color to copy"; color: root.copiedPaletteColor ? root.accent : root.faint; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; horizontalAlignment: Text.AlignRight; elide: Text.ElideRight }
                }
                Repeater {
                    model: root.paletteBanks
                    delegate: Row {
                        id: paletteBank
                        required property var modelData
                        width: paletteOverview.width
                        spacing: Style.space(4)
                        Repeater {
                            model: paletteBank.modelData
                            delegate: Ui.Button {
                                required property var modelData
                                width: (paletteBank.width - paletteBank.spacing * (paletteBank.modelData.length - 1)) / paletteBank.modelData.length
                                height: Style.space(25)
                                radius: Style.space(4)
                                color: root.safeColor(modelData.color)
                                foreground: root.ink; accent: root.accent
                                focusable: true
                                tooltipText: modelData.key + " · " + modelData.color + " · Copy color"
                                Accessible.name: modelData.key + " " + modelData.color
                                onActiveFocusChanged: {
                                    root.focusedPaletteColor = activeFocus ? modelData.color : ""
                                    if (activeFocus) root.reveal(paletteOverview)
                                }
                                onClicked: root.copyPaletteColor(modelData.color)
                            }
                        }
                    }
                }
                Flow {
                    width: parent.width; spacing: Style.space(5)
                    Repeater {
                        model: root.paletteRoles
                        delegate: Ui.Button {
                            id: paletteRole
                            required property var modelData
                            width: Math.floor((paletteOverview.width - Style.space(15)) / 4)
                            height: Style.space(28)
                            foreground: root.ink; accent: root.accent; focusable: true
                            tooltipText: modelData.key + " · " + root.paletteColors[modelData.key] + " · Copy color"
                            Accessible.name: modelData.key + " " + root.paletteColors[modelData.key]
                            onActiveFocusChanged: {
                                root.focusedPaletteColor = activeFocus ? root.paletteColors[modelData.key] : ""
                                if (activeFocus) root.reveal(paletteOverview)
                            }
                            onClicked: root.copyPaletteColor(root.paletteColors[modelData.key])
                            Row {
                                anchors.centerIn: parent; spacing: Style.space(6)
                                Rectangle { width: Style.space(13); height: width; radius: width / 2; anchors.verticalCenter: parent.verticalCenter; color: root.safeColor(root.paletteColors[paletteRole.modelData.key]); border.width: 1; border.color: Util.alpha(root.ink,0.24) }
                                Text { text: paletteRole.modelData.label; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
                            }
                        }
                    }
                }
            }

            // ---- how the grade works: a labeled row, not a stray link
            Rectangle {
                width: parent.width
                visible: !!root.report
                height: methodColumn.implicitHeight
                radius: Style.space(8)
                color: "transparent"
                border.width: 1; border.color: Util.alpha(root.ink, 0.1)
                Column {
                    id: methodColumn
                    width: parent.width
                    Ui.Button {
                        width: parent.width; height: methodLabel.implicitHeight + Style.space(18)
                        text: ""; foreground: root.ink; accent: root.accent; focusable: true; leftAlign: true
                        radius: Style.space(8)
                        Accessible.name: "How this grade works" + (root.showMethod ? ", expanded" : ", collapsed")
                        onActiveFocusChanged: if (activeFocus) root.reveal(this)
                        onClicked: root.showMethod = !root.showMethod
                        Text { id: methodGlyph; x: Style.space(14); anchors.verticalCenter: parent.verticalCenter; width: Style.space(18); text: root.glyphs.note; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.body }
                        Text { id: methodLabel; x: methodGlyph.x + methodGlyph.width + Style.space(6); anchors.verticalCenter: parent.verticalCenter; text: "How this grade works"; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; font.weight: Font.DemiBold }
                        Text { anchors.right: parent.right; anchors.rightMargin: Style.space(14); anchors.verticalCenter: parent.verticalCenter; text: root.report ? "v" + root.report.analyzerVersion + "  " + (root.showMethod ? root.glyphs.open : root.glyphs.closed) : ""; color: root.faint; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
                    }
                    Text {
                        visible: root.showMethod
                        x: Style.space(14); width: parent.width - Style.space(28)
                        bottomPadding: Style.space(12)
                        text: root.report ? (root.report.capReasons || []).concat(root.report.assumptions || []).join("\n\n") + "\n\nAnalyzer " + root.report.analyzerVersion + " · " + root.report.rulesetVersion + "\n" + root.report.profile + "\nOmarchy " + (root.report.omarchyVersion || "not detected") : ""
                        textFormat: Text.PlainText; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap
                    }
                }
            }
        }
    }
    // More below: fade the bottom edge so a cut-off card reads as scrollable.
    Rectangle {
        anchors.bottom: scroll.bottom
        width: scroll.width; height: Style.space(36)
        visible: scroll.overflowing && scroll.contentY < scroll.contentHeight - scroll.height - 2
        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.rgba(root.background.r, root.background.g, root.background.b, 0) }
            GradientStop { position: 1.0; color: Qt.rgba(root.background.r, root.background.g, root.background.b, 1) }
        }
    }
}
