//! How long the 110M dictionary sidecar takes per audio length, against its budget.
//! Args: V2_MODEL WAV. Prints JSON with each length's decode times in milliseconds.
use koett_engine::{
    audio::AudioRecording,
    dictionary::DictionaryEntry,
    transcription::{ParakeetTranscriber, Transcriber},
};
use std::{path::Path, time::Instant};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [model, wav] = args.as_slice() else {
        return Err("V2_MODEL WAV".into());
    };
    let dictionary = [
        DictionaryEntry::new("Phoebe", &[]),
        DictionaryEntry::new("Koett", &[]),
        DictionaryEntry::new("Parakeet", &[]),
    ];
    // Same thread count as the sidecar.
    let mut engine = ParakeetTranscriber::load_with_dictionary(Path::new(model), 4, &dictionary)?;
    let source = AudioRecording::read_wav(Path::new(wav))?;
    let rate = source.sample_rate as usize;
    engine.transcribe(&source)?;
    for seconds in [1, 2, 3, 4, 5, 6, 8, 10, 15, 20] {
        let wanted = seconds * rate;
        let samples: Vec<f32> = source
            .samples
            .iter()
            .cycle()
            .take(wanted)
            .copied()
            .collect();
        let audio = AudioRecording::new(source.sample_rate, samples)?;
        let mut times = Vec::new();
        for _ in 0..5 {
            let started = Instant::now();
            engine.transcribe(&audio)?;
            times.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{}",
            serde_json::json!({"seconds":seconds,"median_ms":times[2].round(),"worst_ms":times[4].round(),"within_budget":times[4] < 250.0})
        );
    }
    Ok(())
}
