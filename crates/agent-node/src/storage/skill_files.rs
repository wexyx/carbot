//! User Skill content belongs to the instance, never to the installed release.
//! Each save publishes a new directory before its journal reference is committed.
use agent_runtime::skills::SkillDefinition;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

const POINTER: &str = "skill_files_directory";

fn field(collection: &str, row: &Value) -> Option<(&'static str, &'static str)> {
    match collection {
        "skills" => Some(("skill", "business")),
        "management_skills" => Some(("definition", "management")),
        "capability_library" if row["kind"] == "skill" => match row["scope"].as_str()? {
            "business" => Some(("definition", "business")),
            "management" => Some(("definition", "management")),
            _ => None,
        },
        _ => None,
    }
}

fn directory(root: &Path, relative: &Path, create: bool) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for part in relative.components() {
        let Component::Normal(name) = part else {
            return Err("invalid Skill path".into());
        };
        path.push(name);
        if create {
            match fs::create_dir(&path) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(e.to_string()),
            }
        }
        if !fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_dir()
        {
            return Err(format!(
                "Skill directory must not be a symlink: {}",
                path.display()
            ));
        }
    }
    Ok(path)
}

pub(super) fn persist(
    root: &Path,
    collection: &str,
    key: &str,
    row: &Value,
) -> Result<Value, String> {
    let Some((field, scope)) = field(collection, row) else {
        return Ok(row.clone());
    };
    // Deletion is a tombstone: retain files so existing history remains recoverable.
    if row["deleted"] == true {
        return Ok(row.clone());
    }
    let definition: SkillDefinition =
        serde_json::from_value(row[field].clone()).map_err(|e| e.to_string())?;
    definition.validate()?;
    if definition
        .files()
        .keys()
        .any(|name| Path::new(name).components().count() > 9)
    {
        return Err("Skill directory nesting exceeds 8".into());
    }
    let identity = format!("{:x}", Sha256::digest(format!("{collection}:{key}")));
    let version = row["version"]
        .as_u64()
        .or_else(|| row["revision"].as_u64())
        .unwrap_or(0);
    let relative = PathBuf::from("skills/user")
        .join(scope)
        .join(format!("{}-{}", definition.id(), &identity[..16]))
        .join(format!("v{version}-{}", uuid::Uuid::new_v4()));
    let path = directory(root, &relative, true)?;
    for (name, content) in definition.files() {
        let file = Path::new(name);
        let parent = directory(&path, file.parent().unwrap_or(Path::new("")), true)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options
            .open(parent.join(file.file_name().ok_or("invalid Skill filename")?))
            .map_err(|e| e.to_string())?;
        output
            .write_all(content.as_bytes())
            .and_then(|_| output.sync_all())
            .map_err(|e| e.to_string())?;
        // Make directory entries durable before committing their journal reference.
        #[cfg(unix)]
        for ancestor in parent.ancestors() {
            fs::File::open(ancestor)
                .and_then(|d| d.sync_all())
                .map_err(|e| e.to_string())?;
            if ancestor == root {
                break;
            }
        }
    }
    let mut saved = row.clone();
    saved[field]
        .as_object_mut()
        .ok_or("invalid Skill definition")?
        .remove("files");
    saved[POINTER] = json!(relative.to_string_lossy());
    Ok(saved)
}

fn read_files(
    root: &Path,
    relative: &Path,
    files: &mut BTreeMap<String, String>,
    total: &mut usize,
) -> Result<(), String> {
    if relative.components().count() > 8 {
        return Err("Skill directory nesting exceeds 8".into());
    }
    for entry in fs::read_dir(root.join(relative)).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "Skill filename must be UTF-8")?;
        if name.starts_with('.') {
            continue;
        }
        let path = relative.join(name);
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            read_files(root, &path, files, total)?;
        } else if kind.is_file() {
            if files.len() >= 32 {
                return Err("Skill exceeds 32 files".into());
            }
            let mut content = String::new();
            fs::File::open(entry.path())
                .map_err(|e| e.to_string())?
                .take(65537)
                .read_to_string(&mut content)
                .map_err(|e| e.to_string())?;
            *total += content.len();
            if content.len() > 65536 || *total > 262144 {
                return Err("Skill content exceeds size limit".into());
            }
            files.insert(path.to_string_lossy().replace('\\', "/"), content);
        } else {
            return Err("Skill symlinks and special files are not allowed".into());
        }
    }
    Ok(())
}

pub(super) fn hydrate(root: &Path, collection: &str, row: &Value) -> Result<Value, String> {
    let Some((field, _)) = field(collection, row) else {
        return Ok(row.clone());
    };
    let Some(relative) = row[POINTER].as_str() else {
        return Ok(row.clone());
    };
    if row["deleted"] == true {
        return Ok(row.clone());
    }
    let relative = Path::new(relative);
    if !relative.starts_with("skills/user") {
        return Err("Skill path must be inside skills/user".into());
    }
    let path = directory(root, relative, false)?;
    let mut files = BTreeMap::new();
    read_files(&path, Path::new(""), &mut files, &mut 0)?;
    let mut result = row.clone();
    result[field]["files"] = json!(files);
    let definition: SkillDefinition =
        serde_json::from_value(result[field].clone()).map_err(|e| e.to_string())?;
    definition.validate()?;
    Ok(result)
}
