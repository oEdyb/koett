import Foundation

struct TranscriptRecord: Codable, Sendable {
    let id: String
    let text: String
    let model: String
    let rawText: String?
    let date: Date

    init(
        id: String = UUID().uuidString,
        text: String,
        model: String,
        rawText: String? = nil,
        date: Date = Date()
    ) {
        self.id = id
        self.text = text
        self.model = model
        self.rawText = rawText
        self.date = date
    }

    var hasText: Bool {
        !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            || !(rawText ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var marker: String {
        "<!-- koett-transcript:\(Data(id.utf8).base64EncodedString()) -->"
    }

    var markdown: String {
        let timestamp = ISO8601DateFormatter().string(from: date)
        let transcript: String
        if let rawText {
            transcript = """

            Raw transcript:

            \(rawText)

            Cleaned transcript:

            \(text.isEmpty ? "(empty)" : text)
            """
        } else {
            transcript = "\n\n\(text)"
        }
        return """

        \(marker)
        ## \(timestamp)

        Model: \(model)
        \(transcript)

        """
    }
}
