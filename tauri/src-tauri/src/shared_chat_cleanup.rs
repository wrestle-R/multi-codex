//! Library-wide previewed deletion with persistent tombstones and writer guards.
use crate::{
    chat_cleanup::{ChatPreview, ChatScope},
    managed_files as files,
    profiles::{self, Result},
    shared_chat_leases::ThreadLeases,
    shared_chats::ChatLibrary,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};
use uuid::Uuid;

type Row = (String, String, i64, Value, bool);
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: u32,
    created: i64,
    rows: Vec<Row>,
    selected: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub deleted_count: usize,
    pub reclaimed_bytes: u64,
}
fn path(data: &Path, token: &str) -> Result<std::path::PathBuf> {
    if !Uuid::parse_str(token).is_ok_and(|id| id.to_string() == token) {
        return Err("Invalid shared cleanup preview".into());
    }
    let root = data.join("shared-chats/cleanup-previews");
    files::directory(data, &root)?;
    files::checked(data, &root.join(format!("{token}.json")), true)
}
pub fn preview(data: &Path, scope: ChatScope) -> Result<ChatPreview> {
    let library = ChatLibrary::open(data)?;
    let rows = library.cleanup_rows()?;
    let now = chrono::Utc::now().timestamp();
    let cutoff = now - 30 * 24 * 60 * 60;
    let mut protected = HashSet::new();
    let ids: HashSet<_> = rows.iter().map(|row| row.0.as_str()).collect();
    let mut parents = BTreeMap::new();
    let mut unknown_child = false;
    for (id, _, _, metadata, _) in &rows {
        let source = metadata["source"]
            .as_str()
            .or_else(|| metadata["source"]["type"].as_str())
            .unwrap_or("unknown");
        if let Some(parent) = metadata["parentThreadId"].as_str() {
            if !ids.contains(parent) {
                unknown_child = true;
                protected.insert(id.clone());
            }
            parents.insert(id.clone(), parent.to_owned());
        } else if source.to_lowercase().contains("subagent") || metadata["source"].is_object() {
            unknown_child = true;
            protected.insert(id.clone());
        }
        if ThreadLeases::unowned_guard(data, id).is_err() {
            protected.insert(id.clone());
        }
        if scope == ChatScope::OlderThan30Days
            && metadata["updatedAt"]
                .as_i64()
                .is_none_or(|time| time >= cutoff)
        {
            protected.insert(id.clone());
        }
    }
    // Unknown descendants cannot authorize deleting their possible ancestors.
    if unknown_child {
        protected.extend(rows.iter().map(|row| row.0.clone()));
    }
    for id in protected.clone() {
        let mut seen = HashSet::new();
        let mut parent = parents.get(&id);
        while let Some(id) = parent {
            if !seen.insert(id.clone()) {
                return Err("Invalid shared chat family; cleanup stopped".into());
            }
            protected.insert(id.clone());
            parent = parents.get(id);
        }
    }
    let selected: Vec<_> = rows
        .iter()
        .filter(|row| !protected.contains(&row.0))
        .map(|row| row.0.clone())
        .collect();
    let token = Uuid::new_v4().to_string();
    let plan = Plan {
        schema: 1,
        created: now,
        rows,
        selected,
    };
    profiles::write_private_file(
        &path(data, &token)?,
        &serde_json::to_vec(&plan).map_err(|_| "Cannot encode shared cleanup preview")?,
    )?;
    Ok(ChatPreview {
        token,
        count: plan.selected.len(),
        transcript_bytes: 0,
        protected_count: protected.len(),
    })
}
pub fn clear(data: &Path, token: &str) -> Result<CleanupResult> {
    let preview = files::regular(data, &path(data, token)?)?;
    let plan: Plan = serde_json::from_slice(
        &fs::read(&preview).map_err(|_| "Cannot read shared cleanup preview")?,
    )
    .map_err(|_| "Invalid shared cleanup preview")?;
    let now = chrono::Utc::now().timestamp();
    if plan.schema != 1 || now < plan.created || now - plan.created > 600 {
        return Err("Shared cleanup preview expired. Check again".into());
    }
    let library = ChatLibrary::open(data)?;
    if library.cleanup_rows()? != plan.rows {
        return Err("Shared chats changed. Check the preview again".into());
    }
    let _guards = plan
        .selected
        .iter()
        .map(|id| ThreadLeases::unowned_guard(data, id))
        .collect::<Result<Vec<_>>>()?;
    library.cleanup_tombstones(&plan.rows, &plan.selected)?;
    let reclaimed_bytes = library.collect_unreachable()?;
    fs::remove_file(preview)
        .map_err(|_| "Chats were cleared, but the preview could not be removed")?;
    Ok(CleanupResult {
        deleted_count: plan.selected.len(),
        reclaimed_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_protects_recent_families_rejects_stale_previews_and_never_resurrects_deleted_chats()
    {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("app");
        let source = data.join("source");
        fs::create_dir_all(&source).unwrap();
        let library = ChatLibrary::open(&data).unwrap();
        let profile = Uuid::new_v4().to_string();
        let parent = Uuid::new_v4().to_string();
        let child = Uuid::new_v4().to_string();
        let old = Uuid::new_v4().to_string();
        let mut imports = Vec::new();
        for (id, timestamp, ancestor) in [
            (&parent, 100, None),
            (&child, chrono::Utc::now().timestamp(), Some(&parent)),
            (&old, 100, None),
        ] {
            let rollout = source.join(format!("{id}.jsonl"));
            fs::write(&rollout, format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\"}}}}\n{{\"type\":\"event_msg\",\"payload\":{{\"message\":\"complete fixture context\"}}}}\n")).unwrap();
            let metadata = serde_json::json!({"id":id,"updatedAt":timestamp,"source":"vscode","parentThreadId":ancestor});
            library
                .import(&profile, id, &source, &rollout, &metadata, false, &[])
                .unwrap();
            imports.push((id.clone(), rollout, metadata));
        }
        let plan = preview(&data, ChatScope::OlderThan30Days).unwrap();
        assert_eq!(plan.count, 1);
        assert_eq!(plan.protected_count, 2);
        let result = clear(&data, &plan.token).unwrap();
        assert_eq!(result.deleted_count, 1);
        assert!(result.reclaimed_bytes > 0);
        assert!(library.get(&parent).is_ok());
        assert!(library.get(&child).is_ok());
        assert!(library.get(&old).is_err());
        let deleted = &imports[2];
        assert!(library
            .import(
                &profile,
                &deleted.0,
                &source,
                &deleted.1,
                &deleted.2,
                false,
                &[]
            )
            .is_err());
        let stale = preview(&data, ChatScope::All).unwrap();
        let mut changed = imports[0].2.clone();
        changed["updatedAt"] = serde_json::json!(chrono::Utc::now().timestamp());
        library
            .import(
                &profile,
                &parent,
                &source,
                &imports[0].1,
                &changed,
                false,
                &[],
            )
            .unwrap();
        assert!(clear(&data, &stale.token).is_err());
        assert!(imports.iter().all(|(_, path, _)| path.exists()));
    }
}
