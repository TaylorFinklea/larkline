// larkline-macos-helper entry point.
//
// Phase 2.A: skeleton only — emit a single JSON line announcing protocol +
// helper version on startup, then exit. This validates that:
//   - the Swift package builds cleanly,
//   - the binary runs and prints to stdout,
//   - downstream Phase 2.B (JSON-line protocol) has a known stable entry.
//
// Phase 2.B will replace this body with a real stdin/stdout loop that
// reads JSON requests, dispatches to EventKit handlers, and writes JSON
// responses one per line.

import Foundation

// Hard-coded for Phase 2.A; Phase 2.B will surface this via a `version`
// command response and Phase 2.F will hydrate from a build-time constant.
let helperVersion = "0.1.0"
let protocolVersion = 1

struct HelloMessage: Encodable {
    let kind = "hello"
    let helper_version: String
    let protocol_version: Int
}

let encoder = JSONEncoder()
encoder.outputFormatting = [.sortedKeys]

let hello = HelloMessage(
    helper_version: helperVersion,
    protocol_version: protocolVersion
)

if let data = try? encoder.encode(hello),
   let line = String(data: data, encoding: .utf8) {
    print(line)
}
