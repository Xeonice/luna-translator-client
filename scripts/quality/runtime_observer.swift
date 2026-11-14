// Independent read-only OS observation. Never injects input, changes geometry,
// requests permission, or treats server metadata as a rendered game frame.
import AppKit
import ApplicationServices
import Security
import Vision
import Darwin

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message+"\n").utf8)); exit(2)
}
#if !RUNTIME_OBSERVER_NATIVE_TEST
let args = CommandLine.arguments
func value(_ flag: String) -> String? {
    guard let i = args.firstIndex(of: flag), i+1 < args.count else { return nil }; return args[i+1]
}
guard let output = value("--output"), let raw = value("--pids") else { fail("explicit --output and --pids required") }
let ids = raw.split(separator: ",").compactMap { Int32($0) }
guard !ids.isEmpty, Set(ids).count == ids.count, ids.allSatisfy({$0 > 0}), ids.count <= 12 else { fail("invalid bounded PID inventory") }
let windowsWanted = Set((value("--windows") ?? "").split(separator:",").compactMap { UInt32($0) })
let directory = URL(fileURLWithPath: output).deletingLastPathComponent()
guard !FileManager.default.fileExists(atPath: output) else { fail("fresh output required") }
try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
#endif
func ax(_ node: AXUIElement, _ name: CFString) -> CFTypeRef? {
    var value: CFTypeRef?; return AXUIElementCopyAttributeValue(node,name,&value) == .success ? value : nil
}
func axText(_ pid: pid_t) -> [[String:Any]] {
    var queue = [AXUIElementCreateApplication(pid)], result = [[String:Any]](), count = 0
    while !queue.isEmpty && count < 2500 {
        let node = queue.removeFirst(); count += 1; var row = [String:Any]()
        for (key, name) in [("role",kAXRoleAttribute),("title",kAXTitleAttribute),("value",kAXValueAttribute),("description",kAXDescriptionAttribute)] {
            if let text = ax(node,name as CFString) as? String, !text.isEmpty { row[key] = String(text.prefix(8192)) }
        }
        if let pv=ax(node,kAXPositionAttribute as CFString), let sv=ax(node,kAXSizeAttribute as CFString), CFGetTypeID(pv)==AXValueGetTypeID(), CFGetTypeID(sv)==AXValueGetTypeID() {
            var pt=CGPoint.zero, sz=CGSize.zero
            if AXValueGetValue(pv as! AXValue,.cgPoint,&pt), AXValueGetValue(sv as! AXValue,.cgSize,&sz) {row["frame"]=serializedAXFrame(pt,sz)}
        }
        if !row.isEmpty { result.append(row) }
        if let children = ax(node,kAXChildrenAttribute as CFString) as? [AXUIElement] { queue.append(contentsOf: children.prefix(2500-count)) }
    }
    return result
}
func signature(_ pid: pid_t) -> [String:Any] {
    var guest: SecCode?; let attrs = [kSecGuestAttributePid as String: NSNumber(value:pid)] as CFDictionary
    let code = SecCodeCopyGuestWithAttributes(nil,attrs,SecCSFlags(),&guest)
    guard code == errSecSuccess, let guest else { return ["guest_status":Int(code)] }
    let validity = SecCodeCheckValidity(guest,SecCSFlags(rawValue:kSecCSStrictValidate),nil)
    var staticCode: SecStaticCode?; let copy = SecCodeCopyStaticCode(guest,SecCSFlags(),&staticCode)
    guard copy == errSecSuccess, let staticCode else { return ["guest_status":Int(code),"validity":Int(validity),"static_status":Int(copy)] }
    var info: CFDictionary?; let status = SecCodeCopySigningInformation(staticCode,SecCSFlags(rawValue:kSecCSSigningInformation),&info)
    let d = info as? [String:Any] ?? [:]
    var result: [String:Any] = ["guest_status":Int(code),"validity":Int(validity),"information_status":Int(status)]
    for (name,key) in [("identifier",kSecCodeInfoIdentifier),("team",kSecCodeInfoTeamIdentifier)] { if let v=d[key as String] as? String {result[name]=v} }
    if let v=d[kSecCodeInfoUnique as String] as? Data {result["cdhash"]=v.map{String(format:"%02x",$0)}.joined()}
    return result
}
func process(_ pid: pid_t) -> [String:Any] {
    var b = proc_bsdinfo(); let n = proc_pidinfo(pid,PROC_PIDTBSDINFO,0,&b,Int32(MemoryLayout<proc_bsdinfo>.size))
    var buffer=[CChar](repeating:0,count:4 * Int(MAXPATHLEN))
    let size=proc_pidpath(pid,&buffer,UInt32(buffer.count))
    var r:[String:Any]=["pid":Int(pid),"bsd_bytes":Int(n),"path_bytes":Int(size)]
    if n == MemoryLayout<proc_bsdinfo>.size {r["start_sec"]=b.pbi_start_tvsec;r["start_usec"]=b.pbi_start_tvusec;r["ppid"]=b.pbi_ppid}
    if size > 0 {
        let path=String(cString:buffer);r["executable"]=path
        if let a=try? FileManager.default.attributesOfItem(atPath:path) {r["device"]=a[.systemNumber];r["inode"]=a[.systemFileNumber]}
    }
    r["guest_code"]=signature(pid);r["ax"]=axText(pid); return r
}
func recognized(_ cg: CGImage) -> [[String:Any]]? {
    let request=VNRecognizeTextRequest();request.recognitionLevel = .accurate
    request.recognitionLanguages=["ja-JP","zh-Hans","en-US"];request.usesLanguageCorrection=false
    do {try VNImageRequestHandler(cgImage:cg).perform([request])} catch {return nil}
    return (request.results ?? []).compactMap {obs -> [String:Any]? in
        guard let text=obs.topCandidates(1).first else{return nil}
        return ["text":text.string,"confidence":text.confidence,"bounds":[obs.boundingBox.origin.x,obs.boundingBox.origin.y,obs.boundingBox.width,obs.boundingBox.height]]
    }
}
// CGFloat has no conditional Array<Double> cast. Preserve one numeric frame
// representation before storing native AX values in an Any-valued dictionary.
func serializedAXFrame(_ point: CGPoint, _ size: CGSize) -> [Double] {
    [Double(point.x), Double(point.y), Double(size.width), Double(size.height)]
}
func windowCaptureArguments(_ wid: CGWindowID, _ target: URL) -> [String] {
    // Request one window without its shadow or attached-window family.
    ["-x", "-a", "-o", "-l", String(wid), target.path]
}
struct CaptureCanvasValidation {
    let failure: String?
    let diagnostic: [String:Any]
}
func validateCaptureCanvas(_ bounds: [String:Any], _ image: CGImage) -> CaptureCanvasValidation {
    var safeBounds=[String:Any]()
    for key in ["X","Y","Width","Height"] {
        if let number=bounds[key] as? Double {
            safeBounds[key]=number.isFinite ? number as Any : "nonfinite_"+String(describing:number)
        } else {safeBounds[key]=bounds[key] == nil ? "missing" : "invalid_numeric_type"}
    }
    var diagnostic:[String:Any]=["pixel_width":image.width,"pixel_height":image.height,
        "selected_window_bounds":safeBounds,"actual_canvas_origin_observed":false,
        "canvas_origin_basis":"selected_cg_bounds_assumed_after_single_window_request"]
    func rejected(_ reason:String) -> CaptureCanvasValidation {
        var details=diagnostic;details["status"]="rejected";details["reason"]=reason
        return CaptureCanvasValidation(failure:reason,diagnostic:details)
    }
    guard let x=bounds["X"] as? Double, let y=bounds["Y"] as? Double,
          let w=bounds["Width"] as? Double, let h=bounds["Height"] as? Double,
          [x,y,w,h].allSatisfy({$0.isFinite}), w>0, h>0, image.width>0, image.height>0 else {
        return rejected("capture_canvas_invalid_bounds")
    }
    let scaleX=Double(image.width)/w, scaleY=Double(image.height)/h
    guard scaleX.isFinite, scaleY.isFinite, scaleX>0, scaleY>0 else {
        diagnostic["pixel_scales_valid"]=false
        return rejected("capture_canvas_invalid_scale")
    }
    diagnostic["pixel_scale_x"]=scaleX;diagnostic["pixel_scale_y"]=scaleY
    let widthProduct=Double(image.width)*h, heightProduct=Double(image.height)*w
    // This is a strict dimension consistency check, not proof of canvas
    // ownership/origin. No scale=1/2 rule or OCR threshold is assumed. A
    // fractional-point/rounded-pixel mismatch remains explicitly unavailable.
    guard widthProduct.isFinite, heightProduct.isFinite,
          widthProduct==heightProduct else {return rejected("capture_canvas_scale_mismatch")}
    diagnostic["status"]="uniform_dimensions";diagnostic["pixel_scale"]=scaleX
    return CaptureCanvasValidation(failure:nil,diagnostic:diagnostic)
}
struct CapturedWindowText {
    let geometry: CaptureCanvasValidation
    let lines: [[String:Any]]?
    let failure: String?
}
func observeCapturedWindowText(_ bounds: [String:Any], _ image: CGImage,
                               recognize: (CGImage) -> [[String:Any]]? = recognized) -> CapturedWindowText {
    let geometry=validateCaptureCanvas(bounds,image)
    if let failure=geometry.failure {return CapturedWindowText(geometry:geometry,lines:nil,failure:failure)}
    guard let lines=recognize(image) else {
        return CapturedWindowText(geometry:geometry,lines:nil,failure:"ocr_unavailable")
    }
    return CapturedWindowText(geometry:geometry,lines:lines,failure:nil)
}
enum PreviewCropFailure: String, Error {
    case windowBoundsInvalid = "window_bounds_invalid"
    case axFrameTypeOrCountInvalid = "ax_frame_type_or_count_invalid"
    case axFrameInvalid = "ax_frame_invalid"
    case axFrameOutsideWindow = "ax_frame_outside_window"
    case sourcePixelsOutsideImage = "source_pixels_outside_image"
    case imageCropUnavailable = "image_crop_unavailable"
    case cropPNGEncodingFailed = "crop_png_encoding_failed"
    case cropOCRUnavailable = "crop_ocr_unavailable"
}
func previewPixelRect(_ bounds: [String:Any], _ frame: [Double], _ width: Int, _ height: Int) throws -> CGRect {
    guard let x=bounds["X"] as? Double, let y=bounds["Y"] as? Double,
          let w=bounds["Width"] as? Double, let h=bounds["Height"] as? Double,
          [x,y,w,h].allSatisfy({$0.isFinite}), w>0, h>0, width>0, height>0 else {
        throw PreviewCropFailure.windowBoundsInvalid
    }
    guard frame.count==4, frame.allSatisfy({$0.isFinite}), frame[2]>1, frame[3]>1 else {
        throw PreviewCropFailure.axFrameInvalid
    }
    let outer=CGRect(x:x,y:y,width:w,height:h)
    let rect=CGRect(x:frame[0],y:frame[1],width:frame[2],height:frame[3])
    guard outer.contains(rect) else { throw PreviewCropFailure.axFrameOutsideWindow }
    let pixels=CGRect(x:(rect.minX-x)*Double(width)/w,y:(rect.minY-y)*Double(height)/h,
                      width:rect.width*Double(width)/w,height:rect.height*Double(height)/h).integral
    guard CGRect(x:0,y:0,width:width,height:height).contains(pixels) else {
        throw PreviewCropFailure.sourcePixelsOutsideImage
    }
    return pixels
}
struct PreparedPreviewCrop {
    let png: Data
    let metadata: [String:Any]
    let reason: String
}
func preparePreviewCrop(_ cg: CGImage, _ bounds: [String:Any], _ node: [String:Any],
                        recognize: (CGImage) -> [[String:Any]]? = recognized) throws -> PreparedPreviewCrop {
    guard let frame=node["frame"] as? [Double], frame.count==4 else {
        throw PreviewCropFailure.axFrameTypeOrCountInvalid
    }
    let pixels=try previewPixelRect(bounds,frame,cg.width,cg.height)
    guard let image=cg.cropping(to:pixels) else { throw PreviewCropFailure.imageCropUnavailable }
    guard let data=NSBitmapImageRep(cgImage:image).representation(using:.png,properties:[:]) else {
        throw PreviewCropFailure.cropPNGEncodingFailed
    }
    guard let words=recognize(image) else { throw PreviewCropFailure.cropOCRUnavailable }
    // Empty OCR is a successfully saved image, not a failed/missing crop. The
    // independent evidence validator still rejects its absent sentence evidence.
    return PreparedPreviewCrop(png:data,metadata:["ocr":words,"width":image.width,"height":image.height,
        "actual_ax_frame":frame,"source_pixel_rect":[Double(pixels.minX),Double(pixels.minY),Double(pixels.width),Double(pixels.height)],
        "description":node["description"] ?? node["title"] ?? ""],reason:words.isEmpty ? "crop_saved_ocr_empty" : "crop_saved")
}
func capture(_ wid: CGWindowID, _ bounds: [String:Any], _ images: [[String:Any]], _ directory: URL) -> [String:Any] {
    let filename="window-\(wid).png", target=directory.appendingPathComponent(filename)
    let captureArguments=windowCaptureArguments(wid,target)
    var imageDetails=[String:Any]()
    func unavailable(_ reason: String) -> [String:Any] {
        var result:[String:Any]=["window_id":wid,"error":reason,"capture_arguments":captureArguments,
            "preview_crops":[[String:Any]](),"preview_crop_diagnostics":[["outcome":"skipped","reason":reason]]]
        for (key,value) in imageDetails {result[key]=value}
        return result
    }
    if FileManager.default.fileExists(atPath:target.path) {return unavailable("screenshot_output_exists")}
    let task=Process();task.executableURL=URL(fileURLWithPath:"/usr/sbin/screencapture");task.arguments=captureArguments
    task.standardOutput=FileHandle.nullDevice;task.standardError=FileHandle.nullDevice
    do {
        try task.run();let deadline=Date().addingTimeInterval(5)
        while task.isRunning && Date()<deadline {Thread.sleep(forTimeInterval:0.05)}
        if task.isRunning {task.terminate();return unavailable("screencapture_timeout")}
    } catch {return unavailable("screencapture_launch_failed")}
    guard task.terminationStatus==0, let image=NSImage(contentsOf:target), let cg=image.cgImage(forProposedRect:nil,context:nil,hints:nil) else {return unavailable("screenshot_unavailable")}
    let observed=observeCapturedWindowText(bounds,cg)
    imageDetails=["png":filename,"width":cg.width,"height":cg.height,"capture_mode":"window_without_shadow",
        "capture_policy":["exclude_attached_windows_requested":true,"exclude_shadow_requested":true],
        "capture_image_geometry":observed.geometry.diagnostic]
    if let failure=observed.failure {return unavailable(failure)}
    guard let lines=observed.lines else {return unavailable("ocr_unavailable")}
    var crops=[[String:Any]](), diagnostics=[[String:Any]]()
    if images.isEmpty {diagnostics.append(["outcome":"skipped","reason":"ax_preview_image_missing"])}
    for (i,node) in images.enumerated() {
        var diagnostic:[String:Any]=["ax_image_index":i,"description":node["description"] ?? node["title"] ?? "",
            "frame_storage_type":node["frame"].map {String(describing:type(of:$0))} ?? "missing"]
        do {
            let prepared=try preparePreviewCrop(cg,bounds,node)
            let name="preview-\(wid)-\(i).png"
            do {try prepared.png.write(to:directory.appendingPathComponent(name),options:.atomic)} catch {
                diagnostic["outcome"]="skipped";diagnostic["reason"]="crop_png_write_failed";diagnostics.append(diagnostic);continue
            }
            var crop=prepared.metadata;crop["png"]=name;crops.append(crop)
            diagnostic["outcome"]="saved";diagnostic["reason"]=prepared.reason
            diagnostic["source_pixel_rect"]=crop["source_pixel_rect"]
        } catch let error as PreviewCropFailure {
            diagnostic["outcome"]="skipped";diagnostic["reason"]=error.rawValue
        } catch {
            diagnostic["outcome"]="skipped";diagnostic["reason"]="unexpected_preview_crop_error"
        }
        diagnostics.append(diagnostic)
    }
    var result=imageDetails;result["window_id"]=wid;result["capture_arguments"]=captureArguments
    result["ocr"]=lines;result["preview_crops"]=crops;result["preview_crop_diagnostics"]=diagnostics
    return result
}
#if !RUNTIME_OBSERVER_NATIVE_TEST
let all=(CGWindowListCopyWindowInfo([.optionAll,.excludeDesktopElements],kCGNullWindowID) as? [[String:Any]]) ?? []
let processes=ids.map{process($0)}
var windows=[[String:Any]]()
for row in all {
    guard let p=row[kCGWindowOwnerPID as String] as? Int32,ids.contains(p), let id=row[kCGWindowNumber as String] as? UInt32 else{continue}
    var r:[String:Any]=["pid":Int(p),"window_id":id,"on_screen":row[kCGWindowIsOnscreen as String] ?? false,"layer":row[kCGWindowLayer as String] ?? -1,"bounds":row[kCGWindowBounds as String] ?? [:]]
    if windowsWanted.contains(id) {
        let images=(processes.first(where:{$0["pid"] as? Int == Int(p)})?["ax"] as? [[String:Any]] ?? []).filter{node in
            node["role"] as? String == "AXImage" && ((node["description"] as? String ?? node["title"] as? String ?? "").contains("目标窗口当前画面"))
        }
        r["capture"]=capture(id,row[kCGWindowBounds as String] as? [String:Any] ?? [:],images,directory)
    }
    windows.append(r)
}
var bootTime=timeval();var length=MemoryLayout<timeval>.size
let bootStatus=sysctlbyname("kern.boottime",&bootTime,&length,nil,0)
let result:[String:Any]=["schema_version":1,"kind":"independent_macos_runtime_observation","observed_at_ms":UInt64(Date().timeIntervalSince1970*1000),"boot_status":bootStatus,"boot_sec":bootTime.tv_sec,"boot_usec":bootTime.tv_usec,"accessibility_trusted":AXIsProcessTrusted(),"screen_capture_preflight":CGPreflightScreenCaptureAccess(),"processes":processes,"windows":windows]
let data=try JSONSerialization.data(withJSONObject:result,options:[.sortedKeys,.prettyPrinted])
try data.write(to:URL(fileURLWithPath:output),options:.atomic)
#endif
