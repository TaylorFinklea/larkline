// Protocol — line-delimited JSON over stdin/stdout.
//
// Wire format (one JSON object per line, separated by newlines):
//
//   Server → Client (on startup):
//     {"kind":"hello","helper_version":"0.1.0","protocol_version":1}
//
//   Client → Server (request):
//     {"id":"<correlation-id>","command":"<name>","args":{...}}
//
//   Server → Client (response):
//     {"id":"<echo>","ok":true,"data":{...}}
//     {"id":"<echo>","ok":false,"error":"reason"}
//
// `id` is opaque to the helper; callers use it to correlate concurrent
// requests. `args` is optional. The helper preserves order: responses
// come back in the same order requests went in (no concurrency on the
// helper side).

import Foundation

struct Request: Decodable {
    let id: String?
    let command: String
    let args: [String: JSONValue]?
}

/// Envelope for successful responses. `data` shape is command-specific;
/// the dispatcher passes through whatever the handler emits.
struct OkResponse<T: Encodable>: Encodable {
    let id: String?
    let ok: Bool
    let data: T

    init(id: String?, data: T) {
        self.id = id
        self.ok = true
        self.data = data
    }
}

/// Envelope for failures. `error` is a human-readable explanation; the
/// caller (lark calendar plugin) renders it in the error item with a
/// help_url to the helper's troubleshooting doc.
struct ErrResponse: Encodable {
    let id: String?
    let ok: Bool
    let error: String

    init(id: String?, error: String) {
        self.id = id
        self.ok = false
        self.error = error
    }
}

/// Minimal heterogeneous JSON value type, just enough to forward request
/// `args` through to handlers without committing to a typed schema at
/// the protocol layer. Each handler does its own decode of the args
/// it cares about (see Commands.swift).
enum JSONValue: Decodable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case null
    case array([JSONValue])
    case object([String: JSONValue])

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() {
            self = .null
        } else if let v = try? c.decode(Bool.self) {
            self = .bool(v)
        } else if let v = try? c.decode(Double.self) {
            self = .number(v)
        } else if let v = try? c.decode(String.self) {
            self = .string(v)
        } else if let v = try? c.decode([JSONValue].self) {
            self = .array(v)
        } else if let v = try? c.decode([String: JSONValue].self) {
            self = .object(v)
        } else {
            throw DecodingError.typeMismatch(
                JSONValue.self,
                .init(codingPath: decoder.codingPath, debugDescription: "unsupported JSON value")
            )
        }
    }

    /// Convenience accessor for handlers that need a specific shape from args.
    /// Returns nil when the path doesn't exist or doesn't match the expected type.
    var stringValue: String? {
        if case .string(let s) = self { return s }
        return nil
    }
}
