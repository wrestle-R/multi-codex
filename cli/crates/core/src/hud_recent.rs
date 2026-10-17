//! Two recent Desktop shortcuts from Codex's own index, independent of live
//! visibility and activity. Never opens auth, transcripts or an app-server.
use crate::{hud, managed_files};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecentChat {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub recency: i64,
}
#[derive(Clone, Default)]
pub struct RecentChats {
    pub chats: Vec<RecentChat>,
    pub unavailable: bool,
}
type Cache = BTreeMap<std::path::PathBuf, (Instant, RecentChats)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

fn indexed(root: &Path, profile: &str) -> Result<Vec<RecentChat>, String> {
    uuid::Uuid::parse_str(profile).map_err(|_| "Invalid account")?;
    let home = managed_files::checked(
        root,
        &root.join("profiles").join(profile).join("codex-home"),
        false,
    )?;
    // Choose the newest numeric schema; never silently switch to an older
    // database if the installed schema cannot be read.
    let mut candidates = std::fs::read_dir(&home)
        .map_err(|_| "Chat index unavailable")?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name
                .strip_prefix("state_")?
                .strip_suffix(".sqlite")?
                .parse::<u32>()
                .ok()?;
            Some((version, entry.path()))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(version, _)| std::cmp::Reverse(*version));
    let Some((_, path)) = candidates.first() else {
        return Err("Chat index unavailable".into());
    };
    managed_files::checked(root, path, false)?;
    // WAL/journal siblings must also remain owned files.
    for suffix in ["-wal", "-shm", "-journal"] {
        managed_files::checked(
            root,
            &std::path::PathBuf::from(format!("{}{suffix}", path.display())),
            true,
        )?;
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "Chat index unavailable")?;
    connection
        .busy_timeout(Duration::from_millis(50))
        .map_err(|_| "Chat index busy")?;
    let mut schema = connection
        .prepare("PRAGMA table_info(threads)")
        .map_err(|_| "Chat index unavailable")?;
    let columns = schema
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(|_| "Chat index unavailable")?
        .collect::<Result<std::collections::BTreeSet<_>, _>>()
        .map_err(|_| "Chat index unavailable")?;
    if !["id", "title", "cwd", "updated_at", "archived", "source"]
        .iter()
        .all(|c| columns.contains(*c))
    {
        return Err("Unsupported chat index".into());
    }
    let times = ["recency_at_ms", "recency_at", "updated_at_ms", "updated_at"]
        .iter()
        .filter(|c| columns.contains(**c))
        .map(|c| {
            if c.ends_with("_ms") {
                c.to_string()
            } else {
                format!("{c}*1000")
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let name = if columns.contains("name") {
        "COALESCE(NULLIF(name,''),NULLIF(title,''),'Codex chat')"
    } else {
        "COALESCE(NULLIF(title,''),'Codex chat')"
    };
    let user = if columns.contains("has_user_event") {
        "AND has_user_event=1"
    } else {
        ""
    };
    let query = format!("SELECT id,{name},cwd,COALESCE({times},0) FROM threads WHERE archived=0 AND lower(source) NOT LIKE '%subagent%' {user} ORDER BY COALESCE({times},0) DESC,id LIMIT 2");
    let mut statement = connection
        .prepare(&query)
        .map_err(|_| "Unsupported chat index")?;
    let rows = statement
        .query_map([], |row| {
            Ok(RecentChat {
                id: row.get(0)?,
                title: row.get(1)?,
                cwd: row.get(2)?,
                recency: row.get(3)?,
            })
        })
        .map_err(|_| "Chat index unavailable")?;
    rows.map(|row| {
        let mut chat = row.map_err(|_| "Invalid chat index")?;
        uuid::Uuid::parse_str(&chat.id).map_err(|_| "Invalid chat identity")?;
        chat.title = hud::title(&serde_json::Value::String(chat.title));
        chat.cwd = chat.cwd.chars().take(2048).collect();
        Ok(chat)
    })
    .collect()
}

pub fn load(root: &Path, profile: &str) -> RecentChats {
    if uuid::Uuid::parse_str(profile).is_err() {
        return RecentChats {
            unavailable: true,
            ..Default::default()
        };
    }
    let file = root.join("hud/recent").join(format!("{profile}.json"));
    let Ok(mut cache) = CACHE.get_or_init(Default::default).lock() else {
        return RecentChats {
            unavailable: true,
            ..Default::default()
        };
    };
    if let Some((when, result)) = cache.get(&file) {
        if when.elapsed() < Duration::from_secs(2) {
            return result.clone();
        }
    }
    let previous = managed_files::checked(root, &file, false)
        .ok()
        .and_then(|_| hud::read_private(&file).ok())
        .and_then(|bytes| serde_json::from_slice::<Vec<RecentChat>>(&bytes).ok())
        .filter(|chats| {
            chats.len() <= 2 && chats.iter().all(|c| uuid::Uuid::parse_str(&c.id).is_ok())
        });
    let result = match indexed(root, profile) {
        Ok(chats) => {
            if previous.as_ref() != Some(&chats) {
                let _ = hud::atomic_json(&file, &chats);
            }
            RecentChats {
                chats,
                unavailable: false,
            }
        }
        Err(_) => RecentChats {
            chats: previous
                .or_else(|| cache.get(&file).map(|(_, r)| r.chats.clone()))
                .unwrap_or_default(),
            unavailable: true,
        },
    };
    cache.insert(file, (Instant::now(), result.clone()));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, String, std::path::PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let profile = uuid::Uuid::new_v4().to_string();
        let home = root
            .path()
            .join("profiles")
            .join(&profile)
            .join("codex-home");
        std::fs::create_dir_all(&home).unwrap();
        let path = home.join("state_5.sqlite");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE threads(id TEXT,title TEXT,name TEXT,cwd TEXT,updated_at INTEGER,recency_at_ms INTEGER,archived INTEGER,source TEXT,has_user_event INTEGER)").unwrap();
        (root, profile, path)
    }
    fn insert(path: &Path, time: i64, archived: bool, source: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        Connection::open(path)
            .unwrap()
            .execute(
                "INSERT INTO threads VALUES(?1,'Title',NULL,'/project',?2,NULL,?3,?4,1)",
                rusqlite::params![id, time, archived, source],
            )
            .unwrap();
        id
    }
    #[test]
    fn two_recent_idle_chats_need_no_observer_and_never_modify_the_index() {
        let (root, profile, path) = fixture();
        insert(&path, 1, false, "vscode");
        let second = insert(&path, 2, false, "vscode");
        let newest = insert(&path, 3, false, "vscode");
        insert(&path, 4, true, "vscode");
        insert(&path, 5, false, "subAgentReview");
        let before = std::fs::read(&path).unwrap();
        let chats = indexed(root.path(), &profile).unwrap();
        assert_eq!(
            chats.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            [newest.as_str(), second.as_str()]
        );
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
    #[test]
    fn deleted_or_archived_chats_disappear_and_recency_wins_over_background_updates() {
        let (root, profile, path) = fixture();
        let selected = insert(&path, 1, false, "vscode");
        insert(&path, 100, false, "cli");
        let db = Connection::open(&path).unwrap();
        db.execute(
            "UPDATE threads SET recency_at_ms=999000 WHERE id=?1",
            [&selected],
        )
        .unwrap();
        assert_eq!(indexed(root.path(), &profile).unwrap()[0].id, selected);
        db.execute("UPDATE threads SET archived=1 WHERE id=?1", [&selected])
            .unwrap();
        assert!(!indexed(root.path(), &profile)
            .unwrap()
            .iter()
            .any(|c| c.id == selected));
        db.execute("DELETE FROM threads", []).unwrap();
        assert!(indexed(root.path(), &profile).unwrap().is_empty());
    }
    #[test]
    fn index_unavailable_preserves_shortcuts_across_launcher_restarts() {
        let (root, profile, path) = fixture();
        let id = insert(&path, 1, false, "vscode");
        assert_eq!(load(root.path(), &profile).chats[0].id, id);
        std::fs::remove_file(path).unwrap();
        CACHE.get().unwrap().lock().unwrap().clear();
        let saved = load(root.path(), &profile);
        assert!(saved.unavailable);
        assert_eq!(saved.chats[0].id, id);
    }
    #[test]
    fn reads_committed_chats_while_the_app_keeps_its_wal_connection_open() {
        let (root, profile, path) = fixture();
        let writer = Connection::open(&path).unwrap();
        writer.execute_batch("PRAGMA journal_mode=WAL").unwrap();
        let newest = insert(&path, 5, false, "vscode");
        assert_eq!(indexed(root.path(), &profile).unwrap()[0].id, newest);
        writer
            .execute("DELETE FROM threads WHERE id=?1", [&newest])
            .unwrap();
        assert!(indexed(root.path(), &profile).unwrap().is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn linked_indexes_are_refused() {
        let (root, profile, path) = fixture();
        let elsewhere = tempfile::NamedTempFile::new().unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), path).unwrap();
        assert!(indexed(root.path(), &profile).is_err());
    }
}
