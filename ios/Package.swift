// swift-tools-version:5.5

import PackageDescription

// Push support is always compiled in; availability is gated at runtime by
// the Rust-side `push-notifications` cargo feature.
let swiftSettings: [SwiftSetting] = [.define("ENABLE_PUSH_NOTIFICATIONS")]

let package = Package(
  name: "tauri-plugin-notifications",
  platforms: [
    .macOS(.v10_13),
    .iOS(.v13),
  ],
  products: [
    // Products define the executables and libraries a package produces, and make them visible to other packages.
    .library(
      name: "tauri-plugin-notifications",
      type: .static,
      targets: ["tauri-plugin-notifications"])
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    // Targets are the basic building blocks of a package. A target can define a module or a test suite.
    // Targets can depend on other targets in this package, and on products in packages this package depends on.
    .target(
      name: "tauri-plugin-notifications",
      dependencies: [
        .byName(name: "Tauri")
      ],
      path: "Sources",
      swiftSettings: swiftSettings),
    .testTarget(
        name: "PluginTests",
        dependencies: ["tauri-plugin-notifications", .byName(name: "Tauri")]
    ),
  ]
)
