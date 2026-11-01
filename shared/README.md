# shared

Crates used by more than one Showcase application will go here once a second application needs them. The folder is empty today: the kiosk (`../kiosk`) is the only application so far.

Before adding a crate here, ask whether anyone outside the Showcase could use it. If they could, it belongs in **NoWayOS**, not here. Upstream what is generic: bounded containers, widgets, drivers, IPC contracts and timer helpers all belong in NoWayOS. `shared/` is for application logic that several Showcase applications have in common.

When the first crate arrives, add `"shared/*"` to `members` in the root `Cargo.toml`.
