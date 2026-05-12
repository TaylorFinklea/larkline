// Events.swift — EventKit `events_for_range` handler.
//
// Phase 2.D returns events for a date range across all calendars (or a
// caller-supplied subset). Each event includes the fields Calendar v2
// needs to render structured rows + actions in lark.nvim:
//
//   - id, title, location, notes (markdown-safe escaping happens in
//     the lark plugin, not here)
//   - start/end as ISO 8601 strings (with timezone offset preserved)
//   - allDay flag
//   - calendar source/title (for "─── Calendar Name" group headers)
//   - meetingURL extracted from EKVirtualConferenceDescriptor's
//     urlDescriptors, with a regex-scan fallback over location/notes
//     for older calendar sources that don't populate the structured
//     descriptor (Outlook-imported events being the common case)
//   - attendees (name + email + status, never expose more than the
//     local user already sees in Calendar.app)
//
// Request envelope:
//   {"id": "...", "command": "events_for_range",
//    "args": {"start_iso": "2026-05-12T00:00:00Z",
//             "end_iso":   "2026-05-19T00:00:00Z",
//             "calendar_ids": ["uuid", ...] /* optional, omitted = all */}}

import EventKit
import Foundation

// MARK: - JSON payload shapes

struct EventAttendee: Encodable {
    let name: String?
    let email: String?
    /// Raw EKParticipantStatus rawValue stringified. Callers map to
    /// human labels ("accepted" / "declined" / "tentative" / "needs_action").
    let status: String
    /// "organizer" / "required" / "optional" / "chair" / "unknown".
    let role: String
    /// True if this attendee is the local user (matches the event's
    /// "self" participant). The lark plugin uses this to compute "your
    /// status" without doing a name match.
    let isCurrentUser: Bool
}

struct EventSummary: Encodable {
    let id: String
    let title: String
    let start_iso: String
    let end_iso: String
    let allDay: Bool
    let location: String?
    let notes: String?
    let meetingURL: String?
    let calendarId: String
    let calendarTitle: String
    let calendarSource: String
    let attendees: [EventAttendee]
}

struct EventsPayload: Encodable {
    let events: [EventSummary]
}

// MARK: - Conference URL extraction

/// Patterns we recognize as meeting URLs in unstructured event location
/// or notes text. Order matters: Teams first because its URL patterns
/// are the most distinctive. Each pattern is anchored so we don't match
/// "(See the Zoom link in calendar)" as a literal join URL.
private let meetingURLPatterns: [NSRegularExpression] = {
    let raw = [
        // Microsoft Teams meet URL — varies by tenant.
        #"https://teams\.microsoft\.com/l/meetup-join/[^\s<>"]+"#,
        #"https://teams\.live\.com/meet/[^\s<>"]+"#,
        // Zoom — both /j/ and /my/ formats.
        #"https://[a-z0-9.-]*zoom\.us/j/[0-9a-zA-Z?=&_-]+"#,
        #"https://[a-z0-9.-]*zoom\.us/my/[a-z0-9._-]+"#,
        // Google Meet.
        #"https://meet\.google\.com/[a-z]{3}-[a-z]{4}-[a-z]{3}"#,
        // Webex.
        #"https://[a-z0-9.-]*webex\.com/(?:meet|join)/[^\s<>"]+"#,
    ]
    return raw.compactMap { try? NSRegularExpression(pattern: $0, options: [.caseInsensitive]) }
}()

/// Best-effort meeting URL extraction.
///
/// Priority:
///   1. EKEvent.url — when present and matches a meeting URL pattern
///      (Calendar.app and Google CalDAV populate this from
///      `conferenceData` for Meet/Zoom/Teams events).
///   2. Regex scan over event location (Outlook dumps Teams join URLs
///      here when importing from .ics).
///   3. Regex scan over event notes (older calendar sources sometimes
///      put the URL in the description body).
///
/// Note: EKVirtualConferenceDescriptor would be the canonical source
/// on macOS 13+, but its public Swift API for synchronous reads is
/// unstable across SDK versions. event.url covers the modern case
/// (Calendar.app already extracts conferenceData → event.url) and the
/// regex fallback covers the legacy/Outlook case. Revisit if a v1.1
/// beta tester reports a missed URL.
private func extractMeetingURL(_ event: EKEvent) -> String? {
    if let url = event.url?.absoluteString,
       isMeetingURL(url) {
        return url
    }
    if let location = event.location,
       let match = firstMeetingURLMatch(in: location) {
        return match
    }
    if let notes = event.notes,
       let match = firstMeetingURLMatch(in: notes) {
        return match
    }
    return nil
}

