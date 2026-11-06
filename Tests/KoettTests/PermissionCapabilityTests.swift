import AVFoundation
import Foundation
@testable import Koett
import XCTest

@MainActor
final class PermissionCapabilityTests: XCTestCase {
    func testDeniedPermissionsNeverRunAProbe() async {
        var probes: [PermissionKind] = []
        let center = PermissionCenter(capabilityProbe: { kind in
            probes.append(kind)
            return true
        }, granted: { $0 == .accessibility })
        await center.checkCapabilities()
        XCTAssertEqual(probes, [.accessibility])
        XCTAssertEqual(center.row(.accessibility).capability, true)
        XCTAssertNil(center.row(.microphone).capability)
        XCTAssertNil(center.row(.screenRecording).capability)
    }

    func testGrantedFlagCannotHideFailedCapabilityAndRetryCanRecover() async {
        var works = false
        let center = PermissionCenter(capabilityProbe: { _ in works }, granted: { _ in true })
        XCTAssertNil(center.row(.screenRecording).capability)
        await center.checkCapabilities([.screenRecording])
        XCTAssertEqual(center.row(.screenRecording).capability, false)
        works = true
        await center.checkCapabilities([.screenRecording])
        XCTAssertEqual(center.row(.screenRecording).capability, true)
    }

    func testGrantRevokedDuringProbeCannotAppearReady() async {
        var granted = true
        let center = PermissionCenter(capabilityProbe: { _ in
            granted = false
            return true
        }, granted: { _ in granted })
        await center.checkCapabilities([.accessibility])
        XCTAssertFalse(center.row(.accessibility).allowed)
        XCTAssertNil(center.row(.accessibility).capability)
    }

    func testRealBufferReceiptIsRequiredAndInputAlwaysStops() async throws {
        let available = ProbeInput(providesBuffer: true)
        let received = try await PermissionCapabilities.microphone(input: available)
        XCTAssertTrue(received)
        XCTAssertEqual(available.stops, 1)
        let noBuffers = ProbeInput(providesBuffer: false)
        let timedOut = try await PermissionCapabilities.microphone(input: noBuffers, timeout: .milliseconds(40))
        XCTAssertFalse(timedOut, "Permission alone is insufficient when no buffers arrive")
        XCTAssertEqual(noBuffers.stops, 1)
    }

    func testPartiallyOpenedInputStopsAfterStartFailure() async {
        let input = ProbeInput(providesBuffer: false, failsStart: true)
        do {
            _ = try await PermissionCapabilities.microphone(input: input)
            XCTFail("Start failure must be propagated")
        } catch {}
        XCTAssertEqual(input.stops, 1)
    }
}

@MainActor
private final class ProbeInput: MicrophoneCapturing {
    let format = AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1)!
    var error: Error? { nil }
    let providesBuffer: Bool
    let failsStart: Bool
    private(set) var stops = 0

    init(providesBuffer: Bool, failsStart: Bool = false) {
        self.providesBuffer = providesBuffer
        self.failsStart = failsStart
    }

    func start(onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {
        if failsStart { throw NSError(domain: "Fixture", code: 1) }
        if providesBuffer {
            let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 16)!
            buffer.frameLength = 16
            onAudio(buffer)
        }
    }

    func stop() { stops += 1 }
}
