use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

/// Watches for process exits using OS-native mechanisms.
///
/// macOS: kqueue EVFILT_PROC + NOTE_EXIT — fires the moment a watched
///        PID calls exit(), giving near-instant port-stopped detection.
/// Other: no-op — scanner falls back to polling on the configured interval.
pub struct ProcessWatcher {
    notify: Arc<Notify>,
    #[cfg(target_os = "macos")]
    pid_tx: std::sync::mpsc::SyncSender<Vec<u32>>,
}

impl ProcessWatcher {
    pub fn spawn() -> Self {
        let notify = Arc::new(Notify::new());

        #[cfg(target_os = "macos")]
        {
            let (pid_tx, pid_rx) = std::sync::mpsc::sync_channel::<Vec<u32>>(1);
            let notify_clone = notify.clone();
            std::thread::Builder::new()
                .name("kqueue-watcher".into())
                .spawn(move || kqueue_thread(pid_rx, notify_clone))
                .expect("failed to spawn kqueue thread");
            Self { notify, pid_tx }
        }

        #[cfg(not(target_os = "macos"))]
        Self { notify }
    }

    /// After each scan, pass the current set of live PIDs so kqueue can
    /// watch them for exits. Non-blocking: drops the update if the channel
    /// is already full (the next scan will update).
    pub fn watch_pids(&self, pids: Vec<u32>) {
        #[cfg(target_os = "macos")]
        let _ = self.pid_tx.try_send(pids);
        #[cfg(not(target_os = "macos"))]
        let _ = pids;
    }

    /// Wait until a process exits (kqueue event) OR `timeout` elapses.
    pub async fn wait_for_event(&self, timeout: Duration) {
        tokio::select! {
            _ = self.notify.notified() => {}
            _ = tokio::time::sleep(timeout) => {}
        }
    }
}

// ─── macOS kqueue implementation ─────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn kqueue_thread(pid_rx: std::sync::mpsc::Receiver<Vec<u32>>, notify: Arc<Notify>) {
    use std::collections::HashSet;

    let kq = unsafe { libc::kqueue() };
    if kq < 0 {
        tracing::warn!("kqueue() failed — process-exit watcher disabled, falling back to polling");
        return;
    }

    let mut watched: HashSet<u32> = HashSet::new();

    loop {
        // Non-blocking check for a new PID list from the scanner.
        match pid_rx.try_recv() {
            Ok(new_pids) => {
                let new_set: HashSet<u32> = new_pids.into_iter().collect();

                // Register any newly appeared PIDs.
                for &pid in new_set.difference(&watched) {
                    let ev = libc::kevent {
                        ident: pid as libc::uintptr_t,
                        filter: libc::EVFILT_PROC,
                        flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_ONESHOT,
                        fflags: libc::NOTE_EXIT,
                        data: 0,
                        udata: std::ptr::null_mut(),
                    };
                    // Ignore errors: PID may have already exited (ESRCH).
                    unsafe {
                        libc::kevent(
                            kq,
                            &ev as *const libc::kevent,
                            1,
                            std::ptr::null_mut(),
                            0,
                            std::ptr::null(),
                        );
                    }
                }

                watched = new_set;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                unsafe { libc::close(kq) };
                return;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }

        // Block for up to 200 ms waiting for a process-exit event.
        let ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 200_000_000,
        };
        let mut out_ev = std::mem::MaybeUninit::<libc::kevent>::uninit();
        let n = unsafe { libc::kevent(kq, std::ptr::null(), 0, out_ev.as_mut_ptr(), 1, &ts) };

        if n > 0 {
            let event = unsafe { out_ev.assume_init() };
            watched.remove(&(event.ident as u32));
            // Wake the async scanner — a process just died.
            notify.notify_one();
        }
    }
}
