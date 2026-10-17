import QtQuick
import QtQuick.Controls
import Quickshell
import qs.Ui as Ui
import qs.Commons
import qs.Commons as Theme

FocusScope {
    id: root
    property var report: null
    property string status: "Not audited"
    property string errorText: ""
    property bool refreshing: false
    property string selectedCategory: ""
    property string selectedGroup: "issues"
    property bool showMethod: false
    property var expandedRules: ({})
    property var technicalRules: ({})
    property string viewedTarget: ""
    property string copiedPaletteColor: ""
    property string focusedPaletteColor: ""
    readonly property var paletteEvidence: report && Array.isArray(report.findings) ? report.findings.find(function(f) { return f.ruleId === "validity.roles" }) : null
    readonly property var paletteColors: paletteEvidence && paletteEvidence.evidence ? paletteEvidence.evidence.paletteColors || paletteEvidence.evidence.resolvedColors || ({}) : ({})
    readonly property var paletteBanks: paletteRows()
    readonly property var paletteRoles: [{key:"background",label:"Base"},{key:"foreground",label:"Text"},{key:"accent",label:"Accent"},{key:"urgent",label:"Urgent"}].filter(function(role) { return root.validPaletteColor(root.paletteColors[role.key]) })
    // ShellColor is the current name; Color supports the earlier Quattro runtime.
    readonly property var themeColors: typeof Theme.ShellColor !== "undefined" ? Theme.ShellColor : Theme.Color
    readonly property color ink: themeColors.popups.text
    readonly property color muted: Util.alpha(ink, 0.7)
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
    onRefreshingChanged: {
        slowRefresh = false
        if (refreshing) slowRefreshTimer.restart()
        else slowRefreshTimer.stop()
    }
    Timer { id: slowRefreshTimer; interval: 350; onTriggered: root.slowRefresh = true }
    signal refreshRequested()
    signal closeRequested()

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
        readonly property color cue: finding.evidence.referenceDefectOwnedByReadability ? root.warning : finding.severity === "failure" ? root.failure : finding.severity === "caution" ? root.warning : root.accent
        spacing: Style.space(10)
        Row {
            width: parent.width; spacing: Style.space(10)
            visible: !!visual.evidence.foreground && !!visual.evidence.effectiveBackground
            Rectangle {
                width: Style.space(96); height: Style.space(54); radius: Style.space(5)
                color: root.safeColor(visual.evidence.effectiveBackground)
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
                    Rectangle { width: parent.width; height: Style.space(32); radius: Style.space(4); color: root.safeColor(modelData.color) }
                    Text { width: parent.width; text: modelData.name + " · " + modelData.color; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                }
            }
        }
        Column {
            width: parent.width; spacing: Style.space(6); visible: visual.contrast || visual.distance
            Text { text: (visual.contrast ? "Contrast" : "OKLab separation") + " · target " + visual.evidence.required + (visual.contrast ? ":1" : ""); color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
            Item {
                width: parent.width; height: Style.space(16)
                Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width; height: Style.space(6); radius: height / 2; color: root.background }
                Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width * Math.max(0, Math.min(1, (visual.measured - visual.minimum) / (visual.maximum - visual.minimum))); height: Style.space(6); radius: height / 2; color: visual.cue }
                Rectangle { x: Math.max(0, Math.min(parent.width - width, parent.width * (visual.target - visual.minimum) / (visual.maximum - visual.minimum))); width: Style.space(2); height: parent.height; color: root.ink }
            }
            Text { width: parent.width; text: visual.contrast ? "1:1 — 21:1 · marker is the required contrast" : "0 — " + Number(visual.maximum).toFixed(3) + " · marker is the required separation"; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
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

    // KeyboardPanel owns the themed popup background and its authored alpha.
    function counts() {
        if (!report) return ""
        var problems = report.findings.filter(function(f) { return !f.policySuppressed && f.severity === "failure" }).length
        var cautions = report.findings.filter(function(f) { return !f.policySuppressed && f.severity === "caution" }).length
        return problems + (problems === 1 ? " problem" : " problems") + " · " + cautions + (cautions === 1 ? " caution" : " cautions") + " · " + number("unassessed") + " limitations"
    }
    function categoryStatus(c) {
        var modeled = report && report.findings.some(function(f) { return f.category === c.id && f.method === "modeled-appearance" && f.assessedBudget > 0 })
        return c.state === "not-checked" ? "Not checked" : c.state === "not-applicable" ? "N/A" : c.state === "limited" ? "Limited" : c.lost > 0 ? "Needs attention" : modeled ? "Modeled" : "Passed"
    }
    function number(group, category) {
        if (!report) return 0
        return report.findings.filter(function(f) { return belongs(f, group) && (!category || f.category === category) }).length
    }
    function belongs(f, group) {
        var shared = f.evidence && f.evidence.referenceDefectOwnedByReadability
        return group === "issues" ? !f.policySuppressed && (f.severity === "failure" || f.severity === "caution") : group === "passed" ? !f.policySuppressed && f.severity === "pass" && !shared : group === "unassessed" ? f.severity === "unassessed" || f.policySuppressed : f.severity === "advisory" || shared
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
        return f.severity === "failure" ? "Needs fixing" : f.severity === "caution" ? "Worth checking" : f.severity === "unassessed" ? "Limitation" : f.severity === "pass" && !(f.evidence && f.evidence.referenceDefectOwnedByReadability) ? "Passed" : "Note"
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
        if (f.ruleId === "validity.palette") return "Palette parsed successfully."
        if (f.ruleId === "validity.roles") return "Background, text, accent and urgent colors resolved."
        if (f.ruleId === "validity.shell") return "Known shell values are valid."
        if (f.ruleId === "validity.backgrounds" && e.decoded !== undefined) return e.decoded + (e.decoded === 1 ? " unique wallpaper sampled successfully." : " unique wallpapers sampled successfully.")
        return f.summary
    }
    function filteredFindings() {
        if (!report) return []
        var list = report.findings.filter(function(f) { return belongs(f, selectedGroup) && (!selectedCategory || f.category === selectedCategory) }).sort(function(a,b) { return (a.severity === "failure" ? 0 : 1) - (b.severity === "failure" ? 0 : 1) || root.report.findings.indexOf(a) - root.report.findings.indexOf(b) })
        // Checks paused for the same runtime reason read as one card, not a wall of copies.
        var paused = list.filter(function(f) { return f.evidence && f.evidence.runtimeContract })
        if (paused.length < 2) return list
        return [{
            ruleId: "runtime.paused", category: "", severity: "unassessed", method: "resolved-token", policySuppressed: false,
            summary: paused[0].summary,
            evidence: { reason: paused[0].evidence.reason, pausedChecks: paused.map(function(f) { return f.ruleId }) },
            sources: [],
            remediation: paused[0].remediation + " Paused: " + paused.map(function(f) { return root.title(f.ruleId) }).join(", ") + "."
        }].concat(list.filter(function(f) { return paused.indexOf(f) < 0 }))
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
            selectedGroup = "issues"
            showMethod = false
            expandedRules = ({})
            technicalRules = ({})
            scroll.contentY = 0
            copiedPaletteColor = ""
            focusedPaletteColor = ""
            paletteCopyExpiry.stop()
        }
    }
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) { root.closeRequested(); event.accepted = true }
        else if (event.key === Qt.Key_PageDown || event.key === Qt.Key_Down) { scroll.contentY = Math.min(Math.max(0, scroll.contentHeight - scroll.height), scroll.contentY + Style.space(60)); event.accepted = true }
        else if (event.key === Qt.Key_PageUp || event.key === Qt.Key_Up) { scroll.contentY = Math.max(0, scroll.contentY - Style.space(60)); event.accepted = true }
        else if (event.key === Qt.Key_R && !(event.modifiers & Qt.ControlModifier)) { root.refreshRequested(); event.accepted = true }
    }

    Column {
        id: header
        width: parent.width
        spacing: Style.space(12)
        Row {
            width: parent.width
            spacing: Style.space(12)
            Text { width: parent.width - refreshButton.width - parent.spacing; anchors.verticalCenter: parent.verticalCenter; text: "THEME REPORT"; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; font.letterSpacing: 1.4 }
            Ui.Button {
                id: refreshButton
                objectName: "refreshButton"
                iconText: "\uf021"
                foreground: root.muted; accent: root.accent
                focusable: true; enabled: !root.refreshing
                tooltipText: root.refreshing ? "Checking theme…" : "Refresh"
                Accessible.name: "Refresh theme report"
                onClicked: root.refreshRequested()
            }
        }
        Text { width: parent.width; text: root.report ? root.displayName(root.report.theme) : "Your theme, examined"; textFormat: Text.PlainText; color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.heading * 1.25; font.weight: Font.DemiBold; wrapMode: Text.Wrap }
        Row {
            width: parent.width
            spacing: Style.space(16)
            Rectangle {
                width: Style.space(64); height: width; radius: Style.space(12); color: root.surface
                Text { anchors.centerIn: parent; text: !root.previous && root.report && root.report.grade ? root.report.grade : "—"; color: root.accent; font.family: root.bodyFont; font.pixelSize: Style.font.heading * 1.8; font.weight: Font.DemiBold }
            }
            Column {
                width: parent.width - Style.space(80)
                spacing: Style.space(5)
                Text { width: parent.width; text: root.verdict(); color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.body; font.weight: Font.DemiBold; wrapMode: Text.Wrap }
                Text { width: parent.width; text: root.explanation(); color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
            }
        }
        Text { width: parent.width; visible: !!root.report; text: root.counts(); color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
        // Errors and paused checks belong next to the verdict, not below the fold.
        Text { objectName: "errorText"; width: parent.width; visible: root.errorText !== ""; text: root.errorText; textFormat: Text.PlainText; color: root.warning; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
        Text {
            objectName: "runtimeBanner"
            width: parent.width; visible: root.runtimePaused && !root.previous
            text: root.contract ? (root.contract.palette ? "Some checks are paused: " : "Checks are paused: ") + "this Omarchy build changed " + (root.contract.unverified || []).join(", ") + ". Update theme-report to measure them; nothing is guessed meanwhile." : ""
            textFormat: Text.PlainText; color: root.warning; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap
        }
    }

    Flickable {
        id: scroll
        anchors.top: header.bottom; anchors.topMargin: Style.space(18)
        anchors.bottom: parent.bottom
        width: parent.width; clip: true
        contentWidth: width; contentHeight: findingsColumn.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        Column {
            id: findingsColumn
            width: scroll.width
            spacing: Style.space(14)
            Column {
                id: paletteOverview
                objectName: "paletteOverview"
                width: parent.width
                spacing: Style.space(7)
                visible: root.paletteBanks.length > 0 || root.paletteRoles.length > 0
                Row {
                    width: parent.width
                    Text { width: parent.width * 0.42; text: "COLOR PALETTE"; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; font.letterSpacing: 1.1 }
                    Text { width: parent.width * 0.58; text: root.copiedPaletteColor ? "Copied " + root.copiedPaletteColor : root.focusedPaletteColor ? root.focusedPaletteColor + " · Enter to copy" : "Click a color to copy"; color: root.copiedPaletteColor ? root.accent : root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; horizontalAlignment: Text.AlignRight; elide: Text.ElideRight }
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
            Column {
                width: parent.width; spacing: Style.space(3); visible: !!root.report
                Repeater {
                    model: root.report ? root.report.categories : []
                    delegate: Ui.Button {
                        id: categoryRow
                        required property var modelData
                        width: findingsColumn.width; height: Math.max(Style.space(48), categoryTitle.implicitHeight + Style.space(18))
                        text: ""; foreground: root.ink; accent: root.accent; focusable: true
                        selected: root.selectedCategory === modelData.id
                        Accessible.name: root.categoryName(modelData.id) + ", grade " + (modelData.grade || "not checked") + ", " + root.categoryStatus(modelData)
                        onActiveFocusChanged: if (activeFocus) root.reveal(this)
                        onClicked: {
                            root.selectedCategory = root.selectedCategory === modelData.id ? "" : modelData.id
                            if (root.selectedCategory) {
                                var fs = root.report.findings.filter(function(f) { return f.category === root.selectedCategory })
                                root.selectedGroup = fs.some(function(f) { return root.belongs(f, "issues") }) ? "issues" : fs.some(function(f) { return root.belongs(f, "unassessed") }) ? "unassessed" : fs.some(function(f) { return root.belongs(f, "passed") }) ? "passed" : "notes"
                            }
                        }
                        Row {
                            anchors.fill: parent; anchors.margins: Style.space(9); spacing: Style.space(10)
                            Rectangle {
                                width: Style.space(32); height: width; radius: Style.space(5); color: root.surface; anchors.verticalCenter: parent.verticalCenter
                                Text { anchors.centerIn: parent; text: categoryRow.modelData.grade || "—"; color: categoryRow.modelData.lost > 0 ? root.warning : categoryRow.modelData.grade ? root.accent : root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.heading; font.weight: Font.DemiBold }
                            }
                            Text { id: categoryTitle; width: parent.width - Style.space(32) - categoryTag.width - parent.spacing * 2; anchors.verticalCenter: parent.verticalCenter; text: root.categoryName(categoryRow.modelData.id); color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.body; font.weight: Font.DemiBold; wrapMode: Text.Wrap }
                            Text { id: categoryTag; width: Style.space(86); anchors.verticalCenter: parent.verticalCenter; text: root.categoryStatus(categoryRow.modelData); color: categoryRow.modelData.lost > 0 || categoryRow.modelData.state === "limited" ? root.warning : root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap; horizontalAlignment: Text.AlignRight }
                        }
                    }
                }
            }
            Rectangle { width: parent.width; height: 1; color: root.surface }
            Flow {
                width: parent.width; spacing: Style.space(5); visible: !!root.report
                Repeater {
                    model: [{id:"issues",label:"Issues"},{id:"passed",label:"Passed"},{id:"unassessed",label:"Limitations"},{id:"notes",label:"Notes"}]
                    delegate: Ui.Button {
                        required property var modelData
                        text: modelData.label + "  " + root.number(modelData.id, root.selectedCategory); foreground: root.selectedGroup === modelData.id ? root.ink : root.muted; accent: root.accent; selected: root.selectedGroup === modelData.id; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall
                        onActiveFocusChanged: if (activeFocus) root.reveal(this)
                        onClicked: { root.selectedGroup = modelData.id; root.showMethod = false }
                    }
                }
                Ui.Button { visible: root.selectedCategory !== ""; text: "Clear category ×"; foreground: root.muted; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall; onClicked: root.selectedCategory = "" }
            }
            Text {
                width: parent.width; visible: root.filteredFindings().length === 0
                text: !root.report ? (root.refreshing ? "Measuring your theme…" : "Run an audit to begin.") : root.selectedCategory ? "No checks in this group for " + root.categoryName(root.selectedCategory) + "." : root.selectedGroup === "issues" ? "No problems found. Explore Passed to see what checked out." : "No checks in this group."
                color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.body; wrapMode: Text.Wrap
            }
            Repeater {
                model: root.filteredFindings()
                delegate: Rectangle {
                    id: findingCard
                    required property var modelData
                    readonly property bool expanded: root.expandedRules[modelData.ruleId] === true
                    readonly property bool technical: root.technicalRules[modelData.ruleId] === true
                    width: findingsColumn.width
                    implicitHeight: cardColumn.implicitHeight + Style.space(24)
                    radius: Style.space(8); color: root.surface
                    Column {
                        id: cardColumn
                        x: Style.space(12); y: Style.space(10); width: parent.width - Style.space(24); spacing: Style.space(7)
                        Ui.Button {
                            width: parent.width; text: ""; height: findingHeading.implicitHeight + Style.space(8); foreground: root.ink; accent: root.accent; focusable: true; leftAlign: true
                            Accessible.name: root.title(findingCard.modelData.ruleId) + ", " + root.severityLabel(findingCard.modelData) + (findingCard.expanded ? ", expanded" : ", collapsed")
                            onActiveFocusChanged: if (activeFocus) root.reveal(findingCard)
                            onClicked: root.toggleDetail(findingCard.modelData.ruleId, false)
                            Row {
                                id: findingHeading; anchors.verticalCenter: parent.verticalCenter; width: parent.width; spacing: Style.space(10)
                                Text { width: parent.width - toggle.implicitWidth - parent.spacing; text: root.title(findingCard.modelData.ruleId); color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.body; font.weight: Font.DemiBold; wrapMode: Text.Wrap }
                                Text { id: toggle; text: findingCard.expanded ? "−" : "+"; color: root.accent; font.pixelSize: Style.font.body }
                            }
                        }
                        Text { width: parent.width; text: root.severityLabel(findingCard.modelData); color: findingCard.modelData.severity === "failure" ? root.failure : findingCard.modelData.severity === "caution" ? root.warning : findingCard.modelData.severity === "pass" ? root.accent : root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall }
                        Text { width: parent.width; text: root.shortReason(findingCard.modelData); textFormat: Text.PlainText; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                        Column {
                            visible: findingCard.expanded; width: parent.width; spacing: Style.space(10)
                            Rectangle { width: parent.width; height: 1; color: root.background }
                            EvidenceVisual { width: parent.width; finding: findingCard.modelData }
                            Text { width: parent.width; text: root.context(findingCard.modelData); color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                            Text { width: parent.width; text: findingCard.modelData.evidence.referenceDefectOwnedByReadability ? "SCORING NOTE" : findingCard.modelData.severity === "pass" ? "WHY IT PASSED" : "WHAT TO DO"; color: root.accent; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; font.weight: Font.DemiBold }
                            Text { width: parent.width; text: findingCard.modelData.severity === "pass" ? root.detail(findingCard.modelData) : findingCard.modelData.remediation; textFormat: Text.PlainText; color: root.ink; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                            Text { width: parent.width; text: findingCard.modelData.sources.map(function(s) { return s.file + (s.section ? "  [" + s.section + "]" : "") + (s.key ? "  " + s.key : "") }).join("\n"); textFormat: Text.PlainText; color: root.muted; font.family: Style.font.family; font.pixelSize: Style.font.bodySmall; wrapMode: Text.WrapAnywhere }
                            Ui.Button { text: findingCard.technical ? "Hide technical data" : "Technical data"; foreground: root.muted; accent: root.accent; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall; onActiveFocusChanged: if (activeFocus) root.reveal(findingCard); onClicked: root.toggleDetail(findingCard.modelData.ruleId, true) }
                            Text { visible: findingCard.technical; width: parent.width; text: "MEASUREMENT · " + findingCard.modelData.ruleId; color: root.accent; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
                            Text { visible: findingCard.technical; width: parent.width; text: JSON.stringify(findingCard.modelData.evidence, null, 2); textFormat: Text.PlainText; color: root.muted; font.family: Style.font.family; font.pixelSize: Style.font.bodySmall; wrapMode: Text.WrapAnywhere }
                        }
                    }
                }
            }
            Ui.Button { text: root.showMethod ? "Hide method & limits" : "Method & limits"; foreground: root.muted; accent: root.accent; focusable: true; fontFamily: root.bodyFont; fontSize: Style.font.bodySmall; onActiveFocusChanged: if (activeFocus) root.reveal(this); onClicked: root.showMethod = !root.showMethod }
            Text { visible: root.showMethod; width: parent.width; text: root.report ? (root.report.capReasons || []).concat(root.report.assumptions || []).join("\n\n") + "\n\nAnalyzer " + root.report.analyzerVersion + " · " + root.report.rulesetVersion + "\n" + root.report.profile + "\nOmarchy " + (root.report.omarchyVersion || "not detected") : "No report is available yet."; textFormat: Text.PlainText; color: root.muted; font.family: root.bodyFont; font.pixelSize: Style.font.bodySmall; wrapMode: Text.Wrap }
        }
    }
}
