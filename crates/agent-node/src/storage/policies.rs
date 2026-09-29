use crate::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub key: String,
    pub version: u64,
    pub body: Value,
}
#[derive(Clone)]
pub struct Store(storage::Store);
impl Store {
    pub fn files(store: storage::Store) -> Self {
        Self(store)
    }
    #[cfg(test)]
    pub fn memory() -> Self {
        Self(storage::Store::memory())
    }
    fn namespace(p: Uuid, kind: &str) -> String {
        format!("documents:{p}:{kind}")
    }
    pub async fn get(&self, p: Uuid, kind: &str, key: &str) -> Result<Document, String> {
        serde_json::from_value(
            self.0
                .get(&Self::namespace(p, kind), key)
                .await
                .filter(|r| r["body"]["deleted"] != true)
                .ok_or("not_found")?,
        )
        .map_err(|e| e.to_string())
    }
    pub async fn list(&self, p: Uuid, kind: &str) -> Result<Vec<Document>, String> {
        let mut docs: Vec<Document> = self
            .0
            .list(&Self::namespace(p, kind))
            .await
            .into_iter()
            .filter(|r| r["body"]["deleted"] != true)
            .map(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        docs.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(docs)
    }
    pub async fn put(
        &self,
        p: Uuid,
        kind: &str,
        key: &str,
        expected: u64,
        body: Value,
    ) -> Result<Document, String> {
        let next = expected.checked_add(1).ok_or("version overflow")?;
        let namespace = Self::namespace(p, kind);
        self.0
            .transaction(|data| {
                let version = data
                    .get(&namespace, key)
                    .and_then(|v| v["version"].as_u64())
                    .unwrap_or(0);
                if version != expected {
                    return Err("version_conflict".into());
                }
                let doc = Document {
                    key: key.into(),
                    version: next,
                    body,
                };
                data.set(&namespace, key, json!(doc));
                Ok(doc)
            })
            .await
    }
}
