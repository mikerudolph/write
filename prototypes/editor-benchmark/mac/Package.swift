// swift-tools-version: 5.9
import Foundation
import PackageDescription

let packageRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().path

let package = Package(
    name: "EditorBench",
    platforms: [.macOS(.v14)],
    targets: [
        .target(name: "CWriteCore"),
        .executableTarget(
            name: "EditorBench",
            dependencies: ["CWriteCore"],
            linkerSettings: [
                .unsafeFlags(["-L\(packageRoot)/../core/target/release"]),
                .linkedLibrary("write_core"),
                .linkedLibrary("iconv"),
            ]
        ),
    ]
)
