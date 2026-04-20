use std::error::Error;
use std::io;
use std::path::PathBuf;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

pub fn get_control_group(
    connection: &Connection, unit_path: &OwnedObjectPath,
) -> Result<Option<String>, Box<dyn Error>> {
    for iface_name in [
        "org.freedesktop.systemd1.Service",
        "org.freedesktop.systemd1.Scope",
        "org.freedesktop.systemd1.Slice",
        "org.freedesktop.systemd1.Socket",
        "org.freedesktop.systemd1.Unit",
    ] {
        let proxy =
            Proxy::new(connection, "org.freedesktop.systemd1", unit_path.clone(), iface_name)?;

        let response: Result<String, zbus::Error> = proxy.get_property("ControlGroup");
        if let Ok(control_group) = response
            && !control_group.is_empty()
        {
            return Ok(Some(control_group));
        }
    }

    Ok(None)
}

pub fn dmem_low_path_for_pid(connection: &Connection, pid: i32) -> Result<PathBuf, Box<dyn Error>> {
    let pid = u32::try_from(pid)?;
    let manager = Proxy::new(
        connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    )?;

    let unit_path: OwnedObjectPath = manager.call("GetUnitByPID", &(pid,))?;
    let mut path = PathBuf::from("/sys/fs/cgroup");

    let control_group = get_control_group(connection, &unit_path)?.ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("ControlGroup was missing for PID {pid}"))
    })?;

    let relative_control_group = control_group.trim_start_matches('/');
    if !relative_control_group.is_empty() {
        path.push(relative_control_group);
    }

    path.push("dmem.low");
    Ok(path)
}
