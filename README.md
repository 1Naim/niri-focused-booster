# niri-focused-booster

`niri-focused-booster` is a tool that leverages the dmem cgroup controller inside the kernel to
prioritize GPU memory for the currently focused app on Niri.

It listens to Niri focus events, resolves the focused window's PID to its cgroup, and updates that
cgroup's memory protection limit. The focused app gets boosted limits while all other apps have it
at 0.

## Requirements

- [`dmemcg-booster`](https://gitlab.steamos.cloud/holo/dmemcg-booster/)
- `systemd`
- kernel patched with
  [this](https://lore.kernel.org/all/20260313-dmemcg-aggressive-protect-v6-0-7c71cc1492db@gmx.de/)
    - `linux-cachyos`
    - [`linux-dmemcg`](https://aur.archlinux.org/packages/linux-dmemcg)


## Installing

This project is available in the [AUR](https://aur.archlinux.org/packages). You can clone and build
it manually or use your favourite AUR helper.

```bash
paru -S niri-focused-booster
```

## Build

```bash
cargo fetch --locked --target "$(rustc --print host-tuple)"
cargo build --release --frozen

# This will install to ~/.cargo/bin
cargo install --path .
```

## Usage

Add the following to your niri configuration file (`~/.config/niri/config.kdl`):

```kdl
spawn-at-startup "niri-focused-booster"
```
