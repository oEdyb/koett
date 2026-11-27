import Foundation

struct MeetingStopKey: Codable, Equatable, Sendable {
    let id: UUID
    let token: UInt64
}

struct MeetingState: Equatable, Sendable {
    enum Phase: String, Codable, Sendable { case idle, starting, recording, confirmStop, saving, processing }

    enum Event {
        case start(UUID), opened(UInt64), startFailed(UInt64)
        case stop(UUID), resume(MeetingStopKey)
        case closing(MeetingStopKey, MeetingCloseGoal, UInt64)
        case interrupted(UInt64, UInt64)
        case sealed(UInt64, process: Bool)
        case recover(UUID), processed(UInt64)
    }

    enum Result: Equatable { case changed, same, busy, wrongState, staleToken }

    private(set) var phase = Phase.idle
    private(set) var id: UUID?
    private(set) var attempt: UInt64 = 0
    private(set) var confirmation: MeetingStopKey?
    private(set) var cutoff: UInt64?
    private var generation: UInt64 = 0

    var reservesForeground: Bool { phase != .idle && phase != .processing }

    mutating func transition(_ event: Event) -> Result {
        switch event {
        case let .start(id):
            if self.id == id { return .same }
            guard phase == .idle else { return .busy }
            self.id = id
            attempt += 1
            phase = .starting
        case let .opened(attempt) where phase == .starting && self.attempt == attempt:
            phase = .recording
        case let .startFailed(attempt) where phase == .starting && self.attempt == attempt:
            clear()
        case let .stop(id):
            guard self.id == id else { return .wrongState }
            if phase == .confirmStop { return .same }
            guard phase == .recording else { return .wrongState }
            generation += 1
            confirmation = .init(id: id, token: generation)
            phase = .confirmStop
        case let .resume(key):
            guard phase == .confirmStop, key == confirmation else { return .staleToken }
            confirmation = nil
            phase = .recording
        case let .closing(key, _, cutoff):
            guard phase == .confirmStop, key == confirmation else { return .staleToken }
            beginSaving(cutoff)
        case let .interrupted(attempt, cutoff) where self.attempt == attempt && reservesForeground && phase != .saving:
            beginSaving(cutoff)
        case let .sealed(attempt, process) where phase == .saving && self.attempt == attempt:
            if process {
                self.attempt += 1
                phase = .processing
            } else { clear() }
        case let .recover(id):
            guard phase == .idle else { return .busy }
            self.id = id
            attempt += 1
            phase = .processing
        case let .processed(attempt) where phase == .processing && self.attempt == attempt:
            clear()
        default:
            return .same
        }
        return .changed
    }

    private mutating func beginSaving(_ cutoff: UInt64) {
        confirmation = nil
        self.cutoff = cutoff
        phase = .saving
    }

    private mutating func clear() {
        phase = .idle
        id = nil
        confirmation = nil
        cutoff = nil
    }
}
