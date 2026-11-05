use std::path::Path;

use serde::{Deserialize, Serialize};
use zot_core::{SecretString, ZotError, ZotResult};

#[derive(Debug, Serialize, Deserialize)]
struct Credential {
    profile: Option<String>,
    server_id: String,
    key: SecretString,
}

fn read(path: &Path) -> ZotResult<Vec<Credential>> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
            super::local_write::invalid(
                "local-api-credentials",
                "Invalid Local API credential file; remove it to authorize again",
            )
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(ZotError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub(super) fn load(
    path: &Path,
    profile: Option<&str>,
    server: &str,
) -> ZotResult<Option<SecretString>> {
    Ok(read(path)?
        .into_iter()
        .find(|c| c.profile.as_deref() == profile && c.server_id == server)
        .map(|c| c.key)
        .filter(|key| !key.is_empty()))
}

pub(super) fn save(
    path: &Path,
    profile: Option<&str>,
    server: &str,
    key: Option<SecretString>,
) -> ZotResult<()> {
    let mut credentials = read(path)?;
    credentials.retain(|c| c.profile.as_deref() != profile || c.server_id != server);
    if let Some(key) = key {
        credentials.push(Credential {
            profile: profile.map(ToOwned::to_owned),
            server_id: server.to_string(),
            key,
        });
    }
    let parent = path.parent().ok_or_else(|| {
        super::local_write::invalid("local-api-credentials", "Credential path has no parent")
    })?;
    std::fs::create_dir_all(parent).map_err(|source| ZotError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let bytes = serde_json::to_vec(&credentials).map_err(|_| {
        super::local_write::invalid("local-api-credentials", "Cannot serialize credentials")
    })?;
    zot_core::config::write_secret_atomically(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_partitioned_redacted_and_removed_independently() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("credentials.json");
        save(&path, None, "server-a", Some("credential-canary".into())).expect("save");
        save(&path, Some("work"), "server-a", Some("work-key".into())).expect("save");
        assert!(load(&path, None, "server-b").expect("load").is_none());
        assert!(!format!("{:?}", read(&path).expect("read")).contains("credential-canary"));
        save(&path, None, "server-a", None).expect("remove");
        assert!(load(&path, None, "server-a").expect("load").is_none());
        assert_eq!(
            load(&path, Some("work"), "server-a")
                .expect("load")
                .expect("key")
                .expose_secret(),
            "work-key"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
