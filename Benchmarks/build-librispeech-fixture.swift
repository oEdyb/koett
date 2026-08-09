#!/usr/bin/env swift

import AVFoundation
import Foundation

private struct BuildLibriSpeechFixture {
    static func run() throws {
        let arguments = Array(CommandLine.arguments.dropFirst())
        guard arguments.count == 3,
              let targetSeconds = Double(arguments[2]),
              targetSeconds > 0 else {
            throw failure(
                "Usage: swift Benchmarks/build-librispeech-fixture.swift "
                    + "<LibriSpeech split folder> <output.wav> <minimum seconds>"
            )
        }

        let splitURL = URL(fileURLWithPath: arguments[0], isDirectory: true)
        let outputURL = URL(fileURLWithPath: arguments[1])
        let baseURL = outputURL.deletingPathExtension()
        let referenceURL = baseURL.appendingPathExtension("txt")
        let sourcesURL = baseURL.appendingPathExtension("sources.tsv")
        let finalURLs = [outputURL, referenceURL, sourcesURL]
        if let existingURL = finalURLs.first(where: {
            FileManager.default.fileExists(atPath: $0.path)
        }) {
            throw failure("Output already exists: \(existingURL.path)")
        }

        let temporaryStem = ".koett-fixture-\(UUID().uuidString)"
        let outputFolder = outputURL.deletingLastPathComponent()
        let temporaryAudioURL = outputFolder
            .appendingPathComponent(temporaryStem)
            .appendingPathExtension("wav")
        let temporaryReferenceURL = outputFolder
            .appendingPathComponent(temporaryStem)
            .appendingPathExtension("txt")
        let temporarySourcesURL = outputFolder
            .appendingPathComponent(temporaryStem)
            .appendingPathExtension("sources.tsv")
        let temporaryURLs = [temporaryAudioURL, temporaryReferenceURL, temporarySourcesURL]
        defer {
            for url in temporaryURLs {
                try? FileManager.default.removeItem(at: url)
            }
        }

        let transcriptByID = try loadTranscripts(in: splitURL)
        let audioURLs = try audioFiles(in: splitURL)
        guard !audioURLs.isEmpty else {
            throw failure("No FLAC files found in \(splitURL.path)")
        }

        let outputFormat = AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: 16_000,
            channels: 1,
            interleaved: false
        )!
        var elapsed = 0.0
        var references: [String] = []
        var sources: [String] = ["utterance_id\tduration_seconds\tpath"]

        do {
            let outputFile = try AVAudioFile(
                forWriting: temporaryAudioURL,
                settings: outputFormat.settings,
                commonFormat: .pcmFormatFloat32,
                interleaved: false
            )

            for audioURL in audioURLs where elapsed < targetSeconds {
                let inputFile = try AVAudioFile(forReading: audioURL)
                guard inputFile.processingFormat.sampleRate == outputFormat.sampleRate,
                      inputFile.processingFormat.channelCount == outputFormat.channelCount else {
                    throw failure("Unexpected audio format: \(audioURL.path)")
                }

                let capacity = AVAudioFrameCount(inputFile.length)
                guard let buffer = AVAudioPCMBuffer(
                    pcmFormat: inputFile.processingFormat,
                    frameCapacity: capacity
                ) else {
                    throw failure("Could not allocate an audio buffer for \(audioURL.path)")
                }
                try inputFile.read(into: buffer)
                try outputFile.write(from: buffer)

                let duration = Double(buffer.frameLength) / inputFile.processingFormat.sampleRate
                let utteranceID = audioURL.deletingPathExtension().lastPathComponent
                guard let reference = transcriptByID[utteranceID] else {
                    throw failure("No reference transcript for \(utteranceID)")
                }
                elapsed += duration
                references.append(reference)
                sources.append(
                    "\(utteranceID)\t\(String(format: "%.6f", duration))\t\(audioURL.path)"
                )
            }
        }

        try references.joined(separator: " ").appending("\n").write(
            to: temporaryReferenceURL,
            atomically: true,
            encoding: .utf8
        )
        try sources.joined(separator: "\n").appending("\n").write(
            to: temporarySourcesURL,
            atomically: true,
            encoding: .utf8
        )

        let moves = [
            (temporaryReferenceURL, referenceURL),
            (temporarySourcesURL, sourcesURL),
            (temporaryAudioURL, outputURL),
        ]
        var movedFinalURLs: [URL] = []
        do {
            for (temporaryURL, finalURL) in moves {
                try FileManager.default.moveItem(at: temporaryURL, to: finalURL)
                movedFinalURLs.append(finalURL)
            }
        } catch {
            for url in movedFinalURLs {
                try? FileManager.default.removeItem(at: url)
            }
            throw error
        }

        print(String(format: "Built %.3f seconds from %d utterances.", elapsed, references.count))
        print(outputURL.path)
    }

    private static func audioFiles(in folder: URL) throws -> [URL] {
        let keys: [URLResourceKey] = [.isRegularFileKey]
        guard let enumerator = FileManager.default.enumerator(
            at: folder,
            includingPropertiesForKeys: keys
        ) else {
            throw failure("Could not read \(folder.path)")
        }

        return enumerator.compactMap { item -> URL? in
            guard let url = item as? URL,
                  url.pathExtension.lowercased() == "flac" else { return nil }
            return url
        }.sorted { $0.lastPathComponent < $1.lastPathComponent }
    }

    private static func loadTranscripts(in folder: URL) throws -> [String: String] {
        guard let enumerator = FileManager.default.enumerator(at: folder, includingPropertiesForKeys: nil)
        else {
            throw failure("Could not read \(folder.path)")
        }

        var transcriptByID: [String: String] = [:]
        for case let url as URL in enumerator where url.lastPathComponent.hasSuffix(".trans.txt") {
            let contents = try String(contentsOf: url, encoding: .utf8)
            for line in contents.split(whereSeparator: \.isNewline) {
                let fields = line.split(separator: " ", maxSplits: 1)
                guard fields.count == 2 else { continue }
                transcriptByID[String(fields[0])] = String(fields[1])
            }
        }
        return transcriptByID
    }

    private static func failure(_ message: String) -> NSError {
        NSError(
            domain: "BuildLibriSpeechFixture",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
    }
}

do {
    try BuildLibriSpeechFixture.run()
} catch {
    fputs("Error: \(error.localizedDescription)\n", stderr)
    exit(EXIT_FAILURE)
}
