use super::*;

type Levels = Arc<Mutex<Result<(f32, f32), String>>>;
pub(crate) struct Capture {
    process: Process,
    reader: Option<thread::JoinHandle<()>>,
    levels: Levels,
}
impl Capture {
    pub fn start(microphone: Option<&str>) -> Result<Self, String> {
        let mut process = spawn("preview", microphone)?;
        let (opened, mut output) = ready::<bool>(&mut process)?;
        if !opened {
            return Err("Microphone preview did not become ready".into());
        }
        let levels = Arc::new(Mutex::new(Ok((0.0, 0.0))));
        let shared = Arc::clone(&levels);
        let lease = Arc::clone(&process._lease);
        let reader = thread::spawn(move || {
            let _lease = lease;
            loop {
                let value = read_levels(&mut output);
                let failed = value.is_err();
                if let Ok(mut stored) = shared.lock() {
                    *stored = value;
                } else {
                    break;
                }
                if failed {
                    break;
                }
            }
        });
        Ok(Self {
            process,
            reader: Some(reader),
            levels,
        })
    }
    pub fn levels(&self) -> Result<(f32, f32), String> {
        self.levels
            .lock()
            .map_err(|_| "Microphone preview failed".to_string())?
            .clone()
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.process.stop();
        if let Some(reader) = self.reader.take() {
            let _ = join(reader);
        }
    }
}
fn read_levels(reader: &mut impl BufRead) -> Result<(f32, f32), String> {
    let (rms, peak) = json_line::<Result<(f32, f32), String>>(reader)??;
    if !rms.is_finite() || !peak.is_finite() || rms < 0.0 || peak < 0.0 {
        return Err("Invalid microphone preview level".into());
    }
    Ok((rms, peak))
}
pub(super) fn serve(output: &mut impl Write, microphone: Option<&str>) -> Result<(), String> {
    let capture = preview::Capture::start(microphone, || true)
        .and_then(|capture| capture.ok_or_else(|| "Microphone preview unavailable".to_string()));
    let opened = capture.as_ref().map(|_| true).map_err(Clone::clone);
    writeln!(
        output,
        "{}",
        serde_json::to_string(&opened).map_err(|e| e.to_string())?
    )
    .map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    let capture = capture?;
    loop {
        let levels = capture.levels();
        writeln!(
            output,
            "{}",
            serde_json::to_string(&levels).map_err(|e| e.to_string())?
        )
        .map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
        levels?;
        thread::sleep(Duration::from_millis(50));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_protocol_accepts_levels_and_propagates_failure_without_audio() {
        assert_eq!(
            read_levels(&mut &b"{\"Ok\":[0.25,0.5]}\n"[..]).unwrap(),
            (0.25, 0.5)
        );
        assert!(read_levels(&mut &b"{\"Ok\":[-1,0]}\n"[..]).is_err());
        assert!(read_levels(&mut &b"{\"Ok\":[0,null]}\n"[..]).is_err());
        assert_eq!(
            read_levels(&mut &b"{\"Err\":\"Disconnected\"}\n"[..]).unwrap_err(),
            "Disconnected"
        );
        assert!(read_levels(&mut &b""[..]).is_err());
    }
}
