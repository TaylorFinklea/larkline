use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Layer, Registry};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CapturedEvent {
    level: Level,
    fields: BTreeMap<String, String>,
}

impl CapturedEvent {
    pub(crate) fn new<const N: usize>(level: Level, fields: [(&str, &str); N]) -> Self {
        Self {
            level,
            fields: fields
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        }
    }
}

#[derive(Clone)]
struct CaptureLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl<S> Layer<S> for CaptureLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.events
            .lock()
            .expect("capture mutex should not be poisoned")
            .push(CapturedEvent {
                level: *event.metadata().level(),
                fields: visitor.fields,
            });
    }
}

#[derive(Default)]
struct FieldVisitor {
    fields: BTreeMap<String, String>,
}

impl FieldVisitor {
    fn record_value<T: ToString + ?Sized>(&mut self, field: &Field, value: &T) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_value(field, &format_args!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field, value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field, &value);
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.record_value(field, &value);
    }
}

pub(crate) fn capture_event(action: impl FnOnce()) -> CapturedEvent {
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = Registry::default().with(CaptureLayer {
        events: Arc::clone(&events),
    });
    tracing::subscriber::with_default(subscriber, action);

    let mut captured = events.lock().expect("capture mutex should not be poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one tracing event");
    captured.pop().expect("captured event should exist")
}
