// swift-tools-version: 5.9
// Phase 1 of docs/IMK_PLAN.md — InputMethodKit helper for Audio Input.
//
// `swift build` produces a CLI executable suitable for testing the
// protocol + socket plumbing. The packaging script (build-app.sh, future)
// wraps the binary into a proper .app bundle with Info.plist that macOS's
// IMK system can register. Without the .app wrapper the binary still runs
// for local testing — it just won't be a system input source until
// installed under ~/Library/Input Methods/.

import PackageDescription

let package = Package(
    name: "ImkHelper",
    platforms: [
        // Match audio-input's minimumSystemVersion in tauri.conf.json.
        .macOS(.v12),
    ],
    products: [
        .executable(name: "imk-helper", targets: ["ImkHelper"]),
        // Expose protocol + socket server as a library so tests can drive
        // them without standing up the full IMK runtime.
        .library(name: "ImkHelperCore", targets: ["ImkHelperCore"]),
    ],
    dependencies: [],
    targets: [
        .target(
            name: "ImkHelperCore",
            path: "Sources/ImkHelperCore"
        ),
        .executableTarget(
            name: "ImkHelper",
            dependencies: ["ImkHelperCore"],
            path: "Sources/ImkHelper"
        ),
        // Tests are an *executable* target rather than a .testTarget
        // because Apple's Command Line Tools (no Xcode) don't ship
        // XCTest. Running `swift run ImkHelperTests` invokes the test
        // runner; non-zero exit on any failure. Keep this until/unless
        // Xcode is required for some other reason.
        .executableTarget(
            name: "ImkHelperTests",
            dependencies: ["ImkHelperCore"],
            path: "Tests/ImkHelperTests"
        ),
    ]
)
