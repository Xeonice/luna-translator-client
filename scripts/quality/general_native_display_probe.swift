// Draft Native-only read-only inventory. No AppKit, AX, GUI or display mutation.
import Foundation
import CoreGraphics
import ColorSync
import CryptoKit

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8)); exit(2)
}
func stableDisplayInventoryCounts(_ initial: UInt32, _ returned: UInt32, _ verified: UInt32) -> Bool {
    return initial > 0 && initial <= 32 && returned == initial && verified == initial
}
#if !GENERAL_NATIVE_DISPLAY_PROBE_TEST
let arguments = CommandLine.arguments
guard arguments.count == 5, arguments[1] == "--nonce", arguments[3] == "--phase",
      arguments[2].range(of: "^[0-9a-f]{32}$", options: .regularExpression) != nil,
      ["pre_launch", "before_capture", "after_capture"].contains(arguments[4]) else {
    fail("exact --nonce 32hex --phase pre_launch|before_capture|after_capture required")
}
let nonce = arguments[2]
let phase = arguments[4]
var count: UInt32 = 0
guard CGGetActiveDisplayList(0, nil, &count) == .success, count > 0, count <= 32 else {
    fail("bounded active display inventory unavailable")
}
var identifiers = [CGDirectDisplayID](repeating: 0, count: Int(count))
let initialCount = count
guard CGGetActiveDisplayList(count, &identifiers, &count) == .success else {
    fail("active display inventory changed during lookup")
}
var verifiedCount: UInt32 = 0
guard CGGetActiveDisplayList(0, nil, &verifiedCount) == .success,
      stableDisplayInventoryCounts(initialCount, count, verifiedCount) else {
    fail("active display count changed; truncated inventory refused")
}
let main = CGMainDisplayID()
var displays = [[String: Any]]()
var totalProfileBytes = 0
for identifier in identifiers.prefix(Int(count)).sorted() {
    let color = CGDisplayCopyColorSpace(identifier)
    guard let mode = CGDisplayCopyDisplayMode(identifier),
          let profile = color.copyICCData() as Data? else {
        fail("display mode or original ICC profile unavailable")
    }
    guard !profile.isEmpty, profile.count <= 4 * 1024 * 1024 else {
        fail("display ICC profile exceeds bounded raw evidence size")
    }
    totalProfileBytes += profile.count
    guard totalProfileBytes <= 16 * 1024 * 1024 else {
        fail("display ICC inventory exceeds bounded aggregate evidence size")
    }
    let bounds = CGDisplayBounds(identifier)
    let uuid = CGDisplayCreateUUIDFromDisplayID(identifier).takeRetainedValue()
    guard let uuidText = CFUUIDCreateString(nil, uuid) as String? else {
        fail("display UUID unavailable")
    }
    displays.append([
        "display_id": identifier, "uuid": uuidText,
        "is_main": identifier == main,
        "is_active": CGDisplayIsActive(identifier) != 0,
        "is_online": CGDisplayIsOnline(identifier) != 0,
        "is_builtin": CGDisplayIsBuiltin(identifier) != 0,
        "is_asleep": CGDisplayIsAsleep(identifier) != 0,
        "vendor": CGDisplayVendorNumber(identifier),
        "model": CGDisplayModelNumber(identifier),
        "serial": CGDisplaySerialNumber(identifier),
        "rotation_degrees": CGDisplayRotation(identifier),
        "bounds": ["x": bounds.origin.x, "y": bounds.origin.y,
                   "width": bounds.width, "height": bounds.height],
        "mode": ["width": mode.width, "height": mode.height,
                 "pixel_width": mode.pixelWidth, "pixel_height": mode.pixelHeight,
                 "refresh_rate": mode.refreshRate, "io_flags": mode.ioFlags],
        "color_space": ["name": color.name as String? ?? "",
                        "model": color.model.rawValue,
                        "components": color.numberOfComponents,
                        "icc": ["bytes": profile.count,
                                "sha256": SHA256.hash(data: profile).map { String(format: "%02x", $0) }.joined(),
                                "base64": profile.base64EncodedString()]]
    ])
}
guard displays.filter({ $0["is_main"] as? Bool == true }).count == 1 else {
    fail("exactly one active primary display required")
}
guard CGGetActiveDisplayList(0, nil, &verifiedCount) == .success,
      stableDisplayInventoryCounts(initialCount, count, verifiedCount), CGMainDisplayID() == main else {
    fail("active or primary display changed during original inventory")
}
let result: [String: Any] = [
    "schema_version": 1, "kind": "general-native-display-inventory/v1",
    "nonce": nonce, "phase": phase,
    "pid": ProcessInfo.processInfo.processIdentifier, "ppid": getppid(),
    "observed_at_us": Int64(Date().timeIntervalSince1970 * 1_000_000),
    "main_display_id": main, "active_display_count": displays.count,
    "displays": displays,
    "capabilities": ["read_only": true, "gui_constructed": false,
                     "screen_capture_invoked": false, "system_mutation": false,
                     "permissions_requested": false], "errors": []
]
let output = try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
FileHandle.standardOutput.write(output);FileHandle.standardOutput.write(Data("\n".utf8))
#endif
