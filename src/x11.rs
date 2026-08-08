use std::path::PathBuf;
use xcb::{Xid, x};

pub fn find_real_pid(pid: i32) -> Option<i32> {
    if !is_xwayland_satellite(pid) {
        return Some(pid);
    }

    let (connection, screen_index) = xcb::Connection::connect(None).ok()?;
    let setup = connection.get_setup();
    let screen = setup.roots().nth(screen_index as usize)?;

    let active_window_atom = intern_atom(&connection, b"_NET_ACTIVE_WINDOW")?;
    if active_window_atom == x::ATOM_NONE {
        return None;
    }

    let window_before = active_window(&connection, screen.root(), active_window_atom)?;

    let wm_pid_atom = intern_atom(&connection, b"_NET_WM_PID")?;
    if wm_pid_atom == x::ATOM_NONE {
        return None;
    }

    let real_pid = connection
        .wait_for_reply(connection.send_request(&x::GetProperty {
            delete: false,
            window: window_before,
            property: wm_pid_atom,
            r#type: x::ATOM_CARDINAL,
            long_offset: 0,
            long_length: 1,
        }))
        .ok()?
        .value::<u32>()
        .first()
        .copied()
        .and_then(|pid| i32::try_from(pid).ok())?;

    let window_after = active_window(&connection, screen.root(), active_window_atom)?;
    if window_before != window_after {
        return None;
    }

    Some(real_pid)
}

fn is_xwayland_satellite(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }

    let proc_base = PathBuf::from("/proc").join(pid.to_string());

    let cmdline = std::fs::read(proc_base.join("cmdline")).ok();
    let Some(cmdline) = cmdline else {
        return false;
    };

    cmdline
        .split(|b| *b == 0)
        .filter(|arg| !arg.is_empty())
        .any(|arg| arg == b"xwayland-satellite" || arg.ends_with(b"/xwayland-satellite"))
}

fn intern_atom(connection: &xcb::Connection, name: &[u8]) -> Option<x::Atom> {
    let reply = connection
        .wait_for_reply(connection.send_request(&x::InternAtom {
            only_if_exists: true,
            name,
        }))
        .ok()?;

    Some(reply.atom())
}

fn active_window(
    connection: &xcb::Connection, root: x::Window, active_window_atom: x::Atom,
) -> Option<x::Window> {
    let active_window = connection
        .wait_for_reply(connection.send_request(&x::GetProperty {
            delete: false,
            window: root,
            property: active_window_atom,
            r#type: x::ATOM_WINDOW,
            long_offset: 0,
            long_length: 1,
        }))
        .ok()?;

    active_window
        .value::<x::Window>()
        .first()
        .copied()
        .filter(|window| *window != x::Window::none())
}
