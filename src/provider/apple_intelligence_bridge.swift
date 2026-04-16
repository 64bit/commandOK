// Swift bridge between Rust and Apple's FoundationModels framework.
//
// Exposes a small C ABI so the Rust `apple_intelligence` provider can drive an
// on-device LanguageModelSession without needing Swift interop on the Rust side.

import Foundation
import FoundationModels

/// Returns 0 if the on-device model is ready to use, nonzero otherwise.
@_cdecl("apple_intelligence_available")
public func apple_intelligence_available() -> Int32 {
    let model = SystemLanguageModel.default
    switch model.availability {
    case .available:
        return 0
    case .unavailable:
        return 1
    @unknown default:
        return 2
    }
}

/// Writes a short, human-readable reason for unavailability into `buf` (a
/// caller-owned buffer of `len` bytes). Returns the number of bytes written
/// (excluding the trailing NUL), or 0 if the model is actually available.
@_cdecl("apple_intelligence_unavailable_reason")
public func apple_intelligence_unavailable_reason(
    _ buf: UnsafeMutablePointer<CChar>,
    _ len: Int
) -> Int {
    let model = SystemLanguageModel.default
    let message: String
    switch model.availability {
    case .available:
        return 0
    case .unavailable(.appleIntelligenceNotEnabled):
        message = "Apple Intelligence is not enabled in System Settings."
    case .unavailable(.modelNotReady):
        message = "The on-device model is still downloading. Try again later."
    case .unavailable(.deviceNotEligible):
        message = "This device is not eligible for Apple Intelligence."
    case .unavailable(let other):
        message = "Apple Intelligence unavailable: \(other)"
    @unknown default:
        message = "Apple Intelligence is unavailable for an unknown reason."
    }
    return copyToBuffer(message, buf, len)
}

/// Streams a response synchronously from the caller's perspective.
///
/// `instructions` and `prompt` are NUL-terminated UTF-8 strings owned by the
/// caller. `user_data` is an opaque pointer forwarded to each callback.
///
/// `on_delta` is invoked once per streamed chunk with a NUL-terminated UTF-8
/// pointer that is only valid for the duration of the call (the Rust side must
/// copy it). `on_done` is invoked exactly once: `status == 0` on success, with
/// `error` NULL; nonzero status with `error` pointing at a UTF-8 message.
@_cdecl("apple_intelligence_stream")
public func apple_intelligence_stream(
    _ instructions: UnsafePointer<CChar>,
    _ prompt: UnsafePointer<CChar>,
    _ user_data: UnsafeMutableRawPointer?,
    _ on_delta: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>) -> Void,
    _ on_done: @convention(c) (UnsafeMutableRawPointer?, Int32, UnsafePointer<CChar>?) -> Void
) {
    let instructionsStr = String(cString: instructions)
    let promptStr = String(cString: prompt)

    let semaphore = DispatchSemaphore(value: 0)

    Task {
        defer { semaphore.signal() }
        do {
            let session = LanguageModelSession(instructions: instructionsStr)
            let stream = session.streamResponse(to: promptStr)

            // The framework yields cumulative snapshots, so compute deltas by
            // tracking the previously-seen prefix length. For a String stream
            // `snapshot.content` is itself a String.
            var emitted = 0
            for try await snapshot in stream {
                let cumulative = snapshot.content
                if cumulative.count <= emitted { continue }
                let startIndex = cumulative.index(cumulative.startIndex, offsetBy: emitted)
                let delta = String(cumulative[startIndex...])
                emitted = cumulative.count
                delta.withCString { cstr in
                    on_delta(user_data, cstr)
                }
            }
            on_done(user_data, 0, nil)
        } catch {
            let message = "\(error)"
            message.withCString { cstr in
                on_done(user_data, 1, cstr)
            }
        }
    }

    semaphore.wait()
}

// MARK: - Helpers

private func copyToBuffer(_ s: String, _ buf: UnsafeMutablePointer<CChar>, _ len: Int) -> Int {
    let utf8 = Array(s.utf8)
    let writable = min(utf8.count, max(0, len - 1))
    for i in 0..<writable {
        buf[i] = CChar(bitPattern: utf8[i])
    }
    if len > 0 {
        buf[writable] = 0
    }
    return writable
}
