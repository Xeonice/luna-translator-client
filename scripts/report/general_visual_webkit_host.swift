// Isolated approved-source renderer. No Tauri, product bootstrap, HTTP, SQL,
// Keychain, Hook, shortcuts, file picker, or system permission requests.
import AppKit
import WebKit
import CryptoKit
import Darwin

var assets: [String: (String, Int, String)] = [
    "/docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.html": ("e02ecc6fcd4abca9951a79f4d6b4708107848a4e5d8a3430d8de3be1c7b6209c", 6900, "text/html"),
    "/docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.css": ("976d8dbb2695025d779f92b192a211c845dc744ac901997542fd313ce2a6dcbd", 11112, "text/css"),
    "/docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.js": ("74a4d60aa2f517fa762f7b1a6765bb4f06af732509bf9b3b0c21562b9544519f", 9570, "text/javascript"),
    "/docs/design/first-product-v1/material-tokens.css": ("3406083383b5a58d2045fdcbbb26624b04b7f49fe3184599719826a207457a03", 2770, "text/css"),
    "/docs/design/first-product-v1/icons.js": ("4dab92462e14e85bbd541ebdaf81c669d8e078fe8b5461b3cf5125ad219423da", 3501, "text/javascript")
]
let promotedHTML = "/docs/design/first-product-v1/candidates/general-settings-structure-20261107/settings.html"
let capturedHTML = "/.build/current-client-execution-20261105/layout-refinement-implementation-20261107/settings-structure-v3/settings.html"
var htmlPath = promotedHTML
let approvalPath = "/docs/development/design-approval-general-settings-20261107.json"
let approvalHash = "4e5fc7f285de9d58d585fb0e0f46df39631ea38a1cfa261b4cb60114969b9989"
let csp = "default-src 'none'; script-src luna-reference:; style-src luna-reference: 'unsafe-inline'; img-src luna-reference: data:; font-src luna-reference:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'"
func sha(_ data: Data) -> String { SHA256.hash(data: data).map { String(format:"%02x", $0) }.joined() }
func die(_ code: String) -> Never {
    let data = try! JSONSerialization.data(withJSONObject: ["status":"rejected", "reason":code], options:[.sortedKeys])
    FileHandle.standardError.write(data + Data([10])); exit(2)
}
func boundedRead(_ path: URL, limit: Int) throws -> Data {
    let fd = open(path.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC)
    guard fd >= 0 else { throw NSError(domain:"bounded-read", code:1) }
    defer { close(fd) }
    var st = stat(); guard fstat(fd, &st) == 0, (st.st_mode & S_IFMT) == S_IFREG, st.st_size >= 0, st.st_size <= limit else { throw NSError(domain:"bounded-read", code:2) }
    var data = Data(); var bytes = [UInt8](repeating:0,count:8192)
    while true { let n = read(fd,&bytes,bytes.count); if n < 0 { throw NSError(domain:"bounded-read", code:3) }; if n == 0 { break }; data.append(contentsOf:bytes.prefix(n)); guard data.count <= limit else { throw NSError(domain:"bounded-read",code:4) } }
    return data
}
func privateDirectory(_ path: URL) -> Bool {
    var st = stat(); guard lstat(path.path, &st) == 0 else { return false }
    return (st.st_mode & S_IFMT) == S_IFDIR && st.st_uid == getuid() && (st.st_mode & 0o077) == 0
}
func validResource(_ url: URL, main: Bool) -> Bool {
    guard let c=URLComponents(url:url,resolvingAgainstBaseURL:false), c.scheme=="luna-reference", c.host=="approved", c.user==nil,c.password==nil,c.port==nil,c.fragment==nil,c.percentEncodedPath==c.path,assets[c.path] != nil else{return false}
    if main {
        guard c.path==htmlPath else{return false}
        let q=c.queryItems ?? []
        let allowed:Set<String> = htmlPath.hasPrefix("/general-reference/") ? ["theme","zoom","state","context"] : ["theme","zoom"]
        return q.count==allowed.count && Set(q.map(\.name))==allowed && q.allSatisfy { item in
            switch item.name {case "theme":return ["dark","light"].contains(item.value ?? "");case "zoom":return ["1","2"].contains(item.value ?? "");case "context":return item.value=="native-proposal";case "state":return ["normal","empty","loading","loading-initial","read-error","save-failed"].contains(item.value ?? "");default:return false}
        }
    }
    return c.query==nil
}
let states=["normal","empty","loading","error","disabled","long-text","light","narrow","zoom-200","keyboard-focus-escape","scope-global","scope-game","restore-inheritance","save-failed"].map{"all-page-settings-general-"+$0}
struct Request { let nonce:String;let width:Int;let height:Int;let theme:String;let zoom:Int;let mode:String;let state:String;let subcase:String;let bundle:String;let manifest:String
    var qualification:Bool {mode=="source-qualification" || mode=="source-saved-game"}
    var context:[String:Any] {["schemaVersion":1,"kind":"general-native-visual-context/v1","transport":"real_react","side":mode=="general-reference" ? "reference":"candidate","stateId":state,"subcase":subcase,"canvasId":"\(width)x\(height)-\(theme)-z\(zoom)","nonce":nonce,"width":width,"height":height,"theme":theme,"zoom":zoom]}
}
func parseRequest(_ data:Data,folder:URL)->Request? {
    guard let j=try? JSONSerialization.jsonObject(with:data) as? [String:Any],Set(j.keys)==Set(["schema_version","kind","execution_nonce","width","height","theme","zoom","mode","state_id","subcase","bundle","bundle_manifest_sha256"]) else{return nil}
    guard let canonical=try? JSONSerialization.data(withJSONObject:j,options:[.sortedKeys,.withoutEscapingSlashes]),data==canonical+Data([10]) else{return nil}
    func integer(_ k:String)->Int? {guard let x=j[k] as? NSNumber,CFGetTypeID(x) != CFBooleanGetTypeID(),x.doubleValue.isFinite,x.doubleValue==Double(x.intValue) else{return nil};return x.intValue}
    guard integer("schema_version")==1,j["kind"] as? String=="general-webkit-observer-request/v1",let nonce=j["execution_nonce"] as? String,nonce.count==32,nonce.utf8.allSatisfy({(48...57).contains($0)||(97...102).contains($0)}),folder.lastPathComponent==nonce,
          let w=integer("width"),let h=integer("height"),[[1000,720],[760,560]].contains([w,h]),let theme=j["theme"] as? String,["dark","light"].contains(theme),let z=integer("zoom"),[1,2].contains(z),
          let mode=j["mode"] as? String,["source-qualification","source-saved-game","general-reference","general-candidate"].contains(mode),let state=j["state_id"] as? String,states.contains(state),let sub=j["subcase"] as? String,
          let bundle=j["bundle"] as? String,let manifest=j["bundle_manifest_sha256"] as? String else{return nil}
    guard sub=="primary" || (sub=="initial" && state==states[2]) || (sub=="menu-open" && state==states[9]) || (sub=="content-end" && state==states[0] && z==2) else{return nil}
    if mode.hasPrefix("source-") {guard state==states[0],sub=="primary",bundle=="",manifest=="" else{return nil}}
    else {guard bundle.hasPrefix(".build/"),!bundle.contains(".."),!bundle.contains("\\"),!bundle.contains(":"),!bundle.hasSuffix("/"),manifest.count==64,manifest.utf8.allSatisfy({(48...57).contains($0)||(97...102).contains($0)}) else{return nil}}
    return Request(nonce:nonce,width:w,height:h,theme:theme,zoom:z,mode:mode,state:state,subcase:sub,bundle:bundle,manifest:manifest)
}

