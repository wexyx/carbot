use super::{ToolContext, ToolSession};
use crate::attachments::AttachmentStore;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    path: String,
}

struct ImageShow {
    context: Arc<ToolContext>,
}

#[crate::tools::tool(name="image_show",description="Display an existing local image from this workspace's Carbot tmp directory. Accepts a file path, never a URL. Returns an instance-local preview URL and reference; include the reference verbatim in your answer so Web renders the image. Does not analyze the image or fetch remote content.",parameters=json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}),runtime=crate)]
impl ImageShow {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        context.workdir().is_some().then_some(Self { context })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
        publish(
            self.context.workdir().ok_or("workspace required")?,
            &input.path,
            &AttachmentStore::default(),
        )
    }
}

pub(super) fn publish(root: &Path, input: &str, store: &AttachmentStore) -> Result<Value, String> {
    // This entry point is local-file-only. Never interpret input as a remote resource.
    if input.contains("://") || input.starts_with("//") || input.starts_with("data:") {
        return Err("image_show accepts local tmp files only, not URLs".into());
    }
    let temporary = crate::workspace::temporary_dir(root)?;
    let path = root.join(input).canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(&temporary) {
        return Err(
            "only images inside this workspace's Carbot tmp directory can be displayed".into(),
        );
    }
    let image = store.publish_image(&path)?;
    Ok(
        json!({"attachment":image,"preview_url":image.preview_url(),"reference":image.reference(),"display_only":true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_links_snapshot_only_tmp_images_and_never_fetch_urls() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = crate::workspace::temporary_dir(dir.path()).unwrap();
        let store = AttachmentStore::new(dir.path().join("private-attachments"));
        let image = tmp.join("shot.png");
        let bytes = b"\x89PNG\r\n\x1a\nfixture";
        std::fs::write(&image, bytes).unwrap();
        let result = publish(dir.path(), image.to_str().unwrap(), &store).unwrap();
        let id = result["attachment"]["id"].as_str().unwrap();
        assert!(
            result["preview_url"]
                .as_str()
                .unwrap()
                .ends_with("?preview=true")
        );
        std::fs::remove_file(&image).unwrap();
        assert_eq!(store.read(id).unwrap().1, bytes);
        let outside = dir.path().join("outside.png");
        std::fs::write(&outside, bytes).unwrap();
        assert!(publish(dir.path(), outside.to_str().unwrap(), &store).is_err());
        #[cfg(unix)]
        {
            let link = tmp.join("escape.png");
            std::os::unix::fs::symlink(&outside, &link).unwrap();
            assert!(publish(dir.path(), link.to_str().unwrap(), &store).is_err());
        }
        for input in [
            "http://127.0.0.1/private",
            "http://169.254.169.254/",
            "file:///etc/passwd",
            "data:image/png;base64,AAAA",
            "//host/share",
        ] {
            assert!(publish(dir.path(), input, &store).is_err());
        }
        let text = tmp.join("fake.png");
        std::fs::write(&text, "not an image").unwrap();
        assert!(publish(dir.path(), text.to_str().unwrap(), &store).is_err());
    }
}
