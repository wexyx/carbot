use super::Manager;
use serde_json::{Value, json};
use std::sync::Arc;
impl Manager {
    pub(crate) async fn start_server(self: &Arc<Self>, input: Value) -> Result<Value, String> {
        let manager = self;
        if input.get("address").is_some() && input.get("port").is_some() {
            return Err("choose address or port, not both".into());
        }
        let configured = crate::http::web_settings::preferred(&manager.core().state().store).await;
        let address = if let Some(port) = input.get("port") {
            let port = port
                .as_u64()
                .filter(|p| *p <= 65535)
                .ok_or("port must be 0..65535")?;
            format!("127.0.0.1:{port}")
        } else {
            input["address"].as_str().unwrap_or(&configured).to_owned()
        };
        let parsed: std::net::SocketAddr =
            address.parse().map_err(|_| "address must be an IP:port")?;
        if !parsed.ip().is_loopback() {
            return Err(
                "public binding requires explicit startup configuration, not model authorization"
                    .into(),
            );
        }
        Ok(json!({"url":manager.web().start(manager.clone(),&address).await?}))
    }
}