final class ClosedResourceScheme: NSObject, WKURLSchemeHandler {
    let data: [String:Data]; var events = [[String:Any]]()
    init(_ data:[String:Data]) { self.data=data }
    func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url=urlSchemeTask.request.url, (urlSchemeTask.request.httpMethod ?? "GET") == "GET", validResource(url,main:url.path == htmlPath), let bytes=data[url.path],let asset=assets[url.path] else {
            events.append(["decision":"rejected","url":urlSchemeTask.request.url?.absoluteString ?? "nil"])
            urlSchemeTask.didFailWithError(NSError(domain:"closed-resource",code:1)); return
        }
        events.append(["decision":"served","path":url.path,"sha256":sha(bytes),"bytes":bytes.count])
        let response=HTTPURLResponse(url:url,statusCode:200,httpVersion:"HTTP/1.1",headerFields:["Content-Type":asset.2+"; charset=utf-8","Content-Security-Policy":csp,"Cache-Control":"no-store","Access-Control-Allow-Origin":"*","Cross-Origin-Resource-Policy":"cross-origin"] )!
        urlSchemeTask.didReceive(response); urlSchemeTask.didReceive(bytes); urlSchemeTask.didFinish()
    }
    func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {}
}
final class NonActivatingPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

final class Host: NSObject, NSApplicationDelegate, WKNavigationDelegate, WKUIDelegate {
    let req:Request; let output:URL; let source:[String:Data]
    var panel:NonActivatingPanel!; var web:WKWebView!; var scheme:ClosedResourceScheme!
    var readiness=[[String:Any]]();var phase=0; var captures=[[String:Any]]();var policyEvents=[[String:Any]]();var securityProbe=[String:Any](); var finished=false
    let started=Date(); let frontBefore=NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1
    init(_ req:Request,_ output:URL,_ source:[String:Data]) { self.req=req;self.output=output;self.source=source }
    func applicationDidFinishLaunching(_ notification:Notification) {
        panel=NonActivatingPanel(contentRect:NSRect(x:0,y:0,width:req.width,height:req.height),styleMask:[.borderless,.nonactivatingPanel],backing:.buffered,defer:false)
        panel.title="REFERENCE ONLY · NOT LUNA";panel.isFloatingPanel=false;panel.hidesOnDeactivate=false;panel.hasShadow=false;panel.ignoresMouseEvents=true;panel.isReleasedWhenClosed=false
        panel.appearance=NSAppearance(named:req.theme == "light" ? .aqua : .darkAqua)
        if let screen=NSScreen.main { panel.setFrameOrigin(NSPoint(x:screen.visibleFrame.minX+20,y:screen.visibleFrame.maxY-CGFloat(req.height)-20)) }
        startPhase()
        DispatchQueue.main.asyncAfter(deadline:.now()+25) { [weak self] in if let self,!self.finished { self.finish("failed", "native_background_render_timeout") } }
    }
    func startPhase() {
        if web != nil { web.stopLoading();web.navigationDelegate=nil;web.uiDelegate=nil;web.removeFromSuperview() }
        phase += 1
        let config=WKWebViewConfiguration();config.websiteDataStore = .nonPersistent();config.preferences.javaScriptCanOpenWindowsAutomatically=false
        scheme=ClosedResourceScheme(source);config.setURLSchemeHandler(scheme,forURLScheme:"luna-reference")
        // Diagnostics only; this cannot issue Native commands or change the approved document.
        let diagnostics="window.__sourceHostDiagnostics={errors:[],csp:[]};addEventListener('error',e=>__sourceHostDiagnostics.errors.push(String(e.message)));addEventListener('unhandledrejection',e=>__sourceHostDiagnostics.errors.push(String(e.reason)));addEventListener('securitypolicyviolation',e=>__sourceHostDiagnostics.csp.push({directive:e.violatedDirective,blocked:e.blockedURI}));"
        config.userContentController.addUserScript(WKUserScript(source:diagnostics,injectionTime:.atDocumentStart,forMainFrameOnly:true))
        if !req.qualification {
            let context=String(data:try! JSONSerialization.data(withJSONObject:req.context,options:[.sortedKeys,.withoutEscapingSlashes]),encoding:.utf8)!
            var initializer="Object.defineProperty(window,'__LUNA_GENERAL_VISUAL__',{value:Object.freeze("+context+"),writable:false,configurable:false});"
            if req.mode=="general-reference" {initializer += "addEventListener('DOMContentLoaded',()=>import('luna-reference://approved/general-reference/replay.js').catch(e=>{window.__GENERAL_VISUAL_RESULT__={error:String(e)}}),{once:true});"}
            config.userContentController.addUserScript(WKUserScript(source:initializer,injectionTime:.atDocumentStart,forMainFrameOnly:true))
        }
        web=WKWebView(frame:NSRect(x:0,y:0,width:req.width,height:req.height),configuration:config);web.navigationDelegate=self;web.uiDelegate=self
        web.autoresizingMask=[.width,.height]
        if !req.qualification {web.underPageBackgroundColor = .clear;panel.isOpaque=false;panel.backgroundColor = .clear}
        panel.contentView=web;panel.orderBack(nil)
        if req.qualification {htmlPath = phase==1 ? capturedHTML:promotedHTML} else {htmlPath=req.mode=="general-reference" ? "/general-reference/settings.html":"/general-visual.html"}
        var query="?theme="+req.theme+"&zoom="+String(req.zoom)
        if req.mode=="general-reference" {let suffix=req.state.replacingOccurrences(of:"all-page-settings-general-",with:"");let state=suffix=="loading" ? (req.subcase=="initial" ? "loading-initial":"loading"):suffix=="error" ? "read-error":["empty","save-failed"].contains(suffix) ? suffix:"normal";query += "&state="+state+"&context=native-proposal"}
        let u=URL(string:"luna-reference://approved"+htmlPath+query)!
        web.load(URLRequest(url:u))
    }
    func webView(_ webView:WKWebView,decidePolicyFor navigationAction:WKNavigationAction,decisionHandler:@escaping(WKNavigationActionPolicy)->Void) {
        let allowed=navigationAction.targetFrame?.isMainFrame == true && navigationAction.request.url.map { validResource($0,main:true) } == true
        policyEvents.append(["kind":"navigation","allowed":allowed,"url":navigationAction.request.url?.absoluteString ?? "nil"]);decisionHandler(allowed ? .allow : .cancel)
    }
    func webView(_ webView:WKWebView,decidePolicyFor navigationResponse:WKNavigationResponse,decisionHandler:@escaping(WKNavigationResponsePolicy)->Void) {
        let allowed=navigationResponse.isForMainFrame && navigationResponse.response.url.map { validResource($0,main:true) } == true && navigationResponse.canShowMIMEType
        policyEvents.append(["kind":"response","allowed":allowed]);decisionHandler(allowed ? .allow : .cancel)
    }
    func webView(_ webView:WKWebView,didFinish navigation:WKNavigation!) { readyThenCapture(1) }
    func webView(_ webView:WKWebView,didFail navigation:WKNavigation!,withError error:Error) { finish("failed","navigation_failed_"+String((error as NSError).code)) }
    func webView(_ webView:WKWebView,didFailProvisionalNavigation navigation:WKNavigation!,withError error:Error) { finish("failed","provisional_navigation_failed_"+String((error as NSError).code)) }
    func webView(_ webView:WKWebView,createWebViewWith configuration:WKWebViewConfiguration,for navigationAction:WKNavigationAction,windowFeatures:WKWindowFeatures)->WKWebView? { policyEvents.append(["kind":"new_window","allowed":false]);return nil }
    func webView(_ webView:WKWebView,runOpenPanelWith parameters:WKOpenPanelParameters,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping([URL]?)->Void) { policyEvents.append(["kind":"file_panel","allowed":false]);completionHandler(nil) }
    @available(macOS 12.0,*) func webView(_ webView:WKWebView,requestMediaCapturePermissionFor origin:WKSecurityOrigin,initiatedByFrame frame:WKFrameInfo,type:WKMediaCaptureType,decisionHandler:@escaping(WKPermissionDecision)->Void) { policyEvents.append(["kind":"media_permission","allowed":false]);decisionHandler(.deny) }
    func webView(_ webView:WKWebView,runJavaScriptAlertPanelWithMessage message:String,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping()->Void) { policyEvents.append(["kind":"alert","allowed":false]);completionHandler() }
    func webView(_ webView:WKWebView,runJavaScriptConfirmPanelWithMessage message:String,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping(Bool)->Void) { policyEvents.append(["kind":"confirm","allowed":false]);completionHandler(false) }
    func webView(_ webView:WKWebView,runJavaScriptTextInputPanelWithPrompt prompt:String,defaultText:String?,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping(String?)->Void) { policyEvents.append(["kind":"prompt","allowed":false]);completionHandler(nil) }
    func readyThenCapture(_ number:Int) {
        let js="""
        await document.fonts.ready;
        await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
        const rect=s=>{const n=document.querySelector(s);if(!n)return null;const r=n.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,scrollHeight:n.scrollHeight,clientHeight:n.clientHeight,scrollTop:n.scrollTop}};
        return {href:location.href,viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},theme:document.documentElement.dataset.theme,zoom:getComputedStyle(document.documentElement).zoom,fonts:document.fonts.status,motionReduced:matchMedia('(prefers-reduced-motion: reduce)').matches,visibility:document.visibilityState,animations:document.getAnimations().filter(a=>a.playState==='running'||a.pending).length,state:window.SettingsStructureCandidate?.state,saved:window.SettingsStructureCandidate?.saved,effectiveGame:window.SettingsStructureCandidate?.effectiveGame,bodyBackground:getComputedStyle(document.body).backgroundColor,window:rect('.window'),content:rect('.content'),footer:rect('.footer'),title:rect('.titlebar'),sidebar:rect('.sidebar'),groups:[...document.querySelectorAll('.group h2')].map(n=>n.textContent),h1Font:{family:getComputedStyle(document.querySelector('h1')).fontFamily,size:getComputedStyle(document.querySelector('h1')).fontSize,weight:getComputedStyle(document.querySelector('h1')).fontWeight},diagnostics:window.__sourceHostDiagnostics,userAgent:navigator.userAgent};
        """
        let savedRecipe="""
        if(!window.__sourceSavedRecipeDone){window.__sourceSavedRecipeDone=true;document.querySelector('#scope-button').click();document.querySelector('[data-scope=game]').click();document.querySelector('#ruby-switch').click();document.querySelector('#save').click();}
        const until=performance.now()+4000;while(!document.querySelector('#toast').hidden&&performance.now()<until)await new Promise(r=>setTimeout(r,20));
        """
        let generalReady="""
        const until=performance.now()+12000;while(!window.__GENERAL_VISUAL_RESULT__&&performance.now()<until)await new Promise(r=>setTimeout(r,20));
        await document.fonts.ready;
        const animationDeadline=performance.now()+1500;
        while(document.getAnimations().some(a=>a.playState==='running'||a.pending)&&performance.now()<animationDeadline)await new Promise(r=>setTimeout(r,16));
        await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
        return {observation:window.__GENERAL_VISUAL_RESULT__||null,viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},fonts:document.fonts.status,animations:document.getAnimations().filter(a=>a.playState==='running'||a.pending).length,diagnostics:window.__sourceHostDiagnostics,userAgent:navigator.userAgent};
        """
        let actualJS=req.qualification ? ((req.mode=="source-saved-game" ? savedRecipe:"")+js):generalReady
        web.callAsyncJavaScript(actualJS,arguments:[:],in:nil,in:.page) { [weak self] result in
            guard let self,!self.finished else{return}
            switch result {
            case .failure(let error):self.finish("failed","readiness_failed_"+String((error as NSError).code))
            case .success(let value):
                if let facts=value as? [String:Any]{self.readiness.append(facts)}
                guard let facts=value as? [String:Any],facts["fonts"] as? String=="loaded",facts["animations"] as? Int==0,let diagnostics=facts["diagnostics"] as? [String:Any],(diagnostics["errors"] as? [Any])?.isEmpty==true,(diagnostics["csp"] as? [Any])?.isEmpty==true else{self.finish("failed","document_not_ready");return}
                if self.req.qualification {
                    guard facts["theme"] as? String==self.req.theme,let state=facts["state"] as? [String:Any],state["scope"] as? String==(self.req.mode=="source-saved-game" ? "game":"global"),state["dirty"] as? Bool==false,let groups=facts["groups"] as? [String],groups==["分词与词典","阅读记录","本机偏好"] else{self.finish("failed","source_state_not_ready");return}
                } else {
                    guard let observation=facts["observation"] as? [String:Any],observation["error"] is NSNull,let context=observation["context"] as? [String:Any],NSDictionary(dictionary:context).isEqual(to:self.req.context) else{self.finish("failed","real_component_observation_failed");return}
                }
                self.snapshot(number,facts)
            }
        }
    }
    func snapshot(_ number:Int,_ facts:[String:Any]) {
        let config=WKSnapshotConfiguration();config.rect=web.bounds;config.afterScreenUpdates=true
        web.takeSnapshot(with:config) { [weak self] image,error in
            guard let self,!self.finished else{return}
            guard error == nil,let image,let cg=image.cgImage(forProposedRect:nil,context:nil,hints:nil),let png=NSBitmapImageRep(cgImage:cg).representation(using:.png,properties:[:]) else {self.finish("failed","snapshot_failed");return}
            let name="document-\(self.phase)-frame-\(number)";let path=self.output.appendingPathComponent(name+".png")
            do {try png.write(to:path,options:.withoutOverwriting);try FileManager.default.setAttributes([.posixPermissions:0o600],ofItemAtPath:path.path)} catch {self.finish("failed","snapshot_write_failed");return}
            let wid=CGWindowID(self.panel.windowNumber)
            let windows=CGWindowListCopyWindowInfo([.optionIncludingWindow],wid) as? [[String:Any]] ?? []
            let own=windows.filter{($0[kCGWindowOwnerPID as String] as? Int)==Int(getpid())}
            let entry:[String:Any]=["name":name,"png_sha256":sha(png),"png_bytes":png.count,"pixel_width":cg.width,"pixel_height":cg.height,"image_size_points":[image.size.width,image.size.height],"cg_color_space":cg.colorSpace?.name as String? ?? "unknown","nonpersistent_data_store":!self.web.configuration.websiteDataStore.isPersistent,"facts":facts,"resources":self.scheme.events,"native":["pid":getpid(),"window_id":wid,"window_frame":[self.panel.frame.minX,self.panel.frame.minY,self.panel.frame.width,self.panel.frame.height],"content_bounds":[self.web.bounds.minX,self.web.bounds.minY,self.web.bounds.width,self.web.bounds.height],"backing_scale":self.panel.backingScaleFactor,"is_visible":self.panel.isVisible,"is_on_active_space":self.panel.isOnActiveSpace,"occlusion_state":self.panel.occlusionState.rawValue,"cg_own_windows":own,"frontmost_pid":NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1]]
            self.captures.append(entry)
            if number == 1 {self.readyThenCapture(2)} else if self.phase == 1 {self.startPhase()} else {self.probeClosedPolicy()}
        }
    }
    func probeClosedPolicy() {
        // After all four immutable reference frames: a local virtual-resource
        // fetch probes real CSP enforcement without attempting any network URL.
        let previous=scheme.events.count
        let js="""
        let fetched=false;try{await fetch('luna-reference://approved/proof-not-allowlisted');fetched=true}catch(e){fetched=false}
        await new Promise(r=>setTimeout(r,0));
        const popup=window.open(location.href,'_blank');
        return {localFetchResolved:fetched,cspViolations:window.__sourceHostDiagnostics.csp,popupReturned:popup!==null};
        """
        web.callAsyncJavaScript(js,arguments:[:],in:nil,in:.page) { [weak self] result in
            guard let self,!self.finished else{return}
            switch result {
            case .failure: self.finish("failed","closed_policy_probe_execution_failed")
            case .success(let value):
                guard let proof=value as? [String:Any],let violations=proof["cspViolations"] as? [[String:Any]],violations.contains(where:{($0["directive"] as? String)?.hasPrefix("connect-src") == true}),proof["localFetchResolved"] as? Bool == false,proof["popupReturned"] as? Bool == false,self.scheme.events.count == previous else{self.finish("failed","closed_policy_probe_not_proven");return}
                self.securityProbe=["status":"passed","phase":"after_all_four_reference_frames","local_virtual_fetch_csp":proof,"resource_handler_received_probe":false,"external_network_attempted":false,"system_permission_probe_attempted":false]
                self.finish("captured","raw_comparison_pending_driver")
            }
        }
    }
    func finish(_ status:String,_ reason:String) {
        guard !finished else{return};finished=true
            let record:[String:Any]=["schema_version":1,"kind":"general-webkit-observer-capture/v1","status":status,"reason":reason,"evidence_role":"native_component_render","subject":req.qualification ? "approved_source_relocation":req.mode,"controlled_transport":true,"product_port_loaded":req.mode=="general-candidate","packaged_native":false,"product_accepted":false,"reference_png_approved":false,"state_id":req.state,"subcase":req.subcase,"mode":req.mode,"adaptation":req.qualification ? "source_body_unchanged":"approved_native_content","execution_nonce":req.nonce,"pid":getpid(),"os":ProcessInfo.processInfo.operatingSystemVersionString,"webkit_framework_version":Bundle(identifier:"com.apple.WebKit")?.object(forInfoDictionaryKey:"CFBundleVersion") as? String ?? "unknown","started_at":ISO8601DateFormatter().string(from:started),"finished_at":ISO8601DateFormatter().string(from:Date()),"frontmost_pid_before":frontBefore,"frontmost_pid_after":NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1,"activation_requested":false,"source_assets":assets.mapValues{["sha256":$0.0,"bytes":$0.1] as [String:Any]},"csp":csp,"request":["width":req.width,"height":req.height,"theme":req.theme,"zoom":req.zoom],"policy_events":policyEvents,"readiness":readiness,"post_capture_security_probe":securityProbe,"captures":captures]
        do {let data=try JSONSerialization.data(withJSONObject:record,options:[.prettyPrinted,.sortedKeys]);try data.write(to:output.appendingPathComponent("native-result.json"),options:.withoutOverwriting)} catch {FileHandle.standardError.write(Data("receipt_write_failed\n".utf8))}
        panel?.orderOut(nil);web?.stopLoading();NSApplication.shared.stop(nil);exit(status == "captured" ? 0 : 1)
    }
}

