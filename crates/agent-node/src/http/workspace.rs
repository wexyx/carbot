use crate::*;
#[derive(Deserialize)]
pub(crate) struct Decision {
    allow: bool,
    #[serde(default)]
    conversation: bool,
}
pub(crate) async fn pending() -> Json<Value> {
    Json(json!({"requests":agent_runtime::workspace::pending()}))
}
pub(crate) async fn configuration() -> Json<Value> {
    Json(
        json!({"workdir":std::env::var("AGENT_WORKDIR").unwrap_or_else(|_|".".into()),"outside_access":std::env::var("AGENT_OUTSIDE_ACCESS").unwrap_or_else(|_|"deny".into()),"backend":"native","mutable":false}),
    )
}
pub(crate) async fn decide(
    Path(id): Path<Uuid>,
    Json(input): Json<Decision>,
) -> Result<Json<Value>, StatusCode> {
    if input.conversation {
        if !input.allow {
            return Err(StatusCode::BAD_REQUEST);
        }
        agent_runtime::workspace::allow_conversation(id).map_err(|_| StatusCode::CONFLICT)?;
    } else {
        agent_runtime::workspace::decide(id, input.allow).map_err(|_| StatusCode::CONFLICT)?;
    }
    Ok(Json(json!({"id":id,"allowed":input.allow})))
}
