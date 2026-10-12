//! Thread writer fencing: helper death never releases a surviving backend's ownership.
use crate::managed_files as files;
use crate::profiles::{self, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriterOwner {
    pub schema: u32,
    pub thread_id: String,
    pub instance_id: String,
    pub backend_pid: u32,
    pub backend_start_time: u64,
    pub session_home: PathBuf,
    pub engine: PathBuf,
    #[serde(default)]
    pub profile_id: Option<String>,
}

pub struct WriterLease {
    _lock: File,
    pub owner: WriterOwner,
}
pub struct ThreadLeases {
    pub(crate) data_root: PathBuf,
    root: PathBuf,
    pub(crate) engine: PathBuf,
}

fn id(value: &str) -> Result<()> {
    if !Uuid::parse_str(value).is_ok_and(|uuid| uuid.to_string() == value) {
        return Err("Invalid shared thread or writer identifier".into());
    }
    Ok(())
}

fn process_exists(pid: u32) -> Result<bool> {
    #[cfg(unix)]
    {
        let pid = i32::try_from(pid).map_err(|_| "Invalid backend PID")?;
        if unsafe { libc::kill(pid, 0) } == 0 {
            return Ok(true);
        }
        if std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            return Ok(false);
        }
        Err("Native backend exit could not be verified; ownership is retained".into())
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, GetLastError, ERROR_INVALID_PARAMETER},
            System::Threading::{
                GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        };
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return if GetLastError() == ERROR_INVALID_PARAMETER {
                    Ok(false)
                } else {
                    Err("Native backend exit could not be verified; ownership is retained".into())
                };
            }
            let mut code = 0;
            let success = GetExitCodeProcess(handle, &mut code);
            CloseHandle(handle);
            if success == 0 {
                return Err(
                    "Native backend exit could not be verified; ownership is retained".into(),
                );
            }
            Ok(code == 259)
        }
    }
}

fn process_system(pid: u32) -> System {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_environ(UpdateKind::Always),
    );
    system
}

fn process_start(pid: u32, _process: &sysinfo::Process) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        stat.rsplit_once(") ")?
            .1
            .split_whitespace()
            .nth(19)?
            .parse()
            .ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        Some(_process.start_time())
    }
}

fn matches_process(owner: &WriterOwner) -> bool {
    let system = process_system(owner.backend_pid);
    let Some(process) = system.process(Pid::from_u32(owner.backend_pid)) else {
        return false;
    };
    let expected = format!("CODEX_HOME={}", owner.session_home.display());
    process_start(owner.backend_pid, process) == Some(owner.backend_start_time)
        && process
            .exe()
            .and_then(|path| fs::canonicalize(path).ok())
            .as_ref()
            == Some(&owner.engine)
        && process
            .environ()
            .iter()
            .any(|value| value == expected.as_str())
}

