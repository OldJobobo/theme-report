import QtQuick
import Quickshell
import qs.Commons
import qs.Commons as Theme
import "../.." as Plugin

ShellRoot {
    Plugin.ReportPanel { id: panel; width: 720; height: 760 }
    function verify(value, name) { if (!value) throw new Error("UI assertion failed: " + name) }
    property int refreshCount: 0
    Connections { target: panel; function onRefreshRequested() { refreshCount++ } }
    function namedItem(item, name) {
        if (item.objectName === name) return item
        for (var i = 0; i < item.children.length; i++) {
            var found = namedItem(item.children[i], name)
            if (found) return found
        }
        return null
    }
    function finding(severity, extra) { return {severity:severity, policySuppressed:false, category:"readability", ruleId:"readability.bar", summary:"Summary", sources:[], remediation:"No change needed.", method:"resolved-token", assessedBudget:1, evidence:extra || ({})} }
    Component.onCompleted: {
        try {
            var refresh = namedItem(panel, "refreshButton")
            verify(refresh && refresh.enabled && refresh.iconText !== "" && refresh.tooltipText === "Refresh", "compact header Refresh")
            refresh.clicked(); verify(refreshCount === 1, "Refresh signal")
            panel.refreshing = true
            verify(!refresh.enabled && !panel.previous, "busy Refresh is disabled without flashing previous")
            panel.slowRefresh = true
            verify(panel.previous, "slow refresh labels previous evidence")
            panel.refreshing = false
            verify(!panel.slowRefresh && !panel.previous, "finished refresh clears previous")
            panel.errorText = "Failure"; verify(panel.previous, "error retains previous evidence")
            panel.errorText = ""; verify(!panel.previous, "no stale state")
            var colors=panel.themeColors
            colors.accent="#d178a9"; colors.urgent="#f02a4f"
            colors.shellValues={"popups.background":"#2a0d1a","popups.background-alpha":"0.94","popups.text":"#f0dde3"}
            verify(Qt.colorEqual(panel.ink,"#f0dde3") && Qt.colorEqual(panel.accent,"#d178a9") && Qt.colorEqual(panel.failure,"#f02a4f"),"theme palette bindings")
            verify(Math.abs(panel.background.a-0.94)<0.01 && panel.bodyFont===Style.font.family,"popup alpha and theme font")
            colors.accent="#a9e1da"
            colors.shellValues={"popups.background":"#eff1f5","popups.background-alpha":"1","popups.text":"#24273a"}
            verify(Qt.colorEqual(panel.ink,"#24273a") && Qt.colorEqual(panel.background,"#eff1f5") && Qt.colorEqual(panel.accent,"#a9e1da"),"live light-theme rebinding")
            colors.shellValues={"hyprland.active-border":"rgba(D83A66ee) rgba(F05C7Acc) rgba(A9E1DAaa) rgba(F05C7Acc) 35deg","popups.border":"hyprland.active-border","popups.border-alpha":"0.82","popups.border-width":"2"}
            // Older Color-named runtimes collide with Qt 6.12's Color type
            // inside their own Border singleton; validate the active native renderer.
            if (typeof Theme.ShellColor !== "undefined") {
                var border=Border.surfaceSpec("popups","border",colors.accent,2)
                verify(border.gradient.enabled && border.gradient.colors.length===4 && border.gradient.angle===35,"native RGBA gradient alias")
                verify(Math.abs(border.gradient.colors[0].a-195/255)<0.00001 && Math.abs(border.gradient.colors[1].a-167/255)<0.00001 && Math.abs(border.gradient.colors[2].a-139/255)<0.00001,"native gradient alpha-byte compositing contract")
            }
            var palette={background:"#16060c",foreground:"#d7bec7",accent:"#d83a66",urgent:"#e33b59"}
            for (var i=0;i<16;i++) palette["color"+i]="#"+("000000"+(i*65793).toString(16)).slice(-6)
            var roles={ruleId:"validity.roles",severity:"pass",category:"validity",policySuppressed:false,evidence:{paletteColors:palette},sources:[]}
            panel.report={themePath:"/palette",theme:"palette",grade:"A",state:"ready",capReasons:[],categories:[],findings:[roles]}
            verify(panel.paletteBanks.length===2 && panel.paletteBanks[0].length===8 && panel.paletteBanks[1][0].key==="color8" && panel.paletteBanks[1][7].key==="color15","ordered normal/bright palette banks")
            verify(panel.paletteRoles.length===4 && panel.paletteRoles[0].key==="background","resolved palette roles")
            panel.copyPaletteColor("bad"); verify(panel.copiedPaletteColor==="","invalid clipboard color rejected")
            panel.copyPaletteColor("#d83a66"); verify(panel.copiedPaletteColor==="#d83a66","palette copy feedback")
            panel.report={themePath:"/semantic",theme:"semantic",grade:"A",state:"ready",capReasons:[],categories:[],findings:[Object.assign({},roles,{evidence:{paletteColors:{background:"#101315",red:"#ff7777",green:"#88ddaa",blue:"not-a-color"}}})]}
            verify(panel.paletteBanks.length===1 && panel.paletteBanks[0].length===2 && panel.paletteRoles.length===1 && panel.copiedPaletteColor==="","semantic-only palette, unsupported colors, copy reset")
            verify(!Qt.colorEqual(panel.warning, panel.accent), "caution hue differs from accent")
            panel.report=null; verify(panel.paletteBanks.length===0 && panel.paletteRoles.length===0,"missing palette has no invented swatches")
            var pass=finding("pass",{contrast:8.12,required:4.5})
            verify(panel.belongs(pass,"passed") && !panel.belongs(pass,"issues"),"passing exploration")
            verify(panel.proof(pass).indexOf("8.12:1")>=0 && panel.proof(pass).indexOf("needs")<0,"positive proof")
            var shared=finding("pass",{referenceDefectOwnedByReadability:true,contrast:2,required:4.5})
            verify(!panel.belongs(shared,"passed") && panel.belongs(shared,"notes"),"single-owner honesty")
            var suppressed=finding("pass");suppressed.policySuppressed=true
            verify(!panel.belongs(suppressed,"passed") && panel.belongs(suppressed,"unassessed"),"suppression honesty")
            verify(panel.categoryStatus({weight:20,id:"shell",state:"ready",lost:0})==="Passed","restored shell category is graded")
            var a=finding("pass");var b=finding("pass");b.category="shell"
            panel.report={themePath:"/fixture",theme:"test",grade:"A",state:"ready",capReasons:[],categories:[],findings:[a,b,finding("failure")]}
            verify(panel.number("passed")===2 && panel.number("passed","readability")===1 && panel.passedFor("readability").length===1 && panel.checksFor("readability").length===1 && panel.checksFor("readability")[0].severity==="failure","per-subject checks and counts")
            verify(panel.categoryOpen("readability") && !panel.categoryOpen("shell"),"subjects with issues open on their own")
            panel.toggleCategory("readability"); verify(!panel.categoryOpen("readability"),"subject collapses on request")
            panel.togglePassed("shell"); verify(panel.passedOpen["shell"]===true,"passed checks unfold")
            panel.toggleDetail("readability.bar",false);panel.toggleDetail("readability.bar",true)
            var replacement=Object.assign({},panel.report);replacement.findings=[a,b]
            panel.report=replacement
            verify(panel.expandedRules["readability.bar"] && panel.technicalRules["readability.bar"],"detail retention during refresh")
            panel.report=Object.assign({},replacement,{theme:"another"})
            verify(!panel.expandedRules["readability.bar"] && !panel.technicalRules["readability.bar"],"detail reset on new theme")
            var paused=function(id){var f=finding("unassessed",{runtimeContract:"partial",reason:"Changed Style.qml"});f.ruleId=id;f.remediation="Update.";return f}
            panel.report={themePath:"/runtime",theme:"scooby-doo",grade:"A",state:"provisional",capReasons:[],categories:[],runtimeContract:{status:"partial",palette:true,unverified:["Style.qml control states"]},findings:[paused("readability.bar"),paused("shell.focus-fill"),finding("unassessed",{reason:"other"})]}
            var cards=panel.checksFor("readability")
            verify(cards.length===2 && cards[0].ruleId==="runtime.paused" && cards[0].remediation.indexOf("Status bar")>=0,"paused checks collapse into one row")
            verify(panel.categorySummary({id:"readability",state:"limited"})==="3 not measured","not-measured subject summary counts every check")
            verify(panel.runtimePaused && namedItem(panel,"runtimeBanner").visible && namedItem(panel,"runtimeBanner").text.indexOf("Style.qml")>=0,"runtime banner")
            verify(panel.displayName("scooby-doo")==="Scooby Doo","title-cased theme name")
            var issue=finding("failure");issue.category="shell";issue.ruleId="shell.popup-border"
            var limited=finding("unassessed");limited.category="semantic";limited.ruleId="semantic.accent"
            var categoryReport={themePath:"/categories",theme:"categories",grade:"B",state:"ready",capReasons:[],categories:[],findings:[issue,limited,finding("pass")]}
            panel.report=categoryReport
            verify(panel.categoryOpen("shell") && !panel.categoryOpen("semantic"),"only subjects with issues start open")
            verify(panel.categorySummary({id:"shell",state:"ready"})==="1 problem" && panel.categorySummary({id:"readability",state:"ready"})==="Its check passed","subject summaries")
            panel.selectCategory("semantic"); verify(panel.selectedCategory==="semantic" && panel.categoryOpen("semantic"),"selecting a subject opens it")
            panel.selectCategory(""); verify(panel.selectedCategory==="","clearing the category")
            panel.report=null; panel.selectCategory("shell")
            panel.report=Object.assign({},categoryReport,{theme:"arrived"})
            verify(panel.selectedCategory==="shell" && panel.categoryOpen("shell") && panel.pendingCategory==="","category requested before the report survives its arrival")
            var roles=finding("failure",{missingOrUnsupported:["background","accent"]});roles.ruleId="validity.roles";roles.summary="Foundational roles resolve"
            verify(panel.shortReason(roles)==="Missing or unsupported: background, accent.","failing check never shows pass wording")
            roles.evidence={};verify(panel.proof(roles)==="Foundational roles resolve","failing validity check falls back to its summary")
            var agentCalls=[], previewClosings=0
            panel.agentRequested.connect(function(k,r,m){ agentCalls.push(k+":"+r+":"+m) })
            panel.previewClosed.connect(function(){ previewClosings++ })
            panel.report=categoryReport; panel.agentName="Pi"
            var fixButton=namedItem(panel,"agentFixButton")
            verify(namedItem(panel,"agentActions").visible && fixButton.text==="Fix 1 issue with Pi","header handoff names the agent and the issue count")
            fixButton.clicked(); namedItem(panel,"agentCopyButton").clicked(); namedItem(panel,"agentPreviewButton").clicked()
            verify(agentCalls.join(",")==="fix::window,fix::copy,fix::preview","header handoff signals: "+agentCalls.join(","))
            panel.agentName=""; verify(fixButton.text==="Choose an agent to fix 1 issue","no default agent")
            panel.agentName="Pi"
            panel.actionsEnabled=false; verify(!fixButton.enabled,"handoff disabled while stale or checking"); panel.actionsEnabled=true
            panel.agentBusy=true; verify(!fixButton.enabled,"handoff disabled while one runs"); panel.agentBusy=false
            var pausedOne=finding("unassessed",{runtimeContract:"partial"}); var note=finding("advisory"); note.ruleId="advisory.lua-syntax"
            verify(panel.handoffKind(issue)==="fix" && panel.handoffKind(limited)==="measure" && panel.handoffKind(finding("pass"))==="" && panel.handoffKind(pausedOne)==="" && panel.handoffKind(note)==="","per-check handoff kinds")
            panel.report=Object.assign({},categoryReport,{findings:[limited,finding("pass")]})
            verify(panel.primaryHandoff().kind==="measure" && fixButton.text==="Ask Pi about 1 unmeasured","only unmeasured checks: ask about them")
            var reviewButton=namedItem(panel,"agentReviewButton")
            panel.report=categoryReport; verify(reviewButton.visible && reviewButton.text==="Review with Pi","review offered beside a fix")
            reviewButton.clicked(); verify(agentCalls[agentCalls.length-1]==="review::window","review signal")
            panel.report=Object.assign({},categoryReport,{findings:[finding("pass")],inputFingerprint:"fp1"})
            verify(panel.primaryHandoff().kind==="review" && namedItem(panel,"agentActions").visible && fixButton.text==="Review with Pi" && !reviewButton.visible,"a passing theme still offers its agent: a review")
            var card=namedItem(panel,"agentReviewCard"); verify(!card.visible,"no review card without a review")
            panel.reviewAgentName="Pi"; panel.reviewMeta={agent:"pi",requestedAt:1,inputFingerprint:"fp1"}
            verify(card.visible && panel.reviewPending && namedItem(panel,"reviewByline").text==="Waiting for Pi to save the review…" && fixButton.text==="Review again with Pi","requested review is pending")
            panel.reviewText="# t: agent review\n\n## Summary\n\nWarm and *calm* overall.\n\n## Strengths\n\n- ok ![x](file:///etc/passwd) <img src=x>\n"
            verify(!panel.reviewPending && panel.reviewSummary(panel.reviewText)==="Warm and calm overall." && !panel.reviewOutdated,"saved review summary")
            verify(panel.reviewMarkdown(panel.reviewText).indexOf("passwd")<0 && panel.reviewMarkdown(panel.reviewText).indexOf("<img")<0 && panel.reviewMarkdown(panel.reviewText).indexOf("agent review")<0,"review markdown drops images, HTML and the title")
            verify(panel.reviewMarkdown("a ![x][1] ![y]( http://h/(p) ) b\n\n[1]: http://h/i.png").indexOf("![")<0,"review markdown never renders reference or unusual images")
            panel.report=Object.assign({},panel.report,{inputFingerprint:"fp2"}); verify(panel.reviewOutdated && namedItem(panel,"reviewByline").text.indexOf("theme changed since")>=0,"review marked out of date")
            panel.reviewOpen=true; verify(namedItem(panel,"reviewBody").visible,"review opens")
            panel.reviewText=""; panel.reviewMeta=null; panel.reviewOpen=false
            panel.agentMessage="Prompt copied"; verify(panel.agentNote==="Prompt copied","handoff result shown")
            panel.agentPreview="PROMPT"; verify(namedItem(panel,"agentPreviewBox").visible,"prompt preview shown")
            panel.report=Object.assign({},categoryReport,{theme:"another-theme"}); verify(previewClosings===1,"a new theme closes the preview")
            panel.agentPreview=""; panel.agentMessage=""
            panel.errorText="Analyzer error"; verify(namedItem(panel,"errorText").visible,"error shown in header"); panel.errorText=""
            verify(panel.safeColor("#aabbcc")==="#aabbcc" && String(panel.safeColor("red"))===String(panel.surface) && String(panel.safeColor("file:/etc/passwd"))===String(panel.surface),"safe swatches")
            console.log("PASS: live theme palette/alpha/font, caution hue, quick-refresh stability, paused-check grouping/banner, header errors, subject cards/inline checks, agent handoff actions, agent review card, category selection, failure wording, overview palette banks/roles/copy,  passing exploration, positive proof, shared/suppressed honesty, category filters/counts, refresh retention/reset, safe color previews")
        } catch (e) { console.error(String(e)) }
        Qt.callLater(function() { Qt.quit() })
    }
}
