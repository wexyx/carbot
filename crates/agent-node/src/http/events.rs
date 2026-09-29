use axum::response::sse::Event;
use serde::Serialize;
pub(crate) fn sse<E: Serialize>(kind: &str, value: &E) -> Event {
    Event::default()
        .event(kind)
        .data(serde_json::to_string(value).unwrap())
}
