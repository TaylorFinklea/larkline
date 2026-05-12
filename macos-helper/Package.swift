// swift-tools-version: 5.9
//
// larkline-macos-helper — Swift CLI bundled alongside `lark` on macOS that
// exposes EventKit (Calendar) over a line-delimited JSON protocol on
// stdin/stdout. Larkline's calendar plugin shells out to this binary for
// rich event data (conference URLs, attendees, notes) that `icalbuddy`
// cannot provide. On Linux, `lark` ships without this binary; the calendar
// plugin falls back to `icalbuddy`.
//
// Build: `swift build -c release` → `.build/release/larkline-macos-helper`

import PackageDescription

let package = Package(
    name: "larkline-macos-helper",
    platforms: [
        // macOS 13 (Ventura) gives us EKVirtualConferenceDescriptor and the
        // modern requestFullAccessToEvents API. Older macOS uses the legacy
        // requestAccess(to:) which still works but the API is split.
        .macOS(.v13),
    ],
    products: [
        .executable(name: "larkline-macos-helper", targets: ["LarklineMacOSHelper"]),
    ],
    targets: [
        .executableTarget(
            name: "LarklineMacOSHelper",
            path: "Sources/LarklineMacOSHelper",
            // EventKit and Foundation are the only frameworks we touch in
            // Phase 2; resist the urge to pull in third-party deps.
            linkerSettings: [
                .linkedFramework("EventKit"),
                .linkedFramework("Foundation"),
            ]
        ),
    ]
)
