#[cfg(target_os = "linux")]
fn main() -> Result<(), String> {
    let runtime = std::env::var("XDG_RUNTIME_DIR").map_err(|_| "Private lab required")?;
    if !runtime.ends_with("/agent-lab/koett-ask-finish")
        || std::env::var_os("AGENT_LAB_GENERATION").is_none()
    {
        return Err("Private koett-ask lab required".into());
    }
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    if std::env::args().any(|s| s == "--screen") {
        let frame = koett_engine::assistant::screen::capture(&cancelled)?;
        println!("one_frame_jpeg_bytes={}; image_files=0", frame.len());
    } else {
        assert!(
            koett_engine::assistant::secret(
                if std::env::args().any(|s| s == "--locked") {
                    "custom"
                } else {
                    "openRouter"
                },
                None,
                &cancelled
            )
            .is_err()
        );
        println!("private_secret_service_absent_fails_closed=passed");
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn main() {}
