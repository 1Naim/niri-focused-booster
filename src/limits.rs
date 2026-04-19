use std::collections::HashMap;
use std::io;
use std::path::Path;

pub type DmemLimit = HashMap<String, u64>;

// https://gitlab.steamos.cloud/holo/dmemcg-booster/-/blob/903e18c761c41ecca2a6dced9335a2c3f0703b11/src/cgroup.rs#L99
pub fn parse_limits_file(path: &Path) -> Option<DmemLimit> {
    if let Ok(str) = std::fs::read_to_string(path) {
        let mut limits = DmemLimit::new();
        for line in str.lines() {
            let words: Vec<_> = line.split(' ').collect();
            if words.len() != 2 {
                eprintln!("WARNING: Unexpected dmem line: \"{line}\"");
                continue;
            }

            if let Ok(val) = words[1].parse::<u64>() {
                limits.insert(words[0].to_string(), val);
            } else {
                eprintln!("WARNING: Could not parse dmem limit in line: \"{line}\"");
            }
        }

        Some(limits)
    } else {
        None
    }
}

// https://github.com/pixelcluster/kcgroups/blob/d106f1c8f44c1f231a9050b2bf82a34bb7d5db32/src/kapplicationscope.cpp#L248-L267
pub fn set_dmem_low(path: &Path, limits: &DmemLimit) -> io::Result<()> {
    let mut contents = String::new();
    for (device, limit) in limits {
        contents.push_str(device);
        contents.push(' ');
        contents.push_str(limit.to_string().as_str());
        contents.push('\n');
    }

    std::fs::write(path, contents)
}
