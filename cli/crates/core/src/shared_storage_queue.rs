//! Update-time setup policy. Active accounts remain queued and are never stopped.
use crate::{
    managed_files as files,
    profiles::{self, Result},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Queue {
    pub schema: u32,
    pub enabled: bool,
    pub profiles: BTreeMap<String, Status>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Status {
    pub state: String,
    pub reason: Option<String>,
    pub version: String,
}
pub fn load(data: &Path) -> Result<Queue> {
    let path = data.join("migrations/sharing-queue.json");
    files::directory(data, &data.join("migrations"))?;
    if !path.exists() {
        return Ok(Queue {
            schema: 1,
            enabled: true,
            ..Queue::default()
        });
    }
    let queue: Queue = serde_json::from_slice(
        &fs::read(files::regular(data, &path)?).map_err(|_| "Cannot read storage queue")?,
    )
    .map_err(|_| "Invalid storage migration queue")?;
    if queue.schema != 1 {
        return Err("Unsupported storage queue schema".into());
    }
    Ok(queue)
}
pub fn save(data: &Path, queue: &Queue) -> Result<()> {
    profiles::write_private_file(
        &data.join("migrations/sharing-queue.json"),
        &serde_json::to_vec_pretty(queue).map_err(|_| "Cannot encode storage queue")?,
    )
}
