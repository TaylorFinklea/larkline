// Calendar.swift — EventKit-backed handlers.
//
// Phase 2.C: `list_calendars` enumerates the user's calendars across all
// sources (iCloud, Google CalDAV, Exchange, local) and returns a JSON
// array with id, title, source, allowsModifications, and CGColor.
//
// TCC permission: macOS prompts the user the first time the helper
// calls `EKEventStore.requestFullAccessToEvents` (or the legacy
// `requestAccess(to:)` on macOS 13 and earlier). If access is denied,
// the handler returns an ErrResponse with a help_url pointing at the
// System Settings → Privacy & Security → Calendars panel.
//
// We deliberately don't cache the EKEventStore between requests: the
// store reads write-event changes via NSNotification, and we want the
// caller to see fresh data on every list_calendars call. EKEventStore
// init is cheap (~5ms in practice).

import EventKit
import Foundation

/// Single source of truth for the calendar event store. Each handler
/// gets a fresh instance to avoid stale-cache surprises if the user
/// adds/removes calendars between requests.
func makeEventStore() -> EKEventStore {
    return EKEventStore()
}

/// Block until TCC access has been resolved (granted or denied).
/// EKEventStore exposes both a callback-based legacy API and (since
/// macOS 14) an async/await one. We use the callback variant with a
/// DispatchSemaphore so this handler stays synchronous from the
/// caller's perspective.
///
/// Returns true if full read+write access was granted.
func requestCalendarAccess(_ store: EKEventStore) -> Bool {
    let semaphore = DispatchSemaphore(value: 0)
    var granted = false

    if #available(macOS 14.0, *) {
        store.requestFullAccessToEvents { ok, _ in
            granted = ok
            semaphore.signal()
        }
    } else {
        // macOS 13 fallback. `.event` is the EKEntityType.
        store.requestAccess(to: .event) { ok, _ in
            granted = ok
            semaphore.signal()
        }
    }

    semaphore.wait()
    return granted
}

struct CalendarSummary: Encodable {
    let id: String
    let title: String
    let source: String
    let allowsModifications: Bool
    /// Hex color string like "#FF5733" — derived from the calendar's
    /// CGColor for use in the TUI / preview pane.
    let color: String?
}

struct ListCalendarsPayload: Encodable {
    let calendars: [CalendarSummary]
}

/// Convert a CGColor to a "#RRGGBB" hex string. Returns nil for colors
/// that can't be sampled (rare — usually means a calendar with no
/// configured color which falls through to its source default).
private func hexColor(_ cgColor: CGColor?) -> String? {
    guard let cgColor = cgColor,
          let components = cgColor.components,
          components.count >= 3 else {
        return nil
    }
    let r = Int((components[0] * 255).rounded())
    let g = Int((components[1] * 255).rounded())
    let b = Int((components[2] * 255).rounded())
    return String(format: "#%02X%02X%02X", r, g, b)
}

func listCalendarsHandler(_ req: Request) throws -> ListCalendarsPayload {
    let store = makeEventStore()
    guard requestCalendarAccess(store) else {
        throw HelperError.bad("calendar access denied — grant in System Settings → Privacy & Security → Calendars, then retry")
    }

    let summaries = store.calendars(for: .event).map { cal in
        CalendarSummary(
            id: cal.calendarIdentifier,
            title: cal.title,
            source: cal.source.title,
            allowsModifications: cal.allowsContentModifications,
            color: hexColor(cal.cgColor)
        )
    }

    return ListCalendarsPayload(calendars: summaries)
}
