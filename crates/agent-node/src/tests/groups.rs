use crate::*;
use policies::{Member, Mode, Policy};

#[tokio::test]
async fn durable_context_is_scoped_ordered_and_keeps_interrupted_progress() {
    let s = state("context").await;
    let p = Uuid::new_v4();
    let topic = Uuid::new_v4();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    for (project, session, kind, id, text) in [
        (p, topic, "message.created", first, "original requirement"),
        (p, topic, "agent.tool.finished", first, "saved checkpoint"),
        (p, topic, "agent.delta", first, "partial work"),
        (p, topic, "message.created", second, "amend requirement"),
        (
            Uuid::new_v4(),
            topic,
            "agent.message",
            first,
            "OTHER PROJECT SECRET",
        ),
        (
            p,
            Uuid::new_v4(),
            "agent.message",
            first,
            "OTHER TOPIC SECRET",
        ),
    ] {
        assert!(
            persist_event(
                &s,
                project,
                &format!("session:{session}"),
                &event(session, kind, json!({"message_id":id,"content":text}))
            )
            .await
        );
    }
    let text = conversation::context(&s, p, topic, Some(second))
        .await
        .unwrap();
    assert!(text.contains("original requirement"));
    assert!(text.contains("saved checkpoint"));
    assert!(text.contains("partial work"));
    assert!(!text.contains("amend requirement"));
    assert!(!text.contains("SECRET"));
}

#[tokio::test]
async fn interrupt_fences_late_output_and_restart_never_reexecutes() {
    let s = state("interrupt").await;
    let p = Uuid::new_v4();
    let topic = Uuid::new_v4();
    let id = Uuid::new_v4();
    let (events, _) = broadcast::channel(16);
    s.sessions.lock().await.insert(
        topic,
        Session {
            project_id: p,
            client_id: Some("worker".into()),
            events,
            requests: HashMap::from([(id, "worker".into())]),
        },
    );
    s.store
        .insert(
            "runs",
            &id.to_string(),
            json!({"id":id,"session_id":topic,"project_id":p,"status":"running","local":true}),
        )
        .await
        .unwrap();
    let _ = conversation::interrupt_session(&s, topic).await.unwrap();
    assert!(conversation::stopped(&s, id).await);
    assert!(
        !persist_event(
            &s,
            p,
            &format!("session:{topic}"),
            &event(
                topic,
                "agent.message",
                json!({"message_id":id,"content":"late"})
            )
        )
        .await
    );
    assert!(
        accept_client_event(
            s.clone(),
            Credential {
                project_id: p,
                client_id: "worker".into(),
                role: "worker".into()
            },
            ClientEvent {
                session_id: topic,
                message_id: id,
                kind: "agent.done".into(),
                content: None,
                error: None,
                error_code: None
            }
        )
        .await
        .is_err()
    );
    let queued = Uuid::new_v4();
    s.store
        .insert(
            "runs",
            &queued.to_string(),
            json!({"id":queued,"session_id":topic,"status":"queued"}),
        )
        .await
        .unwrap();
    conversation::recover(&s).await.unwrap();
    assert!(conversation::stopped(&s, queued).await);
    assert!(s.clients.lock().await.is_empty());
}

