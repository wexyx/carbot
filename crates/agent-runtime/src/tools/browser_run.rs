use super::{ToolContext, ToolSession};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    url: String,
    #[serde(default)]
    screenshot: bool,
}
struct BrowserRun {
    context: Arc<ToolContext>,
}
#[crate::tools::tool(name="browser_run",description="Read an HTTP(S) page using Puppeteer and optionally save a screenshot. Host execution, retaining Chromium's own sandbox and a fresh temporary profile. Does not execute arbitrary scripts or shell commands. Include the returned image.reference verbatim in the final answer to display the screenshot in Web. Follow host approval; never bypass denials.",parameters=json!({"type":"object","properties":{"url":{"type":"string"},"screenshot":{"type":"boolean","default":false}},"required":["url"],"additionalProperties":false}),runtime=crate)]
impl BrowserRun {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        context.workdir().is_some().then_some(Self { context })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
        let mut result = crate::browser::BrowserExecution::run(
            self.context.workdir().ok_or("workspace required")?,
            &input.url,
            input.screenshot,
        )
        .await?;
        if let Some(path) = result["screenshot"].as_str() {
            let image = super::image_show::publish(
                self.context.workdir().ok_or("workspace required")?,
                path,
                &crate::attachments::AttachmentStore::default(),
            )?;
            result["image"] = image;
        }
        Ok(result)
    }
}
