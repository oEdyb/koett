import Foundation

enum DictationFailurePresentation {
    static func message(
        error: String,
        transcriptionCompleted: Bool,
        audioSaved: Bool
    ) -> String {
        if audioSaved {
            return "Dictation failed · Audio saved"
        }
        let lowercased = error.lowercased()
        if transcriptionCompleted,
           lowercased.contains("on the clipboard"),
           lowercased.contains("could not be sent") {
            return "Text copied · Paste failed"
        }
        if lowercased.contains("microphone"), lowercased.contains("could not start") {
            return "Microphone couldn't start"
        }
        return "Dictation failed"
    }
}
