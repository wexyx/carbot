use crate::*;
pub(crate) fn hash_secret(secret: &str) -> String {
    format!("{:x}", Sha256::digest(secret.as_bytes()))
}
pub(crate) async fn auth_client(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Credential, StatusCode> {
    let ak = headers
        .get("x-agent-ak")
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let sk = headers
        .get("x-agent-sk")
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let row = state
        .store
        .get("credentials", ak)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if row["sk_hash"] != json!(hash_secret(sk)) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let project = storage::field(&row, "project_id");
    let client = storage::field(&row, "client_id");
    if state
        .store
        .get("peer_blocks", &format!("{project}:{client}"))
        .await
        .is_some_and(|r| r["disabled"] == true)
    {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(Credential {
        client_id: storage::field(&row, "client_id"),
        project_id: Uuid::parse_str(&storage::field(&row, "project_id"))
            .map_err(|_| StatusCode::UNAUTHORIZED)?,
        role: storage::field(&row, "role"),
    })
}
