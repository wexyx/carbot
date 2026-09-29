use serde::{Deserialize, Serialize};
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccessRequest {
    pub path: String,
    #[serde(default)]
    pub write: bool,
}
