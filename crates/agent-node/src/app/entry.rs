use super::{application, startup};
pub fn run() {
    let interactive = match startup::configure() {
        Ok(None) => return,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
        Ok(Some(interactive)) => interactive,
    };
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("create runtime")
        .block_on(async {
            let dir = std::path::PathBuf::from(
                std::env::var_os("CARBOT_DATA_DIR").unwrap_or_else(|| ".carbot".into()),
            );
            let store = match crate::storage::open(&dir).await {
                Ok(store) => store,
                Err(error) => {
                    eprintln!("Cannot start Carbot instance: {error}");
                    std::process::exit(2);
                }
            };
            match crate::configuration::startup().await {
                Ok(config) => application::serve(interactive, config, store).await,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
            }
        });
}
