mod dbus;
mod limits;

use dbus::*;
use limits::*;
use niri_ipc::state::{EventStreamStatePart, WindowsState};
use niri_ipc::{Event, Request, socket::Socket};
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use zbus::blocking::Connection;

fn main() -> Result<(), Box<dyn Error>> {
    let boosted_limits =
        parse_limits_file(Path::new("/sys/fs/cgroup/dmem.capacity")).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Failed to read /sys/fs/cgroup/dmem.capacity",
            )
        })?;

    let non_boosted_limits: DmemLimit = boosted_limits.keys().map(|key| (key.clone(), 0)).collect();

    let conn = Connection::session()?;

    let mut event_socket = Socket::connect()?;
    let reply = event_socket.send(Request::EventStream)?;
    if let Err(message) = reply {
        return Err(
            io::Error::other(format!("Failed to request niri event stream: {message}")).into(),
        );
    }

    let mut focused_dmem_low_path: Option<PathBuf> = None;
    let mut windows = WindowsState::default();
    let mut new_focused_path;

    let mut read_event = event_socket.read_events();
    while let Ok(event) = read_event() {
        windows.apply(event.clone());

        match event {
            Event::WindowFocusChanged { id } => {
                new_focused_path = id
                    .and_then(|window_id| windows.windows.get(&window_id))
                    .and_then(|window| window.pid)
                    .and_then(|pid| match dmem_low_path_for_pid(&conn, pid) {
                        Ok(path) => Some(path),
                        Err(error) => {
                            eprintln!("WARNING: Failed to resolve cgroup for PID {pid}: {error}");
                            None
                        }
                    });
            }
            Event::WindowOpenedOrChanged { window } => {
                if !window.is_focused {
                    continue;
                }

                new_focused_path =
                    window
                        .pid
                        .and_then(|pid| match dmem_low_path_for_pid(&conn, pid) {
                            Ok(path) => Some(path),
                            Err(error) => {
                                eprintln!(
                                    "WARNING: Failed to resolve cgroup for PID {pid}: {error}"
                                );
                                None
                            }
                        });
            }
            _ => continue,
        }

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
    }

    Ok(())
}
