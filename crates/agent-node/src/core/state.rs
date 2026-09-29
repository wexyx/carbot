use crate::*;
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) policy_store: policy_store::Store,
    pub(crate) control_pending: Arc<Mutex<HashMap<Uuid, control::Pending>>>,
    pub(crate) sessions: Arc<Mutex<HashMap<Uuid, Session>>>,
    pub(crate) clients: Arc<Mutex<HashMap<(Uuid, String), ClientConnection>>>,
    pub(crate) store: storage::Store,
    pub(crate) node_id: String,
    pub(crate) links: Arc<Vec<node::Link>>,
    pub(crate) link_status: Arc<Mutex<HashMap<String, String>>>,
}
pub(crate) struct Session {
    pub(crate) project_id: Uuid,
    pub(crate) client_id: Option<String>,
    pub(crate) events: broadcast::Sender<WireEvent>,
    pub(crate) requests: HashMap<Uuid, String>,
}
pub(crate) struct ClientConnection {
    pub(crate) sender: mpsc::Sender<WireEvent>,
    pub(crate) role: String,
    pub(crate) node_id: Option<String>,
}
#[derive(Deserialize)]
pub(crate) struct SendMessage {
    pub(crate) content: String,
    pub(crate) client_id: Option<String>,
    pub(crate) route: Option<RouteContext>,
}
#[derive(Deserialize)]
pub(crate) struct ClientEvent {
    pub(crate) session_id: Uuid,
    pub(crate) message_id: Uuid,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) content: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) error_code: Option<String>,
}
#[derive(Serialize)]
pub(crate) struct CredentialResponse {
    pub(crate) ak: String,
    pub(crate) sk: String,
    pub(crate) client_id: String,
    pub(crate) project_id: Uuid,
}
#[derive(Clone)]
pub(crate) struct Credential {
    pub(crate) client_id: String,
    pub(crate) project_id: Uuid,
    pub(crate) role: String,
}