impl ThreadLeases {
    pub(crate) fn profile_has_live_writer(data_root: &Path, profile_id: &str) -> Result<bool> {
        id(profile_id)?;
        let directory = data_root.join("shared-chats/owners");
        if !directory.exists() {
            return Ok(false);
        }
        files::checked(data_root, &directory, false)?;
        for entry in
            fs::read_dir(directory).map_err(|_| "Cannot inspect private writer ownership")?
        {
            let path = entry.map_err(|_| "Cannot inspect private writer")?.path();
            let filename = path.file_stem().and_then(|name| name.to_str());
            if path.extension().is_none_or(|extension| extension != "json")
                || !filename.is_some_and(|name| {
                    Uuid::parse_str(name).is_ok_and(|id| id.to_string() == name)
                })
            {
                continue;
            }
            if !path.exists() {
                continue;
            }
            let owner: WriterOwner = serde_json::from_slice(
                &fs::read(files::regular(data_root, &path)?)
                    .map_err(|_| "Cannot read writer identity")?,
            )
            .map_err(|_| "Writer recovery is required before changing profile data")?;
            if (owner.profile_id.as_deref() == Some(profile_id)
                || owner
                    .session_home
                    .starts_with(crate::managed_launch::runtime_home(data_root, profile_id)))
                && process_exists(owner.backend_pid)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(crate) fn unowned_guard(data_root: &Path, thread_id: &str) -> Result<File> {
        id(thread_id)?;
        let directory = data_root.join("shared-chats/leases");
        files::directory(data_root, &directory)?;
        let path = files::checked(
            data_root,
            &directory.join(format!("{thread_id}.lock")),
            true,
        )?;
        let file = crate::file_security::lock_file(&path)?;
        fs2::FileExt::try_lock_exclusive(&file)
            .map_err(|_| "A writer owns this chat; defer source import until handoff")?;
        let owner = files::checked(
            data_root,
            &data_root
                .join("shared-chats/owners")
                .join(format!("{thread_id}.json")),
            true,
        )?;
        if owner.exists() {
            return Err("A previous writer needs checkpoint recovery before source import".into());
        }
        Ok(file)
    }
    pub(crate) fn read_home(&self) -> Result<tempfile::TempDir> {
        let root = self.root.join("readers");
        files::directory(&self.data_root, &root)?;
        let home = tempfile::tempdir_in(root).map_err(|_| "Cannot allocate private reader home")?;
        crate::profiles::set_owner_only_dir(home.path())?;
        crate::profiles::write_codex_config(home.path())?;
        Ok(home)
    }
    pub fn open(data_root: &Path, engine: &Path) -> Result<Self> {
        files::root(data_root)?;
        let engine = fs::canonicalize(engine).map_err(|_| "Native chat engine is unavailable")?;
        let root = data_root.join("shared-chats");
        files::directory(data_root, &root.join("leases"))?;
        files::directory(data_root, &root.join("owners"))?;
        Ok(Self {
            data_root: data_root.into(),
            root,
            engine,
        })
    }

    fn owner_path(&self, thread_id: &str) -> Result<PathBuf> {
        id(thread_id)?;
        files::checked(
            &self.data_root,
            &self.root.join("owners").join(format!("{thread_id}.json")),
            true,
        )
    }

    fn read_owner(&self, thread_id: &str) -> Result<Option<WriterOwner>> {
        let path = self.owner_path(thread_id)?;
        if !path.exists() {
            return Ok(None);
        }
        let owner: WriterOwner = serde_json::from_slice(
            &fs::read(files::regular(&self.data_root, &path)?)
                .map_err(|_| "Cannot inspect previous chat writer")?,
        )
        .map_err(|_| "Previous chat ownership could not be verified; recovery is required")?;
        if owner.schema != 1 || owner.thread_id != thread_id || !owner.engine.is_absolute() {
            return Err("Previous chat ownership does not match this thread or schema; recovery is required".into());
        }
        id(&owner.instance_id)?;
        files::checked(&self.data_root, &owner.session_home, false)?;
        Ok(Some(owner))
    }

    pub fn acquire(
        &self,
        thread_id: &str,
        instance_id: &str,
        backend_pid: u32,
        session_home: &Path,
    ) -> Result<WriterLease> {
        id(thread_id)?;
        id(instance_id)?;
        if backend_pid == 0 {
            return Err("Native backend identity is missing".into());
        }
        let home = files::checked(&self.data_root, session_home, false)?;
        files::root(&home)?;
        let lock_path = files::checked(
            &self.data_root,
            &self.root.join("leases").join(format!("{thread_id}.lock")),
            true,
        )?;
        let lock = crate::file_security::lock_file(&lock_path)?;
        fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| {
            "This shared chat is being used by another window. Read it or wait for a handoff"
        })?;
        if let Some(previous) = self.read_owner(thread_id)? {
            // Do not infer writer death from a helper/socket lock disappearing.
            if matches_process(&previous) {
                return Err("The previous native backend still owns this chat. Stop it safely before handing off".into());
            }
            // Even incomplete process information must not permit takeover of a live PID.
            if process_system(previous.backend_pid)
                .process(Pid::from_u32(previous.backend_pid))
                .is_some_and(|process| {
                    process_start(previous.backend_pid, process)
                        == Some(previous.backend_start_time)
                })
            {
                return Err(
                    "The previous backend has not been proven stopped; chat handoff is blocked"
                        .into(),
                );
            }
            return Err("The stopped writer has an unfinished checkpoint. Recover its retained private history before taking over this chat".into());
        }
        let system = process_system(backend_pid);
        let process = system
            .process(Pid::from_u32(backend_pid))
            .ok_or("Native backend is no longer running")?;
        let owner = WriterOwner {
            schema: 1,
            thread_id: thread_id.into(),
            instance_id: instance_id.into(),
            backend_pid,
            backend_start_time: process_start(backend_pid, process)
                .ok_or("Cannot verify native process start identity")?,
            session_home: home,
            engine: self.engine.clone(),
            profile_id: None,
        };
        if owner.backend_start_time == 0 || !matches_process(&owner) {
            return Err(
                "Could not verify the backend's private home and engine before acquiring this chat"
                    .into(),
            );
        }
        profiles::write_private_file(
            &self.owner_path(thread_id)?,
            &serde_json::to_vec(&owner).map_err(|_| "Could not record chat ownership")?,
        )?;
        Ok(WriterLease { _lock: lock, owner })
    }

    pub(crate) fn set_profile(&self, lease: &mut WriterLease, profile: &str) -> Result<()> {
        id(profile)?;
        self.verify_writer(lease)?;
        if lease
            .owner
            .profile_id
            .as_deref()
            .is_some_and(|owner| owner != profile)
        {
            return Err("Shared writer profile identity changed".into());
        }
        lease.owner.profile_id = Some(profile.into());
        profiles::write_private_file(
            &self.owner_path(&lease.owner.thread_id)?,
            &serde_json::to_vec(&lease.owner).map_err(|_| "Cannot encode writer identity")?,
        )
    }

    pub fn verify_writer(&self, lease: &WriterLease) -> Result<()> {
        if self.read_owner(&lease.owner.thread_id)?.as_ref() != Some(&lease.owner)
            || !matches_process(&lease.owner)
        {
            return Err("Shared chat writer is stale or unverified; writes are blocked".into());
        }
        Ok(())
    }

    /// Must be called after confirmed backend exit. Dropping the lease alone
    /// deliberately retains the owner record to fence a surviving orphan.
    pub fn verify_stopped(&self, lease: &WriterLease) -> Result<()> {
        let system = process_system(lease.owner.backend_pid);
        if process_exists(lease.owner.backend_pid)?
            && !system
                .process(Pid::from_u32(lease.owner.backend_pid))
                .is_some_and(|process| {
                    process_start(lease.owner.backend_pid, process)
                        .is_some_and(|start| start != 0 && start != lease.owner.backend_start_time)
                })
        {
            return Err(
                "Stop the native backend before releasing its shared chat ownership".into(),
            );
        }
        if self.read_owner(&lease.owner.thread_id)?.as_ref() != Some(&lease.owner) {
            return Err("Shared chat owner changed unexpectedly".into());
        }
        Ok(())
    }
    pub(crate) fn recover_stopped(&self, thread_id: &str) -> Result<WriterLease> {
        id(thread_id)?;
        let path = files::checked(
            &self.data_root,
            &self.root.join("leases").join(format!("{thread_id}.lock")),
            false,
        )?;
        let lock = crate::file_security::lock_file(&path)?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .map_err(|_| "The previous helper still holds this chat")?;
        let owner = self
            .read_owner(thread_id)?
            .ok_or("No stopped writer needs recovery")?;
        let lease = WriterLease { _lock: lock, owner };
        self.verify_stopped(&lease)?;
        Ok(lease)
    }
    pub(crate) fn recoverable(&self) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(self.root.join("owners"))
            .map_err(|_| "Cannot inspect retained writer histories")?
        {
            let path = entry.map_err(|_| "Cannot inspect retained writer")?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let Some(thread) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            if id(thread).is_ok() && self.recover_stopped(thread).is_ok() {
                ids.push(thread.to_owned());
            }
        }
        ids.sort();
        Ok(ids)
    }
    pub fn release(&self, lease: &WriterLease) -> Result<()> {
        self.verify_stopped(lease)?;
        fs::remove_file(self.owner_path(&lease.owner.thread_id)?)
            .map_err(|_| "Could not release stopped chat ownership")?;
        crate::file_security::sync_directory(&self.root.join("owners"))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process::{Child, Command, Stdio};
    struct Backend(Child);
    impl Drop for Backend {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn fixture() -> (tempfile::TempDir, ThreadLeases, Backend, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let app = fs::canonicalize(temp.path()).unwrap().join("app");
        let home = app.join("profiles/runtime/codex-home");
        fs::create_dir_all(&home).unwrap();
        let engine = app.join("fixture-engine");
        fs::copy("/bin/bash", &engine).unwrap();
        let child = Command::new(&engine)
            .args(["-c", "read pending"])
            .env("CODEX_HOME", &home)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let backend = Backend(child);
        let store = ThreadLeases::open(&app, &engine).unwrap();
        // Wait for exec before inspecting the engine and private environment.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !process_system(backend.0.id())
            .process(Pid::from_u32(backend.0.id()))
            .is_some_and(|p| {
                p.exe().and_then(|p| fs::canonicalize(p).ok()).as_ref() == Some(&store.engine)
            })
        {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        (temp, store, backend, home)
    }
    fn acquire_ready(
        store: &ThreadLeases,
        thread: &str,
        instance: &str,
        pid: u32,
        home: &Path,
    ) -> WriterLease {
        // Parallel tests fork short-lived subprocesses. CLOEXEC releases inherited
        // lock descriptors at exec; tolerate only that transient kernel busy state.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            match store.acquire(thread, instance, pid, home) {
                Ok(lease) => return lease,
                Err(error)
                    if error.contains("another window") && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                Err(error) => panic!("Unexpected fixture ownership failure: {error}"),
            }
        }
    }
    #[test]
    fn lease_is_exclusive_and_unrelated_chats_run_concurrently() {
        let (_temp, store, mut backend, home) = fixture();
        let thread = Uuid::new_v4().to_string();
        let instance = Uuid::new_v4().to_string();
        let first = acquire_ready(&store, &thread, &instance, backend.0.id(), &home);
        assert!(store
            .acquire(&thread, &instance, backend.0.id(), &home)
            .is_err());
        let different = store
            .acquire(
                &Uuid::new_v4().to_string(),
                &instance,
                backend.0.id(),
                &home,
            )
            .unwrap();
        store.verify_writer(&first).unwrap();
        assert!(store.release(&first).is_err());
        backend.0.kill().unwrap();
        backend.0.wait().unwrap();
        store.release(&first).unwrap();
        store.release(&different).unwrap();
    }
    #[test]
    fn dropped_helper_lock_does_not_allow_a_surviving_backend_takeover() {
        let (_temp, store, mut backend, home) = fixture();
        let thread = Uuid::new_v4().to_string();
        let instance = Uuid::new_v4().to_string();
        let first = acquire_ready(&store, &thread, &instance, backend.0.id(), &home);
        drop(first);
        let error = store
            .acquire(&thread, &Uuid::new_v4().to_string(), backend.0.id(), &home)
            .err()
            .unwrap();
        assert!(
            error.contains("previous") || error.contains("another window"),
            "{error}"
        );
        backend.0.kill().unwrap();
        backend.0.wait().unwrap();
        assert!(!matches_process(
            &store.read_owner(&thread).unwrap().unwrap()
        ));
    }
    #[test]
    fn backend_private_home_and_owner_metadata_are_verified() {
        let (_temp, store, backend, home) = fixture();
        let thread = Uuid::new_v4().to_string();
        let instance = Uuid::new_v4().to_string();
        assert!(store
            .acquire("../escape", &instance, backend.0.id(), &home)
            .is_err());
        let other = store.data_root.join("other");
        fs::create_dir(&other).unwrap();
        assert!(store
            .acquire(&thread, &instance, backend.0.id(), &other)
            .is_err());
        let lease = acquire_ready(&store, &thread, &instance, backend.0.id(), &home);
        profiles::write_private_file(&store.owner_path(&thread).unwrap(), b"{}").unwrap();
        assert!(store.verify_writer(&lease).is_err());
    }
}
