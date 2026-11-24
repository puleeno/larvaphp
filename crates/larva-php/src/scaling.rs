//! Auto-scaling of PHP threads.
//!
//! Mirrors `scaling.go` from the Go source.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::errors::{Error, Result};
use crate::thread::{PhpThread, ThreadState};
use crate::worker::Worker;

/// Minimum time a request must be stalled before scaling.
#[allow(dead_code)]
const MIN_STALL_TIME: Duration = Duration::from_millis(5);

/// Time to check CPU usage before scaling.
#[allow(dead_code)]
const CPU_PROBE_TIME: Duration = Duration::from_millis(120);

/// Maximum CPU usage for scaling (0.0 - 1.0).
#[allow(dead_code)]
const MAX_CPU_USAGE_FOR_SCALING: f64 = 0.8;

/// Downscale check interval.
#[allow(dead_code)]
const DOWN_SCALE_CHECK_TIME: Duration = Duration::from_secs(5);

/// Max threads to stop in one downscale iteration.
const MAX_TERMINATION_COUNT: usize = 10;

/// Default max idle time for auto-scaled threads.
const DEFAULT_MAX_IDLE_TIME: Duration = Duration::from_secs(5);

/// Auto-scaling manager.
pub struct ScalingManager {
    auto_scaled_threads: RwLock<Vec<Arc<PhpThread>>>,
    max_idle_time: AtomicI64,
    is_running: AtomicBool,
}

impl ScalingManager {
    pub fn new() -> Self {
        ScalingManager {
            auto_scaled_threads: RwLock::new(Vec::new()),
            max_idle_time: AtomicI64::new(DEFAULT_MAX_IDLE_TIME.as_millis() as i64),
            is_running: AtomicBool::new(false),
        }
    }

    /// Initialize auto-scaling.
    pub fn init(&self, max_threads: i32, num_threads: i32) {
        if max_threads <= num_threads {
            return;
        }
        self.is_running.store(true, Ordering::Release);
        // TODO: spawn upscaling/downscaling tasks
    }

    /// Add a regular thread.
    pub fn add_regular_thread(&self) -> Result<Arc<PhpThread>> {
        let thread = self.get_inactive_thread()?;
        // TODO: convert to regular thread
        Ok(thread)
    }

    /// Add a worker thread.
    pub fn add_worker_thread(&self, worker: &Arc<Worker>) -> Result<Arc<PhpThread>> {
        if worker.is_at_thread_limit() {
            return Err(Error::MaxThreadsReached);
        }
        let thread = self.get_inactive_thread()?;
        // TODO: convert to worker thread
        Ok(thread)
    }

    fn get_inactive_thread(&self) -> Result<Arc<PhpThread>> {
        // TODO: find or create an inactive thread
        Err(Error::MaxThreadsReached)
    }

    /// Deactivate idle threads.
    pub fn deactivate_threads(&self) {
        let mut threads = self.auto_scaled_threads.write().unwrap();
        let mut stopped = 0;
        threads.retain(|thread| {
            if stopped >= MAX_TERMINATION_COUNT {
                return true;
            }
            let wait_time = thread.state.wait_time();
            if wait_time > 0 && thread.state.is(ThreadState::Ready) {
                let max_idle = self.max_idle_time.load(Ordering::Acquire);
                if wait_time > max_idle {
                    stopped += 1;
                    return false;
                }
            }
            true
        });
    }

    /// Track an auto-scaled thread.
    pub fn track_auto_scaled_thread(&self, thread: Arc<PhpThread>) {
        let mut threads = self.auto_scaled_threads.write().unwrap();
        if !threads.iter().any(|t| Arc::ptr_eq(t, &thread)) {
            threads.push(thread);
        }
    }
}

impl Default for ScalingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global scaling manager.
pub static SCALING: once_cell::sync::Lazy<RwLock<ScalingManager>> =
    once_cell::sync::Lazy::new(|| RwLock::new(ScalingManager::new()));

/// Probe CPU usage to decide if scaling is worthwhile.
#[cfg(unix)]
pub fn probe_cpus(probe_time: Duration, _max_cpu_usage: f64) -> bool {
    use std::time::Instant;
    let start = Instant::now();

    // TODO: use clock_gettime for accurate CPU measurement
    std::thread::sleep(probe_time);

    let _elapsed = start.elapsed();
    // Simplified: assume CPU is not busy
    true
}

#[cfg(windows)]
pub fn probe_cpus(probe_time: Duration, _max_cpu_usage: f64) -> bool {
    std::thread::sleep(probe_time);
    true
}