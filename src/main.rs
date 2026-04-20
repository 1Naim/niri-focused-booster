mod dbus;
mod limits;

use dbus::*;
use limits::*;
use niri_ipc::state::{EventStreamStatePart, WindowsState};
use niri_ipc::{Event, Request, socket::Socket};
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zbus::blocking::Connection;

fn main() -> Result<(), Box<dyn Error>> {
    let boosted_limits =
        parse_limits_file(Path::new("/sys/fs/cgroup/dmem.capacity")).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Failed to read /sys/fs/cgroup/dmem.capacity")
        })?;

    let non_boosted_limits: DmemLimit = boosted_limits.keys().map(|key| (key.clone(), 0)).collect();
    let mut event_socket = Socket::connect()?;
    let conn = Connection::session()?;

    let reply = event_socket.send(Request::EventStream)?;
    if let Err(message) = reply {
        return Err(
            io::Error::other(format!("Failed to request niri event stream: {message}")).into()
        );
    }

    let focused_dmem_low_path_for_cleanup = Arc::new(Mutex::new(None::<PathBuf>));
    let cleanup_focused_dmem_low_path = Arc::clone(&focused_dmem_low_path_for_cleanup);
    let cleanup_non_boosted_limits = non_boosted_limits.clone();

    ctrlc::set_handler(move || {
        let focused_path = match cleanup_focused_dmem_low_path.lock() {
            Ok(path) => path.clone(),
            Err(error) => {
                eprintln!("WARNING: Failed to lock focused dmem.low path during cleanup: {error}");
                std::process::exit(1);
            }
        };

        if let Some(path) = focused_path.as_ref()
            && let Err(error) = set_dmem_low(path, &cleanup_non_boosted_limits)
        {
            eprintln!("WARNING: Failed to cleanup dmem.low at {}: {error}", path.display());
            std::process::exit(1);
        }

        std::process::exit(0);
    })?;

    let mut focused_dmem_low_path: Option<PathBuf> = None;
    let mut windows = WindowsState::default();

    let mut read_event = event_socket.read_events();
    while let Ok(event) = read_event() {
        windows.apply(event.clone());

        let resolve_dmem_path = |pid| match dmem_low_path_for_pid(&conn, pid) {
            Ok(path) => Some(path),
            Err(error) => {
                eprintln!("WARNING: Failed to resolve cgroup for PID {pid}: {error}");
                None
            }
        };

        let new_focused_path = match event {
            Event::WindowFocusChanged { id } => id
                .and_then(|window_id| windows.windows.get(&window_id))
                .and_then(|window| window.pid)
                .and_then(resolve_dmem_path),
            Event::WindowOpenedOrChanged { window } => {
                if !window.is_focused {
                    continue;
                }

                window.pid.and_then(resolve_dmem_path)
            }
            Event::WindowsChanged { .. } => windows
                .windows
                .values()
                .find(|window| window.is_focused)
                .and_then(|window| window.pid)
                .and_then(resolve_dmem_path),
            _ => continue,
        };

        if focused_dmem_low_path == new_focused_path {
            continue;
        }

        if let Some(previous_path) = focused_dmem_low_path.as_ref()
            && let Err(error) = set_dmem_low(previous_path, &non_boosted_limits)
        {
            eprintln!(
                "WARNING: Failed to set non-focused dmem.low at {}: {error}",
                previous_path.display()
            );
        }

        if let Some(current_path) = new_focused_path.as_ref()
            && let Err(error) = set_dmem_low(current_path, &boosted_limits)
        {
            eprintln!(
                "WARNING: Failed to set focused dmem.low at {}: {error}",
                current_path.display()
            );
            focused_dmem_low_path = None;
            continue;
        }

        focused_dmem_low_path = new_focused_path;
        if let Ok(mut path) = focused_dmem_low_path_for_cleanup.lock() {
            *path = focused_dmem_low_path.clone();
        }
    }

    Ok(())
}
