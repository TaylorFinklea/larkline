// larkline-macos-helper entry point.
//
// Phase 2.B: line-delimited JSON request/response loop on stdin/stdout.
// Emits a `hello` line on startup, then reads one JSON request per
// stdin line and writes one JSON response per stdout line. Exits when
// stdin closes (EOF) — Larkline kills the subprocess on plugin teardown.
//
// Each line is encoded with `outputFormatting = .sortedKeys` so the
// payload bytes are deterministic (helps integration test diff'ing
// without ordering flakiness).

import Foundation

// Build-time constants surfaced through the `version` command and the
// startup `hello` line. Phase 2.F (CI) will hydrate `helperVersion`
// from a build script; for now they're hardcoded.
let helperVersion = "0.1.0"
let protocolVersion = 1

let encoder: JSONEncoder = {
    let e = JSONEncoder()
    e.outputFormatting = [.sortedKeys]
    return e
}()

let decoder = JSONDecoder()

/// Write a single JSON object to stdout followed by a newline, flushing
/// immediately so the caller (lark plugin) sees responses as they
/// happen rather than buffered up.
func writeLine<T: Encodable>(_ value: T) {
    guard let data = try? encoder.encode(value),
          var line = String(data: data, encoding: .utf8) else {
        return
    }
    line.append("\n")
    FileHandle.standardOutput.write(Data(line.utf8))
}

// --- Startup hello ---

struct HelloMessage: Encodable {
    let kind: String
    let helper_version: String
    let protocol_version: Int
}

writeLine(HelloMessage(
    kind: "hello",
    helper_version: helperVersion,
    protocol_version: protocolVersion
))

// --- Request loop ---

while let line = readLine(strippingNewline: true) {
    // Empty line: ignore (allows callers to "tickle" the loop without a
    // real request).
    if line.isEmpty {
        continue
    }

    // Parse the request envelope. If the JSON is malformed, the
    // request id is unrecoverable — emit an error with id=null.
    guard let data = line.data(using: .utf8) else {
        writeLine(ErrResponse(id: nil, error: "request not valid UTF-8"))
        continue
    }

    let request: Request
    do {
        request = try decoder.decode(Request.self, from: data)
    } catch {
        writeLine(ErrResponse(id: nil, error: "invalid request JSON: \(error.localizedDescription)"))
        continue
    }

    // Dispatch. Each handler returns Encodable on success or throws
    // HelperError on known-bad input.
    do {
        let payload = try dispatch(request)
        // Wrap payload in OkResponse. We construct the response via a
        // small AnyEncodable shim because the protocol payload type is
        // existential (`any Encodable`).
        writeLine(_OkEnvelope(id: request.id, data: AnyEncodable(payload)))
    } catch let HelperError.bad(message) {
        writeLine(ErrResponse(id: request.id, error: message))
    } catch {
        writeLine(ErrResponse(id: request.id, error: "internal error: \(error.localizedDescription)"))
    }
}

// --- Helpers ---

/// `OkResponse<any Encodable>` doesn't work directly because Encodable
/// can't be used as a type parameter in a Codable context without an
/// existential workaround. AnyEncodable + a non-generic envelope keeps
/// the JSON shape (`{"id":..., "ok":true, "data":...}`) intact.
private struct _OkEnvelope: Encodable {
    let id: String?
    let ok: Bool = true
    let data: AnyEncodable

    init(id: String?, data: AnyEncodable) {
        self.id = id
        self.data = data
    }
}

private struct AnyEncodable: Encodable {
    let value: any Encodable

    init(_ value: any Encodable) {
        self.value = value
    }

    func encode(to encoder: Encoder) throws {
        try value.encode(to: encoder)
    }
}
