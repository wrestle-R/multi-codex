//! Serialized launch/placement transactions. Failed placement keeps a retry token,
//! so retrying cannot launch another VS Code window.
use crate::desktop_environment::{self, DesktopInventory};
use crate::profiles::Result;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub completed: bool,
    pub error: Option<String>,
    pub retry_token: Option<String>,
}

struct PendingLaunch {
    profile: String,
    workspace: String,
    profile_home: PathBuf,
    backend: String,
    previous: HashSet<String>,
    window: Option<String>,
    created: Instant,
}

pub struct LaunchRequest<'a> {
    pub profile: &'a str,
    pub workspace: &'a str,
    pub home: &'a Path,
    pub desktop: Option<&'a str>,
    pub retry: Option<&'a str>,
}

#[derive(Default)]
pub struct LaunchCoordinator {
    pending: HashMap<String, PendingLaunch>,
}

pub trait DesktopControl {
    fn inventory(&self) -> Result<DesktopInventory>;
    fn find_window(&self, previous: &HashSet<String>, home: &Path) -> Result<String>;
    fn move_window(&self, window: &str, desktop: &str) -> Result<()>;
    fn validate_window(&self, window: &str, home: &Path) -> Result<()>;
}
pub struct NativeDesktopControl;
impl DesktopControl for NativeDesktopControl {
    fn validate_window(&self, window: &str, home: &Path) -> Result<()> {
        let snapshot = desktop_environment::inventory()?;
        if snapshot.windows().iter().any(|candidate| {
            candidate.id == window
                && crate::profiles::window_process_uses_profile(candidate.pid, home)
        }) {
            Ok(())
        } else {
            Err("The already-opened profile window is no longer available. It will not be relaunched by a placement retry.".into())
        }
    }
    fn inventory(&self) -> Result<DesktopInventory> {
        desktop_environment::inventory()
    }
    fn find_window(&self, previous: &HashSet<String>, home: &Path) -> Result<String> {
        desktop_environment::find_new_window(previous, home)
    }
    fn move_window(&self, window: &str, desktop: &str) -> Result<()> {
        desktop_environment::move_window(window, desktop)
    }
}

impl LaunchCoordinator {
    pub fn execute_serialized(
        coordinator: &Mutex<Self>,
        request: LaunchRequest<'_>,
        launch: impl FnOnce() -> Result<()>,
        control: &impl DesktopControl,
    ) -> Result<LaunchResult> {
        coordinator
            .lock()
            .map_err(|_| "Launch coordinator unavailable")?
            .execute(request, launch, control)
    }
    pub fn discard(&mut self, token: &str) {
        self.pending.remove(token);
    }