let repository=URL(fileURLWithPath:#filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().standardizedFileURL
let args=Array(CommandLine.arguments.dropFirst());let validateOnly=args.last=="--validate-only"
guard (args.count==2 || (args.count==3 && validateOnly)),args[0]=="--request" else{die("arguments_not_allowlisted")}
let requestURL=URL(fileURLWithPath:args[1]).standardizedFileURL
let outputFolder=requestURL.deletingLastPathComponent()
guard requestURL.lastPathComponent=="request.json",requestURL.path.hasPrefix(repository.appendingPathComponent(".build").path+"/"),requestURL.path==requestURL.resolvingSymlinksInPath().path,privateDirectory(outputFolder),let data=try? boundedRead(requestURL,limit:8192),let req=parseRequest(data,folder:outputFolder) else{die("invalid_closed_request")}
var source=[String:Data]()
if req.qualification {
    for (path,asset) in assets {let url=repository.appendingPathComponent(String(path.dropFirst()));guard url.path==url.resolvingSymlinksInPath().path,let d=try? boundedRead(url,limit:131072),d.count==asset.1,sha(d)==asset.0 else{die("approved_source_identity_failed")};source[path]=d}
    let capturedRoot="docs/design/first-product-v1/candidates/general-settings-structure-20261107/captured/"
    let oldHTML=repository.appendingPathComponent(capturedRoot+"settings.html")
    guard let original=try? boundedRead(oldHTML,limit:131072),original.count==6970,sha(original)=="6734155af5a9cd9251b21f665156c67a0c97bb473c162ddb9856e3efcd674b9a" else{die("captured_source_identity_failed")}
    source[capturedHTML]=original;assets[capturedHTML]=(sha(original),original.count,"text/html")
    for name in ["settings.css","settings.js"] {let key=capturedHTML.replacingOccurrences(of:"settings.html",with:name);let current=promotedHTML.replacingOccurrences(of:"settings.html",with:name);source[key]=source[current];assets[key]=assets[current]}
    guard let approved=try? boundedRead(repository.appendingPathComponent(String(approvalPath.dropFirst())),limit:32768),sha(approved)==approvalHash else{die("approval_identity_failed")}
} else {
    let bundle=repository.appendingPathComponent(req.bundle)
    guard bundle.path==bundle.resolvingSymlinksInPath().path,let manifestBytes=try? boundedRead(bundle.appendingPathComponent("general-assets.json"),limit:262144),sha(manifestBytes)==req.manifest,let manifest=try? JSONSerialization.jsonObject(with:manifestBytes) as? [String:Any],manifest["kind"] as? String=="general-closed-vite-artifacts/v1",let resources=manifest[req.mode=="general-reference" ? "reference":"candidate"] as? [String:[String:Any]],resources.count>0,resources.count<=128 else{die("bundle_manifest_invalid")}
    assets=[:]
    for (path,row) in resources {
        guard path.hasPrefix("/"),!path.contains(".."),!path.contains("\\"),!path.contains(":"),!path.contains("?"),!path.contains("#"),path.range(of:"^/(assets/[A-Za-z0-9_.-]+|general-reference/(assets/)?[A-Za-z0-9_.-]+|general-reference/source/[A-Za-z0-9_.-]+|general-visual.html)$",options:.regularExpression) != nil,let expected=row["sha256"] as? String,let count=row["bytes"] as? Int,count>0,count<=16000000,let mime=row["mime"] as? String,["text/html","text/javascript","text/css","image/svg+xml","font/woff2","application/octet-stream"].contains(mime) else{die("bundle_resource_not_finite")}
        let url=bundle.appendingPathComponent(String(path.dropFirst()));guard url.path==url.resolvingSymlinksInPath().path,let bytes=try? boundedRead(url,limit:16000000),bytes.count==count,sha(bytes)==expected else{die("bundle_resource_identity_failed")};source[path]=bytes;assets[path]=(expected,count,mime)
    }
    htmlPath=req.mode=="general-reference" ? "/general-reference/settings.html":"/general-visual.html"
    guard assets[htmlPath] != nil else{die("bundle_entry_absent")}
}
if validateOnly {print("{\"status\":\"validated_closed_observer\",\"native_started\":false}");exit(0)}
guard !FileManager.default.fileExists(atPath:outputFolder.appendingPathComponent("native-result.json").path) else{die("fresh_output_required")}
let app=NSApplication.shared;app.setActivationPolicy(.accessory)
let host=Host(req,outputFolder,source);app.delegate=host;app.run()
