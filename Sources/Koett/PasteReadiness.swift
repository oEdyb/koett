import CoreGraphics
import Foundation

enum PasteBlockReason: String, Codable, Sendable {
    case accessibilityDenied, noTarget, focusChanged, ownApp, windowChanged, targetNotReady
    case modifiersHeld, eventCreationFailed, clipboardChanged, clipboardUnavailable, cancelled

    var retryable: Bool { self == .modifiersHeld || self == .targetNotReady }
    var message: String {
        switch self {
        case .accessibilityDenied: "Accessibility access missing"
        case .noTarget: "No paste target"
        case .focusChanged: "Focus changed"
        case .ownApp: "Koett has focus"
        case .windowChanged: "Target window changed"
        case .targetNotReady: "Target window not ready"
        case .modifiersHeld: "Modifier keys still held"
        case .eventCreationFailed: "Paste event could not be created"
        case .clipboardChanged: "Newer clipboard copy preserved"
        case .clipboardUnavailable: "Clipboard could not be written"
        case .cancelled: "Dictation cancelled"
        }
    }
}

enum PasteStage: String, Codable, Sendable { case dictation, sketchImage, sketchText, manual }

struct PasteSnapshot: Sendable {
    var access = true
    var targetFailure: PasteBlockReason?
    var physicalFlags: UInt64 = 0
    var combinedFlags: UInt64 = 0
    var expectedPID: Int32?
    var foregroundPID: Int32?
    var expectedApp: String?
    var foregroundApp: String?

    var reason: PasteBlockReason? {
        guard access else { return .accessibilityDenied }
        if let targetFailure { return targetFailure }
        let modifiers: CGEventFlags = [.maskCommand, .maskControl, .maskAlternate, .maskShift]
        return CGEventFlags(rawValue: physicalFlags).intersection(modifiers).isEmpty ? nil : .modifiersHeld
    }
}

struct PasteDiagnostic: Codable, Sendable {
    let at: Date
    let stage: PasteStage
    let outcome: String
    let reason: PasteBlockReason?
    let waitedFor: PasteBlockReason?
    let waitedMilliseconds: Int
    let physicalFlags: UInt64
    let combinedFlags: UInt64
    let expectedPID: Int32?
    let foregroundPID: Int32?
    let expectedApp: String?
    let foregroundApp: String?
}

@MainActor
final class PasteCoordinator {
    static let timeout: TimeInterval = 1.5
    private let snapshot: () -> PasteSnapshot
    private let postEvent: (CGEventFlags) -> Bool
    private let record: (PasteDiagnostic) -> Void
    private let clock: () -> TimeInterval
    private let pause: @MainActor () async throws -> Void
    private var stage = PasteStage.dictation
    private var waitedFor: PasteBlockReason?
    private var waitedMilliseconds = 0
    private(set) var lastFailure: PasteBlockReason?

    init(snapshot: @escaping () -> PasteSnapshot, postEvent: @escaping (CGEventFlags) -> Bool,
         record: @escaping (PasteDiagnostic) -> Void = { _ in },
         clock: @escaping () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
         pause: @escaping @MainActor () async throws -> Void = { try await Task.sleep(for: .milliseconds(20)) }) {
        self.snapshot = snapshot
        self.postEvent = postEvent
        self.record = record
        self.clock = clock
        self.pause = pause
    }

    func prepare(stage: PasteStage) async -> Bool {
        self.stage = stage
        lastFailure = nil
        waitedFor = nil
        waitedMilliseconds = 0
        let started = clock()
        var state = snapshot()
        while let reason = state.reason, reason.retryable, clock() - started < Self.timeout {
            waitedFor = waitedFor ?? reason
            do { try await pause() } catch { return reject(.cancelled, snapshot: state) }
            if Task.isCancelled { return reject(.cancelled, snapshot: state) }
            state = snapshot()
        }
        waitedMilliseconds = max(0, Int((clock() - started) * 1_000))
        if Task.isCancelled { return reject(.cancelled, snapshot: state) }
        if let reason = state.reason { return reject(reason, snapshot: state) }
        return true
    }

    func post(flags: CGEventFlags) -> Bool {
        let state = snapshot()
        if let reason = state.reason { return reject(reason, snapshot: state) }
        guard postEvent(flags) else { return reject(.eventCreationFailed, snapshot: state) }
        report("posted", reason: nil, snapshot: state)
        return true
    }

    func externalFailure(_ reason: PasteBlockReason) {
        _ = reject(reason, snapshot: snapshot())
    }

    private func reject(_ reason: PasteBlockReason, snapshot: PasteSnapshot) -> Bool {
        lastFailure = reason
        report("blocked", reason: reason, snapshot: snapshot)
        return false
    }

    private func report(_ outcome: String, reason: PasteBlockReason?, snapshot: PasteSnapshot) {
        record(PasteDiagnostic(at: Date(), stage: stage, outcome: outcome, reason: reason,
            waitedFor: waitedFor, waitedMilliseconds: waitedMilliseconds,
            physicalFlags: snapshot.physicalFlags, combinedFlags: snapshot.combinedFlags,
            expectedPID: snapshot.expectedPID, foregroundPID: snapshot.foregroundPID,
            expectedApp: snapshot.expectedApp, foregroundApp: snapshot.foregroundApp))
    }
}
