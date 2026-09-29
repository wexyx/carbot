/// Baked into the binary by build.rs; never read from the user's runtime configuration.
pub(crate) const DISPLAY: &str = env!("CARBOT_BUILD_VERSION");
