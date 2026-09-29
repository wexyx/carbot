use super::shutdown;
use crate::*;
pub(crate) async fn serve(
    interactive: bool,
    config: agent_runtime::config::RuntimeConfig,
    store: storage::Store,
) {
    let data_dir = std::path::PathBuf::from(
        std::env::var("CARBOT_DATA_DIR").unwrap_or_else(|_| ".carbot".into()),
    );
    println!("Local data: {}", data_dir.display());
    agent_runtime::sandbox::initialize()
        .await
        .expect("initialize embedded sandbox");
    let node_id = node::identity(&store).await;
    let links = node::configuration();
    let state = AppState {
        policy_store: policy_store::Store::files(store.clone()),
        control_pending: Default::default(),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        clients: Arc::new(Mutex::new(HashMap::new())),
        store,
        node_id,
        links: Arc::new(links),
        link_status: Arc::new(Mutex::new(HashMap::new())),
    };
    storage::restore(&state).await;
    conversation::recover(&state)
        .await
        .expect("recover interrupted runs");
    local::initialize(&state).await;
    node::start(state.clone());
    node::restore_mounts(&state)
        .await
        .expect("restore peer mounts");
    let manager = management::Manager::new(core::Core::new(state));
    manager
        .core()
        .bootstrap()
        .await
        .expect("initialize local project");
    manager
        .configure(config)
        .await
        .expect("configure AdminAgent");
    let web = manager.web().clone();
    let addr = crate::http::web_settings::preferred(&manager.core().state().store).await;
    match web.start(manager.clone(), &addr).await {
        Ok(url) => println!("Server: {url}"),
        Err(error) if interactive => eprintln!("Server 未启动：{error}，可在会话中重新启动。"),
        Err(error) => {
            eprintln!("{error}");
            manager.stop().await;
            agent_runtime::sandbox::shutdown().await;
            return;
        }
    }
    if interactive {
        if let Err(error) = cli::run(manager.clone()).await {
            eprintln!("{error}");
        }
    } else {
        shutdown::signal().await;
    }
    manager.stop().await;
    web.stop().await;
    agent_runtime::sandbox::shutdown().await;
}
