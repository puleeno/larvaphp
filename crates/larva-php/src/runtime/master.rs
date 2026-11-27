//! Master-side worker pool.
//!
//! Spawns N worker child processes (re-exec of the current binary with
//! `--worker`), each with its own shared-memory segment. Dispatches requests
//! round-robin and reads responses from shared memory.

use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::runtime::executor::{Request, Response};
use crate::runtime::ipc::{state, SharedMemory};

/// One worker: its process, shared memory, and a busy flag.
pub struct WorkerHandle {
    pub id: usize,
    pub shm: Mutex<SharedMemory>,
    pub child: Child,
    pub busy: AtomicBool,
}

/// A pool of worker processes.
pub struct WorkerPool {
    workers: Vec<WorkerHandle>,
    next: AtomicUsize,
    shm_dir: PathBuf,
}

impl WorkerPool {
    /// Spawn `n` workers.
    pub fn spawn(n: usize) -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {}", e))?;
        let shm_dir = std::env::temp_dir().join(format!("larva-runtime-{}", std::process::id()));
        std::fs::create_dir_all(&shm_dir).map_err(|e| format!("create shm dir: {}", e))?;

        let mut workers = Vec::with_capacity(n);
        for i in 0..n {
            let shm_path = shm_dir.join(format!("worker-{}.shm", i));
            let shm = SharedMemory::create(&shm_path)
                .map_err(|e| format!("create shm {}: {}", shm_path.display(), e))?;

            let child = Command::new(&exe)
                .arg("--worker")
                .arg("--id")
                .arg(i.to_string())
                .arg("--shm")
                .arg(&shm_path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("spawn worker {}: {}", i, e))?;

            tracing::info!("worker {} spawned: pid={} shm={}", i, child.id(), shm_path.display());

            workers.push(WorkerHandle {
                id: i,
                shm: Mutex::new(shm),
                child,
                busy: AtomicBool::new(false),
            });
        }

        tracing::info!("worker pool online: {} workers", workers.len());

        Ok(Self {
            workers,
            next: AtomicUsize::new(0),
            shm_dir,
        })
    }

    /// Return status snapshot of all workers.
    pub fn status(&self) -> Vec<(usize, Option<u32>, bool)> {
        self.workers.iter().map(|w| (w.id, Some(w.child.id()), w.busy.load(std::sync::atomic::Ordering::Relaxed))).collect()
    }

    /// Dispatch a request to an idle worker and await its response.
    ///
    /// Round-robin alone queues two requests onto the same worker under load,
    /// doubling tail latency. Claiming an idle worker keeps the N PHP processes
    /// genuinely parallel; we only fall back to round-robin when all are busy.
    pub fn dispatch(&self, req: &Request) -> Result<Response, String> {
        let n = self.workers.len();
        if n == 0 {
            return Err("no workers".to_string());
        }

        let start = self.next.fetch_add(1, Ordering::Relaxed) % n;
        let mut claimed = None;
        for offset in 0..n {
            let idx = (start + offset) % n;
            let candidate = &self.workers[idx];
            if candidate.busy.swap(true, Ordering::AcqRel) {
                continue;
            }
            let idle = candidate
                .shm
                .lock()
                .map(|shm| shm.state() == state::IDLE)
                .unwrap_or(false);
            if idle {
                claimed = Some(candidate);
                break;
            }
            candidate.busy.store(false, Ordering::Release);
        }
        // Every worker is busy (or recovering from an error): queue on the
        // next round-robin slot instead of failing the request.
        let w = match claimed {
            Some(w) => w,
            None => {
                let w = &self.workers[start];
                w.busy.store(true, Ordering::Release);
                w
            }
        };

        let req_bytes = serde_json::to_vec(req).map_err(|e| {
            w.busy.store(false, Ordering::Release);
            format!("encode request: {}", e)
        })?;
        {
            let mut shm = w.shm.lock().unwrap();
            if let Err(e) = shm.write_request(&req_bytes) {
                w.busy.store(false, Ordering::Release);
                return Err(e);
            }
        }

        // Wait for the worker to produce a response (poll with a short sleep).
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let resp_bytes = {
                let shm = w.shm.lock().unwrap();
                if shm.state() == state::RESP_READY {
                    shm.read_response()
                } else if shm.state() == state::ERROR {
                    // The worker is waiting for the master to reset the slot.
                    shm.set_state(state::IDLE);
                    w.busy.store(false, Ordering::Release);
                    return Err("worker reported an error".to_string());
                } else {
                    None
                }
            };
            if let Some(bytes) = resp_bytes {
                let resp = serde_json::from_slice::<Response>(&bytes);
                {
                    let shm = w.shm.lock().unwrap();
                    shm.set_state(state::IDLE);
                }
                w.busy.store(false, Ordering::Release);
                return resp.map_err(|e| format!("decode response: {}", e));
            }
            if Instant::now() > deadline {
                // Leave the slot busy: the worker may still be executing PHP.
                return Err(format!("worker {} timed out", w.id));
            }
            std::thread::sleep(Duration::from_micros(100));
        }
    }

    /// Shut down all workers.
    pub fn shutdown(&mut self) {
        for w in &mut self.workers {
            let _ = w.child.kill();
        }
        // Best-effort cleanup of the shm directory.
        let _ = std::fs::remove_dir_all(&self.shm_dir);
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.shutdown();
    }
}