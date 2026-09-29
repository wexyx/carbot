fn main() {
    println!("cargo:rerun-if-env-changed=CARBOT_RELEASE_VERSION");
    let version = match std::env::var("CARBOT_RELEASE_VERSION") {
        Ok(tag) => {
            let value = tag.strip_prefix('v').unwrap_or(&tag);
            assert!(
                !value.is_empty()
                    && value.len() <= 96
                    && value.as_bytes()[0].is_ascii_digit()
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b".-+_".contains(&b)),
                "CARBOT_RELEASE_VERSION must be a version tag, e.g. v1.2.3 or v1.2.3-rc.1"
            );
            format!("v{value}")
        }
        Err(std::env::VarError::NotPresent) => {
            format!("v{}-dev", std::env::var("CARGO_PKG_VERSION").unwrap())
        }
        Err(error) => panic!("invalid CARBOT_RELEASE_VERSION: {error}"),
    };
    println!("cargo:rustc-env=CARBOT_BUILD_VERSION={version}");
}