#[tokio::test]
async fn old_management_api_is_retired_and_chat_transport_requires_admin() {
    let state = state("http-fixture").await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let client = reqwest::Client::new();
    for path in [
        "/v1/carbot/control",
        "/v1/carbot/runs",
        "/v1/spaces",
        "/v1/management/start",
    ] {
        assert_eq!(
            client
                .post(format!("{url}{path}"))
                .header("x-admin-token", "test-admin")
                .json(&json!({}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        client
            .get(format!("{url}/v1/repl"))
            .header("origin", "https://evil.example")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .get(format!("{url}/v1/repl"))
            .header("x-admin-token", "test-admin")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    server.abort();
}

#[tokio::test]
async fn nested_subgroups_freeze_recursively_and_keep_context_between_rounds() {
    let a = state("a").await;
    let b = state("b").await;
    let c = state("c").await;
    let (pa, pb, pc) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let logs = Arc::new(Mutex::new(Vec::new()));
    executor(&c, pc, "worker", logs.clone()).await;
    mount(&a, pa, "b", &b, pb).await;
    mount(&b, pb, "c", &c, pc).await;
    call(
        &c,
        pc,
        &[],
        "template.update",
        json!({"expected_version":0,"policy":relay("worker")}),
    )
    .await
    .unwrap();
    let mut discussion = relay("c");
    discussion.mode = Mode::A2a;
    discussion.rounds = 2;
    call(
        &b,
        pb,
        &[],
        "template.update",
        json!({"expected_version":0,"policy":discussion}),
    )
    .await
    .unwrap();
    let group = call(
        &a,
        pa,
        &[],
        "group.create",
        json!({"name":"nested","policy":relay("b")}),
    )
    .await
    .unwrap();
    let detail = call(&a, pa, &[], "group.get", json!({"key":group["key"]}))
        .await
        .unwrap();
    let key = detail["bindings"][0]["body"]["subgroup_key"].clone();
    let snapshot = call(
        &a,
        pa,
        &["b"],
        "execution.freeze",
        json!({"key":key,"content":"research"}),
    )
    .await
    .unwrap();
    let frozen_child = &snapshot["body"]["members"]["c"];
    let mut changed = relay("worker");
    changed.instructions = "new-only".into();
    call(
        &c,
        pc,
        &[],
        "subgroup.update",
        json!({"key":frozen_child["body"]["subgroup_key"],"expected_version":1,"policy":changed}),
    )
    .await
    .unwrap();
    let result = call(
        &a,
        pa,
        &["b"],
        "task.run",
        json!({"execution_id":snapshot["key"],"invocation_id":Uuid::new_v4(),"content":"research"}),
    )
    .await
    .unwrap();
    assert!(
        result["answer"]
            .as_str()
            .unwrap()
            .contains("worker completed")
    );
    let logs = logs.lock().await;
    assert_eq!(logs.len(), 2);
    assert!(!logs.iter().any(|s| s.contains("new-only")));
    assert!(logs[1].contains("worker completed"));
}

pub(crate) async fn state(id: &str) -> AppState {
    let store = storage::Store::memory();
    AppState {
        policy_store: policy_store::Store::memory(),
        control_pending: Default::default(),
        sessions: Default::default(),
        clients: Default::default(),
        store,
        node_id: id.into(),
        links: Arc::new(vec![]),
        link_status: Default::default(),
    }
}
fn relay(id: &str) -> Policy {
    Policy {
        relay_strategy: Default::default(),
        mode: Mode::Relay,
        members: vec![Member {
            path: vec![id.into()],
            role: "worker".into(),
        }],
        leader: None,
        rounds: 1,
        instructions: String::new(),
    }
}
async fn call(
    s: &AppState,
    p: Uuid,
    path: &[&str],
    op: &str,
    input: Value,
) -> Result<Value, String> {
    control::call(
        s,
        p,
        path.iter().map(|x| (*x).into()).collect(),
        op.into(),
        input,
        vec![],
    )
    .await
}

async fn mount(parent: &AppState, p: Uuid, name: &str, child: &AppState, q: Uuid) {
    let (tx, mut rx) = mpsc::channel::<WireEvent>(32);
    parent.clients.lock().await.insert(
        (p, name.into()),
        ClientConnection {
            sender: tx,
            role: "Carbot".into(),
            node_id: Some(child.node_id.clone()),
        },
    );
    let (parent, child, name) = (parent.clone(), child.clone(), name.to_owned());
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let request: control::Request =
                serde_json::from_value(event.data["request"].clone()).unwrap();
            let (parent, child, name) = (parent.clone(), child.clone(), name.clone());
            tokio::spawn(async move {
                let id = request.id;
                let result = control::receive(child, q, parent.node_id.clone(), request).await;
                let reply = match result {
                    Ok(v) => control::Reply {
                        id,
                        result: Some(v),
                        error: None,
                    },
                    Err(e) => control::Reply {
                        id,
                        result: None,
                        error: Some(e),
                    },
                };
                let _ = control::finish(
                    &parent,
                    &Credential {
                        project_id: p,
                        client_id: name,
                        role: "Carbot".into(),
                    },
                    reply,
                )
                .await;
            });
        }
    });
}
async fn executor(s: &AppState, p: Uuid, id: &str, logs: Arc<Mutex<Vec<String>>>) {
    let (tx, mut rx) = mpsc::channel::<WireEvent>(32);
    s.clients.lock().await.insert(
        (p, id.into()),
        ClientConnection {
            sender: tx,
            role: id.into(),
            node_id: Some(s.node_id.clone()),
        },
    );
    let (s, id) = (s.clone(), id.to_owned());
    tokio::spawn(async move {
        while let Some(command) = rx.recv().await {
            let prompt = command.data["content"].as_str().unwrap();
            logs.lock().await.push(format!("{id}:{prompt}"));
            let (kind, answer) = if id == "limited" {
                ("agent.error", "TOKEN_INSUFFICIENT: fixture".into())
            } else if prompt.contains("RELAY NEGOTIATION ONLY") {
                (
                    "agent.message",
                    json!({"priority":if id=="worker"{95}else{20},"reason":"role match"})
                        .to_string(),
                )
            } else if prompt.contains("Return ONLY JSON") {
                (
                    "agent.message",
                    r#"{"assignments":[{"member":"worker","instruction":"do research"}]}"#.into(),
                )
            } else {
                ("agent.message", format!("{id} completed"))
            };
            let cred = Credential {
                project_id: p,
                client_id: id.clone(),
                role: id.clone(),
            };
            let message_id = serde_json::from_value(command.data["message_id"].clone()).unwrap();
            let _ = accept_client_event(
                s.clone(),
                cred.clone(),
                ClientEvent {
                    session_id: command.session_id,
                    message_id,
                    kind: kind.into(),
                    content: Some(answer),
                    error: None,
                    error_code: None,
                },
            )
            .await;
            if kind != "agent.error" {
                let _ = accept_client_event(
                    s.clone(),
                    cred,
                    ClientEvent {
                        session_id: command.session_id,
                        message_id,
                        kind: "agent.done".into(),
                        content: None,
                        error: None,
                        error_code: None,
                    },
                )
                .await;
            }
        }
    });
}

#[tokio::test]
async fn copied_policies_are_shared_versioned_and_isolated_from_defaults_and_other_groups() {
    let (a, b) = (state("A").await, state("B").await);
    let (p, q) = (Uuid::new_v4(), Uuid::new_v4());
    mount(&a, p, "b", &b, q).await;
    b.policy_store
        .put(
            q,
            "template",
            "default",
            0,
            json!({"policy":relay("worker")}),
        )
        .await
        .unwrap();
    let g = Uuid::new_v4().to_string();
    let h = Uuid::new_v4().to_string();
    let bg = call(&a, p, &["b"], "subgroup.ensure", json!({"group_id":g}))
        .await
        .unwrap();
    let bh = call(&a, p, &["b"], "subgroup.ensure", json!({"group_id":h}))
        .await
        .unwrap();
    let mut edited = relay("worker");
    edited.mode = Mode::A2a;
    edited.rounds = 2;
    let v2 = call(
        &a,
        p,
        &["b"],
        "subgroup.update",
        json!({"key":bg["key"],"expected_version":1,"policy":edited}),
    )
    .await
    .unwrap();
    assert_eq!(v2["version"], 2);
    assert_eq!(
        call(
            &b,
            q,
            &[],
            "subgroup.update",
            json!({"key":bg["key"],"expected_version":1,"policy":relay("worker")})
        )
        .await
        .unwrap_err(),
        "version_conflict"
    );
    edited.instructions = "B edited this shared copy".into();
    call(
        &b,
        q,
        &[],
        "subgroup.update",
        json!({"key":bg["key"],"expected_version":2,"policy":edited}),
    )
    .await
    .unwrap();
    let shared = call(&a, p, &["b"], "subgroup.get", json!({"key":bg["key"]}))
        .await
        .unwrap();
    assert_eq!(shared["version"], 3);
    assert_eq!(
        shared["body"]["policy"]["instructions"],
        "B edited this shared copy"
    );
    call(
        &b,
        q,
        &[],
        "template.update",
        json!({"expected_version":1,"policy":edited}),
    )
    .await
    .unwrap();
    let untouched = call(&b, q, &[], "subgroup.get", json!({"key":bh["key"]}))
        .await
        .unwrap();
    assert_eq!(untouched["version"], 1);
    assert_eq!(untouched["body"]["policy"]["mode"], "relay");
    let same = call(&a, p, &["b"], "subgroup.ensure", json!({"group_id":g}))
        .await
        .unwrap();
    assert_eq!(same["version"], 3);
}

#[tokio::test]
async fn snapshots_keep_the_old_policy_and_support_multiple_phases_without_duplicate_execution() {
    let (a, b) = (state("A").await, state("B").await);
    let (p, q) = (Uuid::new_v4(), Uuid::new_v4());
    let logs = Arc::new(Mutex::new(vec![]));
    executor(&b, q, "worker", logs.clone()).await;
    mount(&a, p, "b", &b, q).await;
    b.policy_store
        .put(
            q,
            "template",
            "default",
            0,
            json!({"policy":relay("worker")}),
        )
        .await
        .unwrap();
    let bg = call(
        &a,
        p,
        &["b"],
        "subgroup.ensure",
        json!({"group_id":Uuid::new_v4()}),
    )
    .await
    .unwrap();
    let frozen = call(
        &a,
        p,
        &["b"],
        "execution.freeze",
        json!({"key":bg["key"],"content":"initial"}),
    )
    .await
    .unwrap();
    let mut changed = relay("worker");
    changed.instructions = "NEW POLICY".into();
    call(
        &b,
        q,
        &[],
        "subgroup.update",
        json!({"key":bg["key"],"expected_version":1,"policy":changed}),
    )
    .await
    .unwrap();
    let invocation = Uuid::new_v4();
    let input =
        json!({"execution_id":frozen["key"],"invocation_id":invocation,"content":"phase one"});
    let answer = call(&a, p, &["b"], "task.run", input.clone())
        .await
        .unwrap();
    assert_eq!(answer["policy_version"], 1);
    assert_eq!(
        call(&a, p, &["b"], "task.run", input).await.unwrap_err(),
        "version_conflict"
    );
    call(
        &a,
        p,
        &["b"],
        "task.run",
        json!({"execution_id":frozen["key"],"invocation_id":Uuid::new_v4(),"content":"phase two"}),
    )
    .await
    .unwrap();
    let logs = logs.lock().await;
    assert_eq!(logs.len(), 2);
    assert!(logs[0].contains("phase one") && logs[1].contains("phase two"));
    assert!(logs.iter().all(|x| !x.contains("NEW POLICY")));
}

#[tokio::test]
async fn discovery_is_recursive_but_never_walks_up_and_replies_are_identity_bound() {
    let (a, b, c) = (state("A").await, state("B").await, state("C").await);
    let (p, q, r) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    mount(&a, p, "b", &b, q).await;
    mount(&b, q, "c", &c, r).await;
    let tree = call(&a, p, &[], "tree.get", json!({})).await.unwrap();
    assert_eq!(
        tree["children"][0]["node"]["children"][0]["node"]["id"],
        "C"
    );
    assert!(call(&b, q, &[".."], "tree.get", json!({})).await.is_err());
    assert!(call(&b, q, &["a"], "tree.get", json!({})).await.is_err());
    assert!(
        call(&a, Uuid::new_v4(), &["b"], "tree.get", json!({}))
            .await
            .is_err()
    );
    let id = Uuid::new_v4();
    let (tx, _) = tokio::sync::oneshot::channel();
    // Check identity binding through a pending call with a deliberately wrong project.
    a.control_pending
        .lock()
        .await
        .insert(id, control::Pending::fixture(p, "b", tx));
    assert_eq!(
        control::finish(
            &a,
            &Credential {
                project_id: q,
                client_id: "b".into(),
                role: "".into()
            },
            control::Reply {
                id,
                result: Some(json!({})),
                error: None
            }
        )
        .await,
        Err(StatusCode::FORBIDDEN)
    );
    assert!(a.control_pending.lock().await.contains_key(&id));
}

#[tokio::test]
async fn competing_edits_have_exactly_one_winner() {
    let store = policy_store::Store::memory();
    let p = Uuid::new_v4();
    store
        .put(p, "template", "default", 0, json!({"x":0}))
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        store.put(p, "template", "default", 1, json!({"x":1})),
        store.put(p, "template", "default", 1, json!({"x":2}))
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(
        store.get(p, "template", "default").await.unwrap().version,
        2
    );
}

#[tokio::test]
async fn relay_a2a_and_pmo_execute_real_member_dispatch() {
    let s = state("root").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["limited", "leader", "worker"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("limited");
    policy.members.push(Member {
        path: vec!["worker".into()],
        role: "researcher".into(),
    });
    let (tx, _) = mpsc::channel(128);
    let answer = policies::engine(&s, p, &policy, "task", None, tx, &[])
        .await
        .unwrap();
    assert_eq!(answer, "worker completed");
    policy.members[0] = Member {
        path: vec!["leader".into()],
        role: "PMO".into(),
    };
    policy.mode = Mode::A2a;
    policy.rounds = 2;
    logs.lock().await.clear();
    let (tx, _) = mpsc::channel(128);
    policies::engine(&s, p, &policy, "discuss", None, tx, &[])
        .await
        .unwrap();
    assert_eq!(logs.lock().await.len(), 4);
    assert!(logs.lock().await[1].contains("leader completed"));
    policy.mode = Mode::Pmo;
    policy.leader = Some(vec!["leader".into()]);
    logs.lock().await.clear();
    let (tx, _) = mpsc::channel(128);
    policies::engine(&s, p, &policy, "plan", None, tx, &[])
        .await
        .unwrap();
    let logs = logs.lock().await;
    assert_eq!(logs.len(), 3);
    assert!(logs[1].contains("do research"));
    assert!(logs[2].contains("worker completed"));
}

#[tokio::test]
async fn relay_negotiates_before_dispatch_and_random_keeps_a_complete_order() {
    let s = state("relay-strategies").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["leader", "worker"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("leader");
    policy.members.push(Member {
        path: vec!["worker".into()],
        role: "research".into(),
    });
    policy.relay_strategy = crate::core::relay::Strategy::Negotiated;
    let (tx, mut rx) = mpsc::channel(128);
    let answer = policies::engine(&s, p, &policy, "task", None, tx, &[])
        .await
        .unwrap();
    assert_eq!(answer, "worker completed");
    let calls = logs.lock().await.clone();
    assert_eq!(calls.len(), 3);
    assert!(calls[0].contains("RELAY NEGOTIATION ONLY"));
    assert!(calls[2].starts_with("worker:"));
    let mut output = String::new();
    while let Some(text) = rx.recv().await {
        output.push_str(&text)
    }
    assert!(output.contains("worker → leader"));
    policy.relay_strategy = crate::core::relay::Strategy::Random;
    logs.lock().await.clear();
    let (tx, mut rx) = mpsc::channel(128);
    policies::engine(&s, p, &policy, "task", None, tx, &[])
        .await
        .unwrap();
    assert_eq!(logs.lock().await.len(), 1);
    let mut output = String::new();
    while let Some(text) = rx.recv().await {
        output.push_str(&text)
    }
    assert!(output.contains("leader → worker") || output.contains("worker → leader"));
}

#[test]
fn policies_reject_overlapping_subtrees_and_unbounded_rounds() {
    let mut p = relay("b");
    p.members.push(Member {
        path: vec!["b".into(), "c".into()],
        role: "".into(),
    });
    assert!(p.validate().is_err());
    let mut p = relay("b");
    p.rounds = 99;
    assert!(p.validate().is_err());
}
