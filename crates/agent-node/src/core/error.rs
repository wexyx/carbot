use serde_json::Value;
pub(crate) struct Detail(pub Value);
pub(crate) enum ErrorKind {
    Invalid,
    NotFound,
    Conflict,
    Internal,
}
