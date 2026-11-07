// swift-tools-version: 5.9
import Foundation
import PackageDescription

let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().path

let rustCore: [LinkerSetting] = [
    .unsafeFlags(["\(root)/../../target/release/libwrite_ffi.a"]),
]

let package = Package(
    name: "Write",
    platforms: [.macOS(.v14)],
    targets: [
        .target(name: "write_ffiFFI"),
        .target(name: "WriteCore", dependencies: ["write_ffiFFI"]),
        .target(name: "WriteApp", dependencies: ["WriteCore"]),
        .executableTarget(name: "Write", dependencies: ["WriteApp"], linkerSettings: rustCore),
        .executableTarget(name: "WriteSmokeTest", dependencies: ["WriteApp", "WriteCore"], linkerSettings: rustCore),
    ]
)
