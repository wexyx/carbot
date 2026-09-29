use serde_json::Value;
pub(super) fn render(op: &str, value: &Value) -> String {
    let text = |row: &Value, key: &str| row[key].as_str().unwrap_or("—").to_owned();
    match op {
        "agents.list" => value["agents"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| {
                format!(
                    "{} · {} · {} · {}{}",
                    text(r, "name"),
                    text(r, "id"),
                    text(r, "provider"),
                    if r["online"] == true {
                        "在线"
                    } else {
                        "已停止"
                    },
                    if r["kind"] == "remote" {
                        " · 远端只读"
                    } else {
                        ""
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "agents.show" => format!(
            "{}\nID：{}\n运行器：{}\n角色：{}\n版本：{}\n{}\n{}",
            text(value, "name"),
            text(value, "id"),
            text(value, "provider"),
            text(value, "role"),
            value["version"],
            if value["kind"] == "remote" {
                "远端只读：修改请在所属节点进行"
            } else {
                "本地 Agent"
            },
            value
                .get("policy")
                .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                .unwrap_or_default()
        ),
        "agents.save" => format!(
            "已保存 {} · {} · {} · 版本 {}",
            text(value, "client_id"),
            text(value, "provider"),
            text(value, "role"),
            value["version"]
        ),
        "projects.list" => value
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| {
                format!(
                    "{} · {} · {}",
                    text(&r["body"], "name"),
                    text(r, "key"),
                    text(&r["body"]["policy"], "mode")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "connections.list" => {
            if value.as_array().is_some_and(|r| r.is_empty()) {
                "尚无上游连接".into()
            } else {
                value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|r| {
                        format!(
                            "{} · {} · {} · {}",
                            text(r, "direction"),
                            text(r, "name"),
                            r["url"].as_str().or(r["node_id"].as_str()).unwrap_or("—"),
                            text(r, "status")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| "结果格式错误".into()),
    }
}
