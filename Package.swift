// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "LocalVoiceInput",
    platforms: [.macOS(.v15)],
    products: [
        .executable(name: "audio-microscope", targets: ["AudioMicroscope"]),
        .executable(name: "apple-speech-baseline", targets: ["AppleSpeechBaseline"]),
        .executable(name: "noise-mixer", targets: ["NoiseMixer"]),
        .executable(name: "parakeet-baseline", targets: ["ParakeetBaseline"]),
        .executable(name: "sentence-recorder", targets: ["SentenceRecorder"]),
        .executable(name: "hold-to-talk", targets: ["HoldToTalk"]),
    ],
    dependencies: [
        .package(
            url: "https://github.com/FluidInference/FluidAudio.git",
            exact: "0.15.5"
        ),
    ],
    targets: [
        .executableTarget(
            name: "AudioMicroscope",
            dependencies: [
                .product(name: "FluidAudio", package: "FluidAudio"),
            ]
        ),
        .executableTarget(name: "AppleSpeechBaseline"),
        .executableTarget(name: "NoiseMixer"),
        .executableTarget(
            name: "ParakeetBaseline",
            dependencies: [
                .product(name: "FluidAudio", package: "FluidAudio"),
            ]
        ),
        .executableTarget(name: "SentenceRecorder"),
        .executableTarget(
            name: "HoldToTalk",
            dependencies: [
                .product(name: "FluidAudio", package: "FluidAudio"),
            ]
        ),
    ],
    swiftLanguageModes: [.v6]
)
