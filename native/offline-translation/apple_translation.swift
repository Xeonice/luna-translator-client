// GPL-3.0-or-later. System-only, headless, explicitly traditional translation.
// Built with macOS SDK >=26.4. Missing languages require a separate interactive
// system host; this helper never requests a download or uses Apple Intelligence.
import Foundation
import Translation
import CryptoKit

let wireProtocol = "translation-worker-v1"
let maxFrame = 1_048_576
let maxOutput = 262_144
struct WorkerFailure: Error { let code: String; let stage: String }
func readFrame() throws -> [String: Any] {
    var data = Data()
    while data.count <= maxFrame {
        guard let byte = try FileHandle.standardInput.read(upToCount: 1), !byte.isEmpty else { throw WorkerFailure(code: "protocol_violation", stage: "eof") }
        data.append(byte)
        if byte.first == 10 { break }
    }
    guard data.count <= maxFrame, data.last == 10,
          let value = try JSONSerialization.jsonObject(with: data) as? [String: Any],
          value["protocol"] as? String == wireProtocol else { throw WorkerFailure(code: "protocol_violation", stage: "decode") }
    return value
}
func emit(_ value: [String: Any]) throws {
    var bytes = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .withoutEscapingSlashes])
    bytes.append(10)
    guard bytes.count <= maxFrame else { throw WorkerFailure(code: "output_too_large", stage: "frame_budget") }
    try FileHandle.standardOutput.write(contentsOf: bytes)
}
func equalJSON(_ lhs: Any?, _ rhs: Any?) -> Bool {
    guard let lhs, let rhs, let a = try? JSONSerialization.data(withJSONObject: lhs, options: [.sortedKeys]), let b = try? JSONSerialization.data(withJSONObject: rhs, options: [.sortedKeys]) else { return false }
    return a == b
}
func digest(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
func osBuild() -> String {
    var size = 0
    guard sysctlbyname("kern.osversion", nil, &size, nil, 0) == 0, size > 0, size < 1024 else { return "unknown" }
    var bytes = [CChar](repeating: 0, count: size)
    guard sysctlbyname("kern.osversion", &bytes, &size, nil, 0) == 0 else { return "unknown" }
    return String(cString: bytes)
}
@available(macOS 26.4, *)
func run(_ hello: [String: Any], instance: String, requestID: String) async throws {
    guard hello["kind"] as? String == "hello", hello["engineId"] as? String == "apple_translation", hello["contractRevision"] as? Int == 1,
          let resource = hello["resource"] as? [String: Any], let files = resource["files"] as? [Any], files.isEmpty,
          let route = resource["route"] as? [[String: Any]], route.count == 1,
          let pair = route.first?["pair"] as? [String: String], let source = pair["source"], let target = pair["target"], !source.isEmpty, !target.isEmpty,
          let resourceIdentity = resource["identity"] as? [String: Any], let resourceDigest = resource["digest"] as? String,
          let version = resource["engineVersion"] as? String else { throw WorkerFailure(code: "protocol_violation", stage: "hello") }
    let build = osBuild()
    guard build != "unknown", version == build else { throw WorkerFailure(code: "resource_changed", stage: "system_build") }
    let data = try JSONSerialization.data(withJSONObject: [files, route, version, resourceIdentity], options: [.sortedKeys, .withoutEscapingSlashes])
    guard digest(data) == resourceDigest else { throw WorkerFailure(code: "resource_changed", stage: "manifest_digest") }
    let sourceLanguage = Locale.Language(identifier: source)
    let targetLanguage = Locale.Language(identifier: target)
    let availability = LanguageAvailability(preferredStrategy: .lowLatency)
    let status = await availability.status(from: sourceLanguage, to: targetLanguage)
    let availabilityName: String
    let reason: Any
    switch status {
    case .installed: availabilityName = "installed"; reason = NSNull()
    case .supported: availabilityName = "supportedNotInstalled"; reason = "missing_resource"
    case .unsupported: availabilityName = "unsupported"; reason = "unsupported_pair"
    @unknown default: availabilityName = "unknown"; reason = "not_ready"
    }
    let observation: [String: Any] = ["engineId": "apple_translation", "engineContractRevision": 1, "runtimeVersion": "macOS-\(build)/Translation/lowLatency/26.4", "platform": "macos", "architecture": "\(ProcessInfo.processInfo.machineArchitecture)", "availability": availabilityName, "physicalCancelSupported": false, "resourceVersion": NSNull(), "reason": reason]
    try emit(["kind": "ready", "protocol": wireProtocol, "workerInstanceId": instance, "requestId": requestID, "resourceDigest": resourceDigest, "observation": observation])
    if hello["operation"] as? String == "observe" { return }
    guard hello["operation"] as? String == "translate" else { throw WorkerFailure(code: "requires_interactive_system_host", stage: "prepare_requires_host") }
    guard availabilityName == "installed" else { throw WorkerFailure(code: availabilityName == "unsupported" ? "unsupported_pair" : "missing_resource", stage: "language_package") }
    let execute = try readFrame()
    if execute["kind"] as? String == "cancel" { throw WorkerFailure(code: "cancelled", stage: "before_execute") }
    guard execute["kind"] as? String == "execute", execute["workerInstanceId"] as? String == instance,
          let identity = hello["identity"] as? [String: Any], equalJSON(execute["identity"], identity),
          let text = execute["sourceText"] as? String, !text.isEmpty, text.utf8.count <= 65_536, !text.contains("\0"),
          digest(Data(text.utf8)) == identity["sourceHash"] as? String else { throw WorkerFailure(code: "protocol_violation", stage: "execute_identity") }
    let session = TranslationSession(installedSource: sourceLanguage, target: targetLanguage, preferredStrategy: .lowLatency)
    // One operation per session/process. Parent sends Cancel then reaps this
    // owned child after grace; no unrelated session can share a cancellation.
    defer { session.cancel() }
    let response = try await session.translate(text)
    guard response.targetText.utf8.count <= maxOutput, !response.targetText.isEmpty, !response.targetText.contains("\0") else { throw WorkerFailure(code: "output_too_large", stage: "output") }
    guard osBuild() == build else { throw WorkerFailure(code: "resource_changed", stage: "post_system_build") }
    try emit(["kind": "final", "protocol": wireProtocol, "workerInstanceId": instance, "identity": identity, "resourceIdentity": resourceIdentity, "resourceDigest": resourceDigest, "route": route, "text": response.targetText])
}
extension ProcessInfo {
    var machineArchitecture: String {
        #if arch(arm64)
        return "arm64"
        #elseif arch(x86_64)
        return "x86_64"
        #else
        return "unknown"
        #endif
    }
}
@main struct AppleTranslationWorker {
    static func main() async {
        var instance = "", requestID = ""
        do {
            let hello = try readFrame()
            instance = hello["workerInstanceId"] as? String ?? ""
            requestID = hello["requestId"] as? String ?? ""
            guard !instance.isEmpty, !requestID.isEmpty else { throw WorkerFailure(code: "protocol_violation", stage: "identity") }
            guard #available(macOS 26.4, *) else { throw WorkerFailure(code: "unsupported_platform", stage: "macos_26_4_required") }
            try await run(hello, instance: instance, requestID: requestID)
        } catch let failure as WorkerFailure {
            try? emit(["kind": "error", "protocol": wireProtocol, "workerInstanceId": instance, "requestId": requestID, "error": ["code": failure.code, "stage": failure.stage]])
        } catch {
            // Never send NSError descriptions: they can include text or paths.
            try? emit(["kind": "error", "protocol": wireProtocol, "workerInstanceId": instance, "requestId": requestID, "error": ["code": "engine_failed", "stage": "system_translation"]])
        }
    }
}
