/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Host scheduling for Apple silicon (Game Mode).
//!
//! The emulator multiplexes every guest thread onto one host thread and
//! paces frames with sleeps. Two host details make that pacing uneven on a
//! Mac:
//!
//! * The host thread is created without a QoS class, so the scheduler is free
//!   to put it on an efficiency core; a frame that lands there is late.
//! * `nanosleep` wakes up late by a variable amount (tens of microseconds to
//!   over a millisecond when timers are coalesced). At 60 Hz that is enough to
//!   miss a frame's deadline now and then, which shows up as stutter.
//!
//! With Game Mode on (the default; `PEAR_GAME_MODE=0` turns it off) the main
//! thread runs at user-interactive QoS and sleeps end with a short spin so a
//! frame is released on time. PearSilicon also declares the host bundle as a
//! game, so macOS Game Mode engages in full screen.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Whether Game Mode tuning is active for this process.
pub fn game_mode() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        !matches!(
            std::env::var("PEAR_GAME_MODE").as_deref(),
            Ok("0") | Ok("off") | Ok("false")
        )
    })
}

#[cfg(target_os = "macos")]
extern "C" {
    fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
    fn dup2(from: i32, to: i32) -> i32;
}

/// PearSilicon starts the engine through LaunchServices (so macOS sees a real
/// app and can enable Game Mode), which leaves no pipe for its output. With
/// `PEAR_LEGACY_LOG_FILE` set, stdout and stderr are appended to that file.
/// Must run before anything is printed.
pub fn redirect_output_if_requested() {
    let Some(path) = std::env::var_os("PEAR_LEGACY_LOG_FILE").filter(|p| !p.is_empty()) else {
        return;
    };
    let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        let fd = file.as_raw_fd();
        // Safety: duplicating a valid descriptor onto the standard streams.
        unsafe {
            dup2(fd, 1);
            dup2(fd, 2);
        }
    }
    drop(file);
}

/// Call once from the emulator thread before the guest starts.
pub fn configure_current_thread() {
    if !game_mode() {
        log!("Game Mode tuning is off (PEAR_GAME_MODE=0).");
        return;
    }
    #[cfg(target_os = "macos")]
    {
        const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;
        // Safety: plain libSystem call on the current thread.
        let result = unsafe { pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0) };
        log!(
            "Game Mode: emulator thread QoS user-interactive ({}), precise frame pacing on.",
            if result == 0 { "ok" } else { "refused" }
        );
    }
}

/// How much of a sleep is spent spinning instead of sleeping. Large enough to
/// absorb a typical late wake-up on macOS, small enough to cost little CPU.
const SPIN_MARGIN: Duration = Duration::from_micros(1500);

/// Sleeps for `duration`, waking on time. Without Game Mode this is a plain
/// `std::thread::sleep`.
pub fn sleep(duration: Duration) {
    if duration.is_zero() {
        return;
    }
    if !game_mode() {
        std::thread::sleep(duration);
        return;
    }
    let deadline = Instant::now() + duration;
    if duration > SPIN_MARGIN {
        std::thread::sleep(duration - SPIN_MARGIN);
    }
    while Instant::now() < deadline {
        std::hint::spin_loop();
    }
}