    pub fn execute(
        &mut self,
        request: LaunchRequest<'_>,
        launch: impl FnOnce() -> Result<()>,
        control: &impl DesktopControl,
    ) -> Result<LaunchResult> {
        let LaunchRequest {
            profile,
            workspace,
            home,
            desktop,
            retry,
        } = request;
        // "Open here" must also wait for the new window and place it onto the
        // launcher's desktop. An existing profile can otherwise reopen elsewhere.
        // Accepting a failed placement with a retry token still means keep it as-is.
        let current_destination = if desktop.is_none() && retry.is_none() {
            control
                .inventory()
                .ok()
                .filter(|snapshot| snapshot.capabilities.move_windows)
                .and_then(|snapshot| {
                    snapshot
                        .current_destination(std::process::id())
                        .map(str::to_owned)
                })
        } else {
            None
        };
        let desktop = desktop.or(current_destination.as_deref());
        self.pending
            .retain(|_, p| p.created.elapsed() < Duration::from_secs(1800));
        let token = if let Some(token) = retry {
            let pending = self.pending.get(token).ok_or("This placement retry expired. Check the already-opened VS Code window before launching again.")?;
            if pending.profile != profile
                || pending.workspace != workspace
                || pending.profile_home != home
            {
                return Err("Placement retry does not belong to this launch".into());
            }
            token.to_string()
        } else {
            if self.pending.len() >= 64 {
                return Err("Too many unfinished placements. Close existing launch dialogs before launching again.".into());
            }
            let snapshot = if let Some(destination) = desktop {
                let snapshot = control.inventory()?;
                snapshot.require_destination(destination)?;
                Some(snapshot)
            } else {
                None
            };
            // Only record a pending placement after launch succeeds. Enumeration failures
            // above must never be swallowed or mistaken for an empty window list.
            launch()?;
            if desktop.is_none() {
                return Ok(complete());
            }
            let snapshot = snapshot.ok_or("Desktop snapshot unavailable")?;
            let token = uuid::Uuid::new_v4().to_string();
            self.pending.insert(
                token.clone(),
                PendingLaunch {
                    profile: profile.into(),
                    workspace: workspace.into(),
                    profile_home: home.into(),
                    backend: snapshot.capabilities.backend.clone(),
                    previous: snapshot.windows().into_iter().map(|w| w.id).collect(),
                    window: None,
                    created: Instant::now(),
                },
            );
            token
        };
        if let Some(destination) = desktop {
            let pending = self
                .pending
                .get_mut(&token)
                .ok_or("Placement state unavailable")?;
            let result = (|| {
                let snapshot = control.inventory()?;
                if snapshot.capabilities.backend != pending.backend {
                    return Err("The desktop session changed. The already-opened window cannot be moved by this session.".into());
                }
                snapshot.require_destination(destination)?;
                let window = match &pending.window {
                    Some(window) => window.clone(),
                    None => {
                        let window =
                            control.find_window(&pending.previous, &pending.profile_home)?;
                        pending.window = Some(window.clone());
                        window
                    }
                };
                control.validate_window(&window, &pending.profile_home)?;
                control.move_window(&window, destination)
            })();
            if let Err(error) = result {
                return Ok(LaunchResult {
                    completed: false,
                    error: Some(error),
                    retry_token: Some(token),
                });
            }
        }
        self.pending.remove(&token);
        Ok(complete())
    }
}
fn complete() -> LaunchResult {
    LaunchResult {
        completed: true,
        error: None,
        retry_token: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop_environment::{Desktop, DesktopCapabilities};
    use std::cell::Cell;
    struct Fake {
        fail: Cell<bool>,
        valid: Cell<bool>,
    }
    impl DesktopControl for Fake {
        fn validate_window(&self, _: &str, _: &Path) -> Result<()> {
            if self.valid.get() {
                Ok(())
            } else {
                Err("The window was closed".into())
            }
        }
        fn inventory(&self) -> Result<DesktopInventory> {
            Ok(DesktopInventory {
                protocol_version: 1,
                capabilities: DesktopCapabilities {
                    backend: "fake".into(),
                    enumerate_desktops: true,
                    enumerate_windows: true,
                    move_windows: true,
                    reason: None,
                },
                desktops: vec![Desktop {
                    id: "fake:12".into(),
                    name: "Work".into(),
                    monitor: None,
                    current: false,
                    windows: vec![],
                }],
            })
        }
        fn find_window(&self, _: &HashSet<String>, _: &Path) -> Result<String> {
            Ok("new-window".into())
        }
        fn move_window(&self, _: &str, _: &str) -> Result<()> {
            if self.fail.get() {
                Err("Permission denied".into())
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn current_desktop_waits_for_identification_and_verified_placement() {
        struct Current {
            control: Fake,
            moves: Cell<usize>,
        }
        impl DesktopControl for Current {
            fn inventory(&self) -> Result<DesktopInventory> {
                let mut snapshot = self.control.inventory()?;
                snapshot.desktops[0].current = true;
                Ok(snapshot)
            }
            fn find_window(&self, previous: &HashSet<String>, home: &Path) -> Result<String> {
                self.control.find_window(previous, home)
            }
            fn validate_window(&self, window: &str, home: &Path) -> Result<()> {
                self.control.validate_window(window, home)
            }
            fn move_window(&self, window: &str, desktop: &str) -> Result<()> {
                assert_eq!(window, "new-window");
                assert_eq!(desktop, "fake:12");
                self.moves.set(self.moves.get() + 1);
                self.control.move_window(window, desktop)
            }
        }
        let control = Current {
            control: Fake {
                fail: Cell::new(false),
                valid: Cell::new(true),
            },
            moves: Cell::new(0),
        };
        let mut coordinator = LaunchCoordinator::default();
        let result = coordinator
            .execute(
                LaunchRequest {
                    profile: "profile",
                    workspace: "/workspace",
                    home: Path::new("/profile"),
                    desktop: None,
                    retry: None,
                },
                || Ok(()),
                &control,
            )
            .unwrap();
        assert!(result.completed);
        assert_eq!(control.moves.get(), 1);
    }

    #[test]
    fn concurrent_launches_use_the_production_serialization_entrypoint() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Barrier,
        };
        let coordinator = Arc::new(Mutex::new(LaunchCoordinator::default()));
        let barrier = Arc::new(Barrier::new(2));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let (coordinator, barrier, active, maximum) = (
                Arc::clone(&coordinator),
                Arc::clone(&barrier),
                Arc::clone(&active),
                Arc::clone(&maximum),
            );
            threads.push(std::thread::spawn(move || {
                let control = Fake {
                    fail: Cell::new(false),
                    valid: Cell::new(true),
                };
                barrier.wait();
                LaunchCoordinator::execute_serialized(
                    &coordinator,
                    LaunchRequest {
                        profile: "p",
                        workspace: "/project",
                        home: Path::new("/profile"),
                        desktop: None,
                        retry: None,
                    },
                    || {
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        maximum.fetch_max(current, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(30));
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    },
                    &control,
                )
                .unwrap()
            }));
        }
        for thread in threads {
            assert!(thread.join().unwrap().completed);
        }
        assert_eq!(maximum.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn retry_uses_existing_window_without_relaunching() {
        let mut coordinator = LaunchCoordinator::default();
        let control = Fake {
            fail: Cell::new(true),
            valid: Cell::new(true),
        };
        let starts = Cell::new(0);
        let first = coordinator
            .execute(
                LaunchRequest {
                    profile: "p",
                    workspace: "/project",
                    home: Path::new("/profile"),
                    desktop: Some("fake:12"),
                    retry: None,
                },
                || {
                    starts.set(starts.get() + 1);
                    Ok(())
                },
                &control,
            )
            .unwrap();
        assert!(!first.completed);
        assert_eq!(first.error.as_deref(), Some("Permission denied"));
        control.valid.set(false);
        let unavailable = coordinator
            .execute(
                LaunchRequest {
                    profile: "p",
                    workspace: "/project",
                    home: Path::new("/profile"),
                    desktop: Some("fake:12"),
                    retry: first.retry_token.as_deref(),
                },
                || panic!("must not relaunch"),
                &control,
            )
            .unwrap();
        assert_eq!(unavailable.error.as_deref(), Some("The window was closed"));
        control.valid.set(true);
        control.fail.set(false);
        let next = coordinator
            .execute(
                LaunchRequest {
                    profile: "p",
                    workspace: "/project",
                    home: Path::new("/profile"),
                    desktop: Some("fake:12"),
                    retry: first.retry_token.as_deref(),
                },
                || panic!("must not relaunch"),
                &control,
            )
            .unwrap();
        assert!(next.completed);
        assert_eq!(starts.get(), 1);
    }
    #[test]
    fn missing_destination_never_launches_and_wrong_retry_never_launches() {
        let mut coordinator = LaunchCoordinator::default();
        let control = Fake {
            fail: Cell::new(true),
            valid: Cell::new(true),
        };
        assert!(coordinator
            .execute(
                LaunchRequest {
                    profile: "p",
                    workspace: "/project",
                    home: Path::new("/profile"),
                    desktop: Some("fake:13"),
                    retry: None
                },
                || panic!("must not launch"),
                &control
            )
            .is_err());
        assert!(coordinator
            .execute(
                LaunchRequest {
                    profile: "p",
                    workspace: "/project",
                    home: Path::new("/profile"),
                    desktop: None,
                    retry: Some("unknown")
                },
                || panic!("must not launch"),
                &control
            )
            .is_err());
    }
    #[test]
    fn accepting_already_opened_window_does_not_launch_again() {
        let mut coordinator = LaunchCoordinator::default();
        let control = Fake {
            fail: Cell::new(true),
            valid: Cell::new(true),
        };
        let first = coordinator
            .execute(
                LaunchRequest {
                    profile: "p",
                    workspace: "/project",
                    home: Path::new("/profile"),
                    desktop: Some("fake:12"),
                    retry: None,
                },
                || Ok(()),
                &control,
            )
            .unwrap();
        assert!(
            coordinator
                .execute(
                    LaunchRequest {
                        profile: "p",
                        workspace: "/project",
                        home: Path::new("/profile"),
                        desktop: None,
                        retry: first.retry_token.as_deref()
                    },
                    || panic!("must not launch"),
                    &control
                )
                .unwrap()
                .completed
        );
    }
}
