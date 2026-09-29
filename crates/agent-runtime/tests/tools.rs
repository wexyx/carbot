use agent_runtime::{
    skills::{ExecutionPolicy, SkillCatalog, SkillDefinition},
    tools::{ToolContext, ToolFactory, ToolRegistry, ToolSession, tool},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

struct Echo;
#[tool(name = "test_echo", description = "Echo")]
impl Echo {
    fn new(_context: Arc<ToolContext>) -> Option<Self> {
        Some(Self)
    }
    async fn execute(&self, args: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        Ok(args.clone())
    }
}

fn context(catalog: SkillCatalog) -> ToolContext {
    ToolContext::new(
        None,
        catalog,
        ExecutionPolicy::new("offline".into(), false).unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn macro_factory_and_dynamic_registration_use_same_contract() {
    let mut registry = ToolFactory::create(context(SkillCatalog::default())).unwrap();
    let mut session = ToolSession::default();
    assert_eq!(
        registry
            .execute("test_echo", &json!({"a":1}), &mut session)
            .await
            .unwrap(),
        json!({"a":1})
    );
    assert!(
        registry
            .register(Arc::new(Echo))
            .unwrap_err()
            .contains("duplicate")
    );
    let snapshot = registry.clone();
    assert!(registry.unregister("test_echo"));
    assert!(
        registry
            .execute("test_echo", &json!({}), &mut session)
            .await
            .is_err()
    );
    assert!(
        snapshot
            .execute("test_echo", &json!({}), &mut session)
            .await
            .is_ok()
    );
    registry.register(Arc::new(Echo)).unwrap();
    assert!(
        registry
            .execute("test_echo", &json!([]), &mut session)
            .await
            .is_err()
    );
    let mut empty = ToolRegistry::new();
    empty.register(Arc::new(Echo)).unwrap();
    assert_eq!(empty.definitions()[0].name(), "test_echo");
}
#[tokio::test]
async fn skill_permissions_and_load_state_are_scoped_to_session() {
    let skill = SkillDefinition::new(
        "demo".into(),
        "demo".into(),
        BTreeMap::from([
            ("SKILL.md".into(), "instructions".into()),
            ("scripts/run.py".into(), "print('ok')".into()),
        ]),
        true,
        true,
    )
    .unwrap();
    let registry = ToolFactory::create(context(SkillCatalog::new(vec![skill]).unwrap())).unwrap();
    let mut session = ToolSession::default();
    let args = json!({"skill_id":"demo","path":"scripts/run.py","args":[]});
    assert!(
        registry
            .execute("python_run", &args, &mut session)
            .await
            .unwrap_err()
            .contains("load skill_read")
    );
    assert_eq!(
        registry
            .execute("skill_read", &json!({"skill_id":"demo"}), &mut session)
            .await
            .unwrap()["instructions"],
        "instructions"
    );
    assert!(
        registry
            .execute("python_run", &args, &mut session)
            .await
            .unwrap_err()
            .contains("Python denied")
    );
    let mut other = ToolSession::default();
    assert!(
        registry
            .execute("python_run", &args, &mut other)
            .await
            .unwrap_err()
            .contains("load skill_read")
    );
    assert!(
        registry
            .execute("skill_read", &json!({"skill_id":"outside"}), &mut session)
            .await
            .is_err()
    );
    assert!(
        registry
            .execute(
                "skill_read",
                &json!({"skill_id":"demo","command":"evil"}),
                &mut session
            )
            .await
            .is_err()
    );
    let other = ToolFactory::create(context(SkillCatalog::default())).unwrap();
    assert!(!other.definitions().iter().any(|d| d.name() == "skill_read"));
}
