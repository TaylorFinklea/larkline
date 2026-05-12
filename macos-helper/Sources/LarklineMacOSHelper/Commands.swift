// Commands — request dispatcher and Phase 2.B smoke-test handlers.
//
// Phase 2.B ships two commands as the bare minimum to validate the
// protocol round-trip:
//   - `version` — returns helper_version + protocol_version
//   - `ping`    — returns {"pong": true}
//
// Phase 2.C–E will add EventKit-backed commands:
//   - `list_calendars`
//   - `events_for_range`
//   - `respond_to_invite`
//
// Adding a new command: extend `dispatch(_:)` with another case and add
// the handler below. Handler signature is `(Request) -> any Encodable`
// for success, throw `HelperError.bad("...")` for known-bad input.

import Foundation

enum HelperError: Error {
    case bad(String)
}

/// Routes a parsed request to the right handler. Returns either an
/// Encodable payload (caller wraps in OkResponse) or throws a
/// HelperError that the caller converts to an ErrResponse.
func dispatch(_ req: Request) throws -> any Encodable {
    switch req.command {
    case "version":
        return versionHandler()
    case "ping":
        return pingHandler()
    case "list_calendars":
        return try listCalendarsHandler(req)
    case "events_for_range":
        return try eventsForRangeHandler(req)
    default:
        throw HelperError.bad("unknown command: \(req.command)")
    }
}

private struct VersionPayload: Encodable {
    let helper_version: String
    let protocol_version: Int
}

private func versionHandler() -> VersionPayload {
    return VersionPayload(
        helper_version: helperVersion,
        protocol_version: protocolVersion
    )
}

private struct PingPayload: Encodable {
    let pong: Bool
}

private func pingHandler() -> PingPayload {
    return PingPayload(pong: true)
}
