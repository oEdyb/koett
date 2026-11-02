import Foundation

struct HistoryCursor: Sendable {
    let createdAtMS: Int64
    let id: String
}

struct HistorySummary: Identifiable, Sendable {
    let id: String
    let createdAtMS: Int64
    let model: String
    let preview: String

    var date: Date { Date(timeIntervalSince1970: Double(createdAtMS) / 1000) }
    var cursor: HistoryCursor { HistoryCursor(createdAtMS: createdAtMS, id: id) }
}