/// True if `s` looks like a conference URL by domain pattern. Used to
/// gate the `event.url` path so we don't surface generic webpages.
private func isMeetingURL(_ s: String) -> Bool {
    for pattern in meetingURLPatterns where pattern.firstMatch(
        in: s,
        range: NSRange(s.startIndex..., in: s)
    ) != nil {
        return true
    }
    return false
}

/// First meeting URL inside `haystack`, or nil.
private func firstMeetingURLMatch(in haystack: String) -> String? {
    let range = NSRange(haystack.startIndex..., in: haystack)
    for pattern in meetingURLPatterns {
        if let m = pattern.firstMatch(in: haystack, range: range),
           let r = Range(m.range, in: haystack) {
            return String(haystack[r])
        }
    }
    return nil
}

// MARK: - Attendee mapping

private func mapParticipantStatus(_ status: EKParticipantStatus) -> String {
    switch status {
    case .accepted: return "accepted"
    case .declined: return "declined"
    case .tentative: return "tentative"
    case .pending: return "needs_action"
    case .delegated: return "delegated"
    case .completed: return "completed"
    case .inProcess: return "in_process"
    case .unknown: return "unknown"
    @unknown default: return "unknown"
    }
}

private func mapParticipantRole(_ role: EKParticipantRole) -> String {
    switch role {
    case .required: return "required"
    case .optional: return "optional"
    case .chair: return "chair"
    case .nonParticipant: return "non_participant"
    case .unknown: return "unknown"
    @unknown default: return "unknown"
    }
}

private func mapAttendee(_ p: EKParticipant) -> EventAttendee {
    // EKParticipant doesn't expose .email directly on macOS — only via
    // the url property (mailto:...). We strip the scheme.
    let urlString = p.url.absoluteString
    let email: String? = {
        let prefix = "mailto:"
        return urlString.lowercased().hasPrefix(prefix)
            ? String(urlString.dropFirst(prefix.count))
            : nil
    }()

    return EventAttendee(
        name: p.name,
        email: email,
        status: mapParticipantStatus(p.participantStatus),
        role: mapParticipantRole(p.participantRole),
        isCurrentUser: p.isCurrentUser
    )
}

// MARK: - Handler

private let iso8601: ISO8601DateFormatter = {
    let f = ISO8601DateFormatter()
    // .withInternetDateTime emits the RFC 3339 form "YYYY-MM-DDTHH:MM:SS±HH:MM".
    // Setting timeZone = .current makes the offset reflect the user's local
    // zone so downstream Lua plugins can display HH:MM directly without
    // timezone math. EventKit dates are timezone-naive (absolute moments);
    // the formatter does the zone application.
    f.formatOptions = [.withInternetDateTime]
    f.timeZone = TimeZone.current
    return f
}()

func eventsForRangeHandler(_ req: Request) throws -> EventsPayload {
    // Parse args.
    guard let args = req.args,
          let startStr = args["start_iso"]?.stringValue,
          let endStr = args["end_iso"]?.stringValue else {
        throw HelperError.bad("missing required args: start_iso, end_iso (ISO 8601)")
    }

    guard let start = iso8601.date(from: startStr) else {
        throw HelperError.bad("invalid start_iso: \(startStr)")
    }
    guard let end = iso8601.date(from: endStr) else {
        throw HelperError.bad("invalid end_iso: \(endStr)")
    }

    let store = makeEventStore()
    guard requestCalendarAccess(store) else {
        throw HelperError.bad("calendar access denied — grant in System Settings → Privacy & Security → Calendars, then retry")
    }

    // Optional calendar filter: caller may scope to specific calendars
    // by id (returned by list_calendars). Omitting → all calendars.
    let allCalendars = store.calendars(for: .event)
    let calendars: [EKCalendar] = {
        if case .array(let ids)? = args["calendar_ids"] {
            let wanted = Set(ids.compactMap { $0.stringValue })
            return allCalendars.filter { wanted.contains($0.calendarIdentifier) }
        }
        return allCalendars
    }()

    let predicate = store.predicateForEvents(
        withStart: start,
        end: end,
        calendars: calendars.isEmpty ? nil : calendars
    )
    let ekEvents = store.events(matching: predicate)

    let summaries = ekEvents.map { ev -> EventSummary in
        let attendees = (ev.attendees ?? []).map(mapAttendee)
        return EventSummary(
            id: ev.eventIdentifier,
            title: ev.title ?? "",
            start_iso: iso8601.string(from: ev.startDate),
            end_iso: iso8601.string(from: ev.endDate),
            allDay: ev.isAllDay,
            location: ev.location,
            notes: ev.notes,
            meetingURL: extractMeetingURL(ev),
            calendarId: ev.calendar.calendarIdentifier,
            calendarTitle: ev.calendar.title,
            calendarSource: ev.calendar.source.title,
            attendees: attendees
        )
    }

    return EventsPayload(events: summaries)
}
