// System-output capture only. stdout is mono s16le/16kHz; stderr is JSON status.
import AVFoundation
import CoreAudio
import Darwin
import Foundation

struct CaptureError: Error, CustomStringConvertible {
    let description: String
}
func check(_ status: OSStatus, _ action: String) throws {
    if status != noErr { throw CaptureError(description: "\(action): OSStatus \(status)") }
}
func event(_ kind: String, _ fields: [String: Any] = [:]) {
    var value = fields
    value["kind"] = kind
    if let data = try? JSONSerialization.data(withJSONObject: value) {
        FileHandle.standardError.write(data + Data([10]))
    }
}
func address(_ selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal,
                               mElement: kAudioObjectPropertyElementMain)
}
@available(macOS 14.2, *)
func processObject(_ pid: pid_t) throws -> AudioObjectID? {
    var property = address(kAudioHardwarePropertyTranslatePIDToProcessObject)
    var pid = pid
    var object = AudioObjectID(kAudioObjectUnknown)
    var size = UInt32(MemoryLayout.size(ofValue: object))
    try check(AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &property,
        UInt32(MemoryLayout.size(ofValue: pid)), &pid, &size, &object), "resolve excluded process")
    return object == kAudioObjectUnknown ? nil : object
}

@available(macOS 14.2, *)
func capture(excluding pid: pid_t) throws {
    let description = CATapDescription(stereoGlobalTapButExcludeProcesses: [try processObject(pid)].compactMap { $0 })
    description.name = "Atogaki system audio"
    description.isPrivate = true
    description.muteBehavior = .unmuted
    var tap = AudioObjectID(kAudioObjectUnknown)
    try check(AudioHardwareCreateProcessTap(description, &tap), "create process tap")
    defer { AudioHardwareDestroyProcessTap(tap) }

    var formatAddress = address(kAudioTapPropertyFormat)
    var asbd = AudioStreamBasicDescription()
    var size = UInt32(MemoryLayout.size(ofValue: asbd))
    try check(AudioObjectGetPropertyData(tap, &formatAddress, 0, nil, &size, &asbd), "read tap format")
    guard asbd.mBytesPerFrame > 0 else { throw CaptureError(description: "invalid tap sample format") }
    guard let sourceFormat = AVAudioFormat(streamDescription: &asbd),
          let outputFormat = AVAudioFormat(commonFormat: .pcmFormatInt16,
                                          sampleRate: 16000, channels: 1, interleaved: true),
          let converter = AVAudioConverter(from: sourceFormat, to: outputFormat) else {
        throw CaptureError(description: "unsupported system audio format")
    }
    converter.downmix = true
    // A tap-only aggregate has no physical microphone input streams.
    let device: [String: Any] = [
        kAudioAggregateDeviceNameKey: "Atogaki capture",
        kAudioAggregateDeviceUIDKey: UUID().uuidString,
        kAudioAggregateDeviceIsPrivateKey: true,
        kAudioAggregateDeviceTapAutoStartKey: true,
        kAudioAggregateDeviceTapListKey: [[kAudioSubTapUIDKey: description.uuid.uuidString,
                                         kAudioSubTapDriftCompensationKey: true]],
    ]
    var aggregate = AudioDeviceID(kAudioObjectUnknown)
    try check(AudioHardwareCreateAggregateDevice(device as CFDictionary, &aggregate), "create aggregate")
    defer { AudioHardwareDestroyAggregateDevice(aggregate) }

    let stopped = DispatchSemaphore(value: 0)
    let failureLock = NSLock()
    var failed = false
    func fail(_ message: String) {
        failureLock.lock(); failed = true; failureLock.unlock()
        event("capture_error", ["error": message]); stopped.signal()
    }
    let queue = DispatchQueue(label: "atogaki.capture")
    // Never block a HAL callback behind slow inference or a full pipe. Report
    // overload and stop the session rather than silently deleting audio.
    _ = fcntl(STDOUT_FILENO, F_SETFL, fcntl(STDOUT_FILENO, F_GETFL) | O_NONBLOCK)
    signal(SIGPIPE, SIG_IGN)
    var ioProc: AudioDeviceIOProcID?
    try check(AudioDeviceCreateIOProcIDWithBlock(&ioProc, aggregate, queue) {
        _, input, _, _, _ in
        let buffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: input))
        guard let first = buffers.first, first.mDataByteSize > 0 else { return }
        let frames = AVAudioFrameCount(first.mDataByteSize / asbd.mBytesPerFrame)
        guard let copied = AVAudioPCMBuffer(pcmFormat: sourceFormat, frameCapacity: frames) else { fail("cannot allocate capture buffer"); return }
        copied.frameLength = frames
        let destination = UnsafeMutableAudioBufferListPointer(copied.mutableAudioBufferList)
        guard destination.count == buffers.count else {
            fail("tap buffer layout changed"); return
        }
        for index in buffers.indices {
            guard buffers[index].mDataByteSize <= destination[index].mDataByteSize,
                  let source = buffers[index].mData, let target = destination[index].mData else {
                fail("invalid tap buffer size"); return
            }
            memcpy(target, source, Int(buffers[index].mDataByteSize))
        }
        let capacity = AVAudioFrameCount(ceil(Double(frames) * 16000 / sourceFormat.sampleRate) + 32)
        guard let converted = AVAudioPCMBuffer(pcmFormat: outputFormat, frameCapacity: capacity) else { fail("cannot allocate resampling buffer"); return }
        var supplied = false
        var conversionError: NSError?
        let status = converter.convert(to: converted, error: &conversionError) { _, state in
            if supplied { state.pointee = .noDataNow; return nil }
            supplied = true; state.pointee = .haveData; return copied
        }
        guard status != .error, conversionError == nil,
              let samples = converted.int16ChannelData?[0] else {
            fail("audio conversion failed"); return
        }
        var count = Int(converted.frameLength) * 2
        var pointer = UnsafeRawPointer(samples)
        while count > 0 {
            let written = Darwin.write(STDOUT_FILENO, pointer, count)
            if written < 0 {
                if errno == EINTR { continue }
                fail("audio pipe closed or overloaded: errno \(errno)"); return
            }
            count -= written; pointer = pointer.advanced(by: written)
        }
    }, "create capture callback")
    guard let ioProc else { throw CaptureError(description: "missing capture callback") }
    defer { AudioDeviceDestroyIOProcID(aggregate, ioProc) }
    try check(AudioDeviceStart(aggregate, ioProc), "start capture; check System Audio Recording permission")
    defer { AudioDeviceStop(aggregate, ioProc) }
    event("capture_ready", ["sample_rate": 16000, "channels": 1, "sample_format": "s16le"])
    // Closing stdin or sending any byte requests graceful cleanup of the tap.
    DispatchQueue.global().async { _ = FileHandle.standardInput.readData(ofLength: 1); stopped.signal() }
    signal(SIGTERM, SIG_IGN); signal(SIGINT, SIG_IGN)
    let term = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .global())
    let interrupt = DispatchSource.makeSignalSource(signal: SIGINT, queue: .global())
    term.setEventHandler { stopped.signal() }; interrupt.setEventHandler { stopped.signal() }
    term.resume(); interrupt.resume()
    stopped.wait()
    term.cancel(); interrupt.cancel()
    failureLock.lock(); let didFail = failed; failureLock.unlock()
    if didFail { throw CaptureError(description: "system audio capture failed") }
    event("capture_stopped")
}

do {
    if CommandLine.arguments.contains("--check") {
        event("capture_supported", ["minimum_macos": "14.2", "sample_rate": 16000])
    } else if #available(macOS 14.2, *) {
        let arguments = CommandLine.arguments
        guard arguments.count == 3, arguments[1] == "--exclude-pid", let pid = pid_t(arguments[2]), pid > 0 else {
            throw CaptureError(description: "usage: atogaki-audio-tap --exclude-pid PID (stdin EOF stops)")
        }
        try capture(excluding: pid)
    } else {
        throw CaptureError(description: "system audio capture requires macOS 14.2+")
    }
} catch {
    event("capture_error", ["error": String(describing: error)])
    exit(1)
}
