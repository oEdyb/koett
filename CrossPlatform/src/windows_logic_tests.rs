#[path = "windows/meter.rs"]
mod meter;
#[path = "windows/paste_timing.rs"]
mod paste_timing;
// Pure pill state; its drawing and accessibility callers only build on Windows.
#[allow(dead_code)]
#[path = "windows/pill_state.rs"]
mod pill_state;
#[path = "windows/pin_policy.rs"]
mod pin_policy;
