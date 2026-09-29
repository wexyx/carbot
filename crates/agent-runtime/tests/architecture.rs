use std::{fs, path::Path};
#[test]
fn module_facades_only_declare_modules_and_reexport_symbols() {
    fn check(path: &Path) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
            } else if matches!(
                path.file_name().and_then(|n| n.to_str()),
                Some("mod.rs" | "lib.rs")
            ) {
                let file = syn::parse_file(&fs::read_to_string(&path).unwrap()).unwrap();
                for item in file.items {
                    assert!(
                        match item {
                            syn::Item::Use(_) => true,
                            syn::Item::Mod(module) => module.content.is_none(),
                            _ => false,
                        },
                        "{} contains non-facade code",
                        path.display()
                    );
                }
            }
        }
    }
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for name in ["agent-runtime", "agent-node", "agent-protocol"] {
        check(&crates.join(name).join("src"));
    }
}
