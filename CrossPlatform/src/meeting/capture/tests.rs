use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::super::{Offset, SAMPLE_RATE, Span};
use super::disk::{CaptureFault, CaptureFaultReason, CaptureHealth, CaptureNotice};
use super::ingress::{Stamp, TrackDrain, TrackLevel};
use super::journal::{GapCause, JournalWriter, TrackChunk, TrackReader};
use super::recorder::TrackRecorder;
use super::writer::{TrackConfig, TrackWriter};
use super::*;

const EPOCH: SourceEpoch = SourceEpoch {
    generation: 0,
    timing: Timing::Arrival,
};
const BLOCK: usize = 320;
const MIB: u64 = 1024 * 1024;
const NO_CUTOFF: Offset = Offset(u64::MAX);

fn make_stamp(at: Instant) -> Stamp {
    Stamp {
        at,
        frames_pushed_total: 0,
        dropped: 0,
        epoch: EPOCH,
    }
}

fn read(bytes: &[u8], cutoff: Offset) -> TrackReader<&[u8]> {
    TrackReader::from_reader(bytes, cutoff, u64::MAX).unwrap()
}

#[test]
fn drift_plus_80_ppm_keeps_two_hours_on_the_session_clock() {
    assert_drift(80);
}

#[test]
fn drift_minus_80_ppm_keeps_two_hours_on_the_session_clock() {
    assert_drift(-80);
}

fn assert_drift(ppm: i64) {
    const BLOCKS: u64 = 2 * 60 * 60 * 50;
    let zero = Instant::now();
    let start = Duration::from_secs(2);
    let mut bytes = Vec::with_capacity(BLOCKS as usize * BLOCK * 2 + 300_000);
    let mut recorder =
        TrackRecorder::new(JournalWriter::new(&mut bytes).unwrap(), zero, SAMPLE_RATE).unwrap();
    const PERIOD: usize = 160 * BLOCK;
    let signal: Vec<f32> = (0..PERIOD)
        .map(|index| (index as f32 - PERIOD as f32 / 2.0) / 32_768.0)
        .collect();
    let blocks = signal.as_chunks::<BLOCK>().0.iter().cycle();
    for (n, block) in (0..BLOCKS).zip(blocks) {
        let drifted_ns = (20 * (1_000_000 + ppm)) as u64 * n;
        let stamp = zero + start + Duration::from_nanos(drifted_ns);
        recorder.push(make_stamp(stamp), block).unwrap();
    }
    recorder.finish(0).unwrap();

    let mut written = 0u64;
    let rate = 1.0 + ppm as f64 / 1_000_000.0;
    for item in read(&bytes, NO_CUTOFF) {
        match item.unwrap() {
            TrackChunk::Pcm { first, samples } => {
                let count = samples.len() as u64;
                let ordinal =
                    |sample: i16| u64::try_from(i32::from(sample) + PERIOD as i32 / 2).unwrap();
                let period = PERIOD as u64;
                let overlap = (ordinal(samples[0]) + period - written % period) % period;
                assert!(overlap <= 2);
                written += overlap;
                assert_eq!(
                    ordinal(samples[samples.len() - 1]),
                    (written + count - 1) % period
                );
                // Error is affine within each contiguous record, so its extrema are the endpoints.
                for index in [0, count - 1] {
                    let truth = start.as_secs_f64() * f64::from(SAMPLE_RATE)
                        + (written + index) as f64 * rate;
                    assert!(((first.0 + index) as f64 - truth).abs() <= 2400.0);
                }
                written += count;
            }
            TrackChunk::Silence(span) => assert!(span.end.0 - span.first.0 <= 2),
            TrackChunk::Gap { span, cause } => panic!("unexpected gap {span:?} ({cause:?})"),
        }
    }
    assert_eq!(written, BLOCKS * BLOCK as u64);
}

fn record_blocks(stamp_ms: impl Fn(u64) -> u64, blocks: u64) -> Vec<u8> {
    let zero = Instant::now();
    let (mut ingress, mut drain) = TrackIngress::pair(SAMPLE_RATE, EPOCH);
    let mut bytes = Vec::new();
    let mut recorder =
        TrackRecorder::new(JournalWriter::new(&mut bytes).unwrap(), zero, SAMPLE_RATE).unwrap();
    let mut samples = Vec::new();
    for n in 0..blocks {
        let stamp = zero + Duration::from_millis(stamp_ms(n));
        let marker = f32::from(i16::try_from(n).unwrap()) / 32_768.0;
        assert!(ingress.push(&[marker; BLOCK], stamp));
        pump(&mut drain, &mut recorder, &mut samples);
    }
    recorder.finish(ingress.dropped_frames()).unwrap();
    bytes
}

fn pump<W: io::Write>(
    drain: &mut TrackDrain,
    recorder: &mut TrackRecorder<W>,
    samples: &mut Vec<f32>,
) {
    while let Some(stamp) = drain.pop(samples) {
        recorder.push(stamp, samples).unwrap();
    }
}

#[test]
fn three_second_pause_has_one_three_second_gap() {
    let bytes = record_blocks(|n| n * 20 + if n >= 263 { 3000 } else { 0 }, 500);
    let chunks = read(&bytes, NO_CUTOFF)
        .collect::<io::Result<Vec<_>>>()
        .unwrap();
    let gaps: Vec<_> = chunks
        .iter()
        .filter_map(|chunk| match chunk {
            TrackChunk::Gap { span, cause } => Some((span, *cause)),
            _ => None,
        })
        .collect();
    assert_eq!(gaps.len(), 1);
    let (span, cause) = gaps[0];
    assert_eq!(cause, GapCause::ClockJump);
    assert!(span.first.0.abs_diff((263 + 100) * BLOCK as u64) <= 800);
    assert!((span.end.0 - span.first.0).abs_diff(48_000) <= 800);
}

#[test]
fn confirmed_twenty_millisecond_error_steers_two_samples_per_record() {
    let bytes = record_blocks(|n| n * 20 + if n >= 263 { 20 } else { 0 }, 500);
    let mut corrections = 0;
    for chunk in read(&bytes, NO_CUTOFF) {
        match chunk.unwrap() {
            TrackChunk::Pcm { .. } => {}
            TrackChunk::Silence(span) => {
                assert_eq!(span.end.0 - span.first.0, 2);
                corrections += 1;
            }
            TrackChunk::Gap { span, cause } => panic!("unexpected gap {span:?} ({cause:?})"),
        }
    }
    assert!(corrections > 1);
}

#[test]
fn a_quarter_second_hole_realigns_within_two_and_a_half_seconds() {
    const CHECK_BLOCK: u64 = 263 + 125;
    let stamp_ms = |n| n * 20 + if n >= 263 { 250 } else { 0 };
    let bytes = record_blocks(stamp_ms, CHECK_BLOCK + 25);
    let mut placement = None;
    let mut silence = 0;
    for chunk in read(&bytes, NO_CUTOFF) {
        match chunk.unwrap() {
            TrackChunk::Pcm { first, samples } => {
                if let Some(index) = samples
                    .iter()
                    .position(|&sample| sample == CHECK_BLOCK as i16)
                {
                    placement = Some(first.0 + index as u64);
                }
            }
            TrackChunk::Silence(span) => silence += span.end.0 - span.first.0,
            TrackChunk::Gap { span, cause } => panic!("unexpected gap {span:?} ({cause:?})"),
        }
    }
    let expected = stamp_ms(CHECK_BLOCK) * 16;
    let actual = placement.expect("check block survives playback");
    assert!(
        actual.abs_diff(expected) <= 80,
        "placed at {actual}, expected {expected}"
    );
    assert_eq!(silence, 4000);
}

#[test]
fn confirmed_pauses_at_and_above_three_hundred_milliseconds_write_one_clock_gap() {
    for pause in [300, 350] {
        let bytes = record_blocks(|n| n * 20 + if n >= 263 { pause } else { 0 }, 500);
        let gaps: Vec<_> = read(&bytes, NO_CUTOFF)
            .filter_map(|chunk| match chunk.unwrap() {
                TrackChunk::Gap { span, cause } => Some((span.end.0 - span.first.0, cause)),
                TrackChunk::Pcm { .. } | TrackChunk::Silence(_) => None,
            })
            .collect();
        assert_eq!(gaps, [(pause * 16, GapCause::ClockJump)]);
    }
}

#[test]
fn one_second_pipe_backlog_bursts_do_not_shift_audio_or_create_gaps() {
    let bytes = record_blocks(
        |n| {
            let arrival = n * 20;
            match n % 125 {
                25..=75 => arrival + (75 - n % 125) * 20,
                _ => arrival,
            }
        },
        625,
    );
    assert_contiguous_pcm(&bytes, 625);
}

#[test]
fn buffered_stall_longer_than_the_clock_window_does_not_shift_audio_or_create_gaps() {
    const STALL_BLOCK: u64 = 263;
    const RESUME_BLOCK: u64 = STALL_BLOCK + 2040 / 20;
    let bytes = record_blocks(
        |n| {
            if n >= STALL_BLOCK {
                n.max(RESUME_BLOCK) * 20
            } else {
                n * 20
            }
        },
        500,
    );
    assert_contiguous_pcm(&bytes, 500);
}

fn assert_contiguous_pcm(bytes: &[u8], blocks: usize) {
    let mut frames = 0;
    for chunk in read(bytes, NO_CUTOFF) {
        let TrackChunk::Pcm { first, samples } = chunk.unwrap() else {
            panic!("a backlog shifted the audio");
        };
        assert_eq!(first, Offset(frames as u64));
        frames += samples.len();
    }
    assert_eq!(frames, blocks * BLOCK);
}

#[test]
fn ring_overflow_preserves_accepted_audio_and_reports_lost_frames() {
    let zero = Instant::now();
    let (mut ingress, mut drain) = TrackIngress::pair(SAMPLE_RATE, EPOCH);
    let mut bytes = Vec::new();
    let mut recorder =
        TrackRecorder::new(JournalWriter::new(&mut bytes).unwrap(), zero, SAMPLE_RATE).unwrap();
    let mut samples = Vec::new();
    let full_ring = [0.25; 8 * SAMPLE_RATE as usize];
    assert!(ingress.push(&full_ring, zero));
    assert!(!ingress.push(&[0.25; BLOCK], zero + Duration::from_secs(8)));
    pump(&mut drain, &mut recorder, &mut samples);
    assert!(ingress.push(&[0.25; BLOCK], zero + Duration::from_millis(8020)));
    pump(&mut drain, &mut recorder, &mut samples);
    recorder.finish(ingress.dropped_frames()).unwrap();

    let mut chunks = read(&bytes, NO_CUTOFF)
        .collect::<io::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(ingress.dropped_frames(), BLOCK as u64);
    assert_eq!(
        chunks.pop(),
        Some(TrackChunk::Pcm {
            first: Offset(128_320),
            samples: vec![8192; BLOCK],
        })
    );
    assert_eq!(
        chunks.pop(),
        Some(TrackChunk::Gap {
            span: Span {
                first: Offset(128_000),
                end: Offset(128_320)
            },
            cause: GapCause::Overrun,
        })
    );
    assert_eq!(chunks.len(), 16);
}

#[test]
fn new_source_generation_cuts_a_gap_even_below_the_clock_threshold() {
    let dir = TestDir::new();
    let zero = Instant::now();
    let config = dir.config(zero);
    let (writer, mut ingress) = TrackWriter::start(config, |_| Ok(2048 * MIB), |_| {}).unwrap();
    assert!(ingress.push(&[0.25; BLOCK], zero));
    ingress.begin_epoch(SourceEpoch {
        generation: 1,
        timing: Timing::Arrival,
    });
    assert!(ingress.push(&[0.25; BLOCK], zero + Duration::from_millis(100)));
    drop(ingress);
    writer.finish().unwrap();

    let chunks = TrackReader::open(&dir.journal(), NO_CUTOFF)
        .unwrap()
        .collect::<io::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        chunks,
        [
            TrackChunk::Pcm {
                first: Offset(0),
                samples: vec![8192; BLOCK]
            },
            TrackChunk::Gap {
                span: Span {
                    first: Offset(320),
                    end: Offset(1600)
                },
                cause: GapCause::SourceRestart
            },
            TrackChunk::Pcm {
                first: Offset(1600),
                samples: vec![8192; BLOCK]
            },
        ]
    );
}

#[test]
fn a_truncated_tail_and_a_flipped_byte_stop_at_the_valid_prefix() {
    let mut extended = Vec::new();
    let mut journal = JournalWriter::new(&mut extended).unwrap();
    journal.pcm(Offset(40), &[0.25; 2]).unwrap();
    let prefix_len = journal.sink().len();
    journal.pcm(Offset(42), &[0.5; 2]).unwrap();
    journal.pcm(Offset(44), &[0.75; 2]).unwrap();

    let truncated = &extended[..prefix_len + 8];
    let mut flipped = extended.clone();
    flipped[prefix_len + 13] ^= 0x04;
    let dir = TestDir::new();
    for damaged in [truncated, flipped.as_slice()] {
        fs::write(dir.journal(), damaged).unwrap();
        let mut reader = TrackReader::open(&dir.journal(), NO_CUTOFF).unwrap();
        assert_eq!(
            reader.next().unwrap().unwrap(),
            TrackChunk::Pcm {
                first: Offset(40),
                samples: vec![8192; 2],
            }
        );
        assert!(reader.next().is_none());
        assert_eq!(
            reader.seal().unwrap(),
            super::super::TrackSeal {
                valid_bytes: prefix_len as u64,
                captured_until: Offset(42),
            }
        );
        assert_eq!(fs::read(dir.journal()).unwrap(), damaged);
    }
}

#[test]
fn reader_rejects_backward_offsets_even_with_valid_checksums() {
    let mut bytes = Vec::new();
    let mut journal = JournalWriter::new(&mut bytes).unwrap();
    journal.pcm(Offset(10), &[0.25; 4]).unwrap();
    let prefix_len = journal.sink().len();
    journal.pcm(Offset(9), &[0.5; 4]).unwrap();
    journal.pcm(Offset(14), &[0.75; 4]).unwrap();
    assert_eq!(
        read(&bytes, NO_CUTOFF).seal().unwrap(),
        super::super::TrackSeal {
            valid_bytes: prefix_len as u64,
            captured_until: Offset(14),
        }
    );
}

#[test]
fn sealed_reader_honors_the_valid_byte_limit_and_clips_gaps_at_cutoff() {
    let dir = TestDir::new();
    let mut bytes = Vec::new();
    let mut journal = JournalWriter::new(&mut bytes).unwrap();
    journal.pcm(Offset(10), &[0.25; 4]).unwrap();
    journal
        .gap(
            Span {
                first: Offset(14),
                end: Offset(40),
            },
            GapCause::Overrun,
        )
        .unwrap();
    let retained_bytes = journal.sink().len();
    journal.pcm(Offset(40), &[0.5; 4]).unwrap();
    fs::write(dir.journal(), &bytes).unwrap();
    let seal = super::super::TrackSeal {
        valid_bytes: retained_bytes as u64,
        captured_until: Offset(14),
    };
    for cutoff in [Offset(25), NO_CUTOFF] {
        let mut reader = TrackReader::open_sealed(&dir.journal(), cutoff, seal).unwrap();
        let chunks = reader.by_ref().collect::<io::Result<Vec<_>>>().unwrap();
        assert_eq!(
            chunks,
            [
                TrackChunk::Pcm {
                    first: Offset(10),
                    samples: vec![8192; 4]
                },
                TrackChunk::Gap {
                    span: Span {
                        first: Offset(14),
                        end: Offset(40).min(cutoff)
                    },
                    cause: GapCause::Overrun
                },
            ]
        );
        assert_eq!(reader.seal().unwrap(), seal);
    }
}

#[test]
fn reader_trims_overlap_fills_short_holes_and_clips_the_cutoff() {
    let mut bytes = Vec::new();
    let mut journal = JournalWriter::new(&mut bytes).unwrap();
    journal.pcm(Offset(10), &[0.25; 4]).unwrap();
    journal.pcm(Offset(12), &[0.5; 4]).unwrap();
    journal.pcm(Offset(18), &[0.75; 4]).unwrap();
    journal.pcm(Offset(22), &[0.0; 4]).unwrap();
    let mut reader = read(&bytes, Offset(20));
    let chunks = reader.by_ref().collect::<io::Result<Vec<_>>>().unwrap();
    assert_eq!(
        chunks,
        [
            TrackChunk::Pcm {
                first: Offset(10),
                samples: vec![8192; 4]
            },
            TrackChunk::Pcm {
                first: Offset(14),
                samples: vec![16_384; 2]
            },
            TrackChunk::Silence(Span {
                first: Offset(16),
                end: Offset(18)
            }),
            TrackChunk::Pcm {
                first: Offset(18),
                samples: vec![24_576; 2]
            },
        ]
    );
    assert!(reader.next().is_none());
    assert_eq!(
        reader.seal().unwrap(),
        super::super::TrackSeal {
            valid_bytes: bytes.len() as u64,
            captured_until: Offset(20),
        }
    );
    assert!(read(&bytes, Offset(10)).next().is_none());
}

#[test]
fn writer_resamples_native_rate_and_flushes_a_short_final_record() {
    let dir = TestDir::new();
    let zero = Instant::now();
    let mut config = dir.config(zero);
    config.sample_rate = 48_000;
    config.epoch.timing = Timing::Callback;
    let (writer, mut ingress) = TrackWriter::start(config, |_| Ok(2048 * MIB), |_| {}).unwrap();
    for n in 0..31 {
        assert!(ingress.push(&[0.25; 960], zero + Duration::from_millis(n * 20)));
    }
    drop(ingress);
    writer.finish().unwrap();
    let chunks = TrackReader::open(&dir.journal(), NO_CUTOFF)
        .unwrap()
        .collect::<io::Result<Vec<_>>>()
        .unwrap();
    let [
        TrackChunk::Pcm {
            first: Offset(0),
            samples: head,
        },
        TrackChunk::Pcm {
            first: Offset(8000),
            samples: tail,
        },
    ] = chunks.as_slice()
    else {
        panic!("{chunks:?}")
    };
    assert_eq!((head.len(), tail.len()), (8000, 1920));
    assert!(head[4000].abs_diff(8192) < 128);
}

#[test]
fn disk_preflight_refuses_to_create_a_journal_below_one_gib() {
    let dir = TestDir::new();
    let result = TrackWriter::start(dir.config(Instant::now()), |_| Ok(1023 * MIB), |_| {});
    let Err(fault) = result else {
        panic!("preflight admitted low disk space")
    };
    assert_eq!(
        fault.reason,
        CaptureFaultReason::DiskLow {
            free_bytes: 1023 * MIB
        }
    );
    assert!(!dir.journal().exists());
}

#[test]
fn running_writer_warns_recovers_and_faults_at_two_hundred_mib() {
    let dir = TestDir::new();
    let zero = Instant::now();
    let tick = Arc::new(AtomicU64::new(0));
    let available = Arc::new(AtomicU64::new(2048 * MIB));
    let (notice, received) = mpsc::channel();
    let (writer, ingress) = TrackWriter::start_with_clock(
        dir.config(zero),
        {
            let available = available.clone();
            move |_| Ok(available.load(Ordering::Acquire))
        },
        move |event| notice.send(event).unwrap(),
        {
            let tick = tick.clone();
            move || zero + Duration::from_secs(tick.load(Ordering::Acquire))
        },
    )
    .unwrap();

    available.store(512 * MIB, Ordering::Release);
    tick.store(30, Ordering::Release);
    assert_eq!(
        received.recv_timeout(Duration::from_secs(1)).unwrap(),
        CaptureNotice::Health(CaptureHealth::DiskLow {
            free_bytes: 512 * MIB
        },)
    );
    available.store(1024 * MIB, Ordering::Release);
    tick.store(60, Ordering::Release);
    assert_eq!(
        received.recv_timeout(Duration::from_secs(1)).unwrap(),
        CaptureNotice::Health(CaptureHealth::DiskHealthy)
    );
    available.store(200 * MIB, Ordering::Release);
    tick.store(90, Ordering::Release);
    let fault = CaptureFault {
        at: Offset(90 * u64::from(SAMPLE_RATE)),
        reason: CaptureFaultReason::DiskLow {
            free_bytes: 200 * MIB,
        },
    };
    assert_eq!(
        received.recv_timeout(Duration::from_secs(1)).unwrap(),
        CaptureNotice::Fault(fault.clone())
    );
    drop(ingress);
    assert_eq!(writer.finish().unwrap_err(), fault);
}

#[test]
fn low_disk_fault_preserves_partial_pcm_and_queued_blocks() {
    let dir = TestDir::new();
    let zero = Instant::now();
    let tick = Arc::new(AtomicU64::new(0));
    let (notice, received) = mpsc::channel();
    let (probing, probe_started) = mpsc::channel();
    let (queued, blocks_queued) = mpsc::channel();
    let (closing, close_started) = mpsc::channel();
    let (checked, push_checked) = mpsc::channel();
    let mut preflight = true;
    let disk_tick = tick.clone();
    let (writer, mut ingress) = TrackWriter::start_with_clock(
        dir.config(zero),
        move |_| {
            if std::mem::take(&mut preflight) {
                return Ok(2048 * MIB);
            }
            probing.send(()).unwrap();
            blocks_queued.recv_timeout(Duration::from_secs(2)).unwrap();
            disk_tick.store(31, Ordering::Release);
            Ok(200 * MIB)
        },
        move |event| notice.send(event).unwrap(),
        {
            let tick = tick.clone();
            move || {
                let seconds = tick.load(Ordering::Acquire);
                if seconds == 31 {
                    closing.send(()).unwrap();
                    push_checked.recv_timeout(Duration::from_secs(2)).unwrap();
                }
                zero + Duration::from_secs(seconds)
            }
        },
    )
    .unwrap();

    assert!(ingress.push(&[0.25; 8320], zero));
    let journal = dir.journal();
    let deadline = Instant::now() + Duration::from_secs(2);
    while TrackReader::open(&journal, NO_CUTOFF)
        .unwrap()
        .seal()
        .unwrap()
        .captured_until
        < Offset(8000)
    {
        assert!(
            Instant::now() < deadline,
            "writer did not commit its first record"
        );
        std::thread::sleep(Duration::from_millis(1));
    }

    tick.store(30, Ordering::Release);
    probe_started.recv_timeout(Duration::from_secs(2)).unwrap();
    for n in 26..30 {
        assert!(ingress.push(&[0.5; BLOCK], zero + Duration::from_millis(n * 20)));
    }
    queued.send(()).unwrap();
    close_started.recv_timeout(Duration::from_secs(2)).unwrap();
    let late_push = ingress.push(&[0.75; BLOCK], zero + Duration::from_secs(31));
    checked.send(()).unwrap();
    assert!(!late_push, "ingress accepted a block after the disk fault");

    let fault = CaptureFault {
        at: Offset(480_000),
        reason: CaptureFaultReason::DiskLow {
            free_bytes: 200 * MIB,
        },
    };
    assert_eq!(
        received.recv_timeout(Duration::from_secs(2)).unwrap(),
        CaptureNotice::Fault(fault.clone())
    );
    assert_eq!(writer.finish(), Err(fault));
    let mut pcm = Vec::new();
    for chunk in TrackReader::open(&journal, NO_CUTOFF).unwrap() {
        let TrackChunk::Pcm { first, samples } = chunk.unwrap() else {
            panic!("the journal should contain contiguous PCM");
        };
        assert_eq!(first, Offset(pcm.len() as u64));
        pcm.extend(samples);
    }
    assert_eq!(pcm.len(), 30 * BLOCK);
    assert!(pcm[..26 * BLOCK].iter().all(|&sample| sample == 8192));
    assert!(pcm[26 * BLOCK..].iter().all(|&sample| sample == 16_384));
}

#[test]
fn worker_panic_notifies_before_finish_and_closes_ingress() {
    let dir = TestDir::new();
    let zero = Instant::now();
    let tick = Arc::new(AtomicU64::new(0));
    let (notice, received) = mpsc::channel();
    let (checked, push_checked) = mpsc::channel();
    let mut preflight = true;
    let (writer, mut ingress) = TrackWriter::start_with_clock(
        dir.config(zero),
        move |_| {
            assert!(std::mem::take(&mut preflight), "injected worker panic");
            Ok(2048 * MIB)
        },
        move |event| {
            notice.send(event).unwrap();
            push_checked.recv_timeout(Duration::from_secs(2)).unwrap();
        },
        {
            let tick = tick.clone();
            move || zero + Duration::from_secs(tick.load(Ordering::Acquire))
        },
    )
    .unwrap();

    assert!(ingress.push(&[0.25; BLOCK], zero));
    tick.store(30, Ordering::Release);
    let CaptureNotice::Fault(fault) = received.recv_timeout(Duration::from_secs(2)).unwrap() else {
        panic!("writer panic did not send a fault");
    };
    assert_eq!(fault.reason, CaptureFaultReason::WorkerPanicked);
    let late_push = ingress.push(&[0.25; BLOCK], zero + Duration::from_secs(30));
    checked.send(()).unwrap();
    assert!(!late_push);
    assert_eq!(writer.finish(), Err(fault));
}

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        let path = parent.join(format!("capture-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn journal(&self) -> PathBuf {
        self.0.join("other.journal")
    }

    fn config(&self, session_zero: Instant) -> TrackConfig {
        TrackConfig {
            path: self.journal(),
            session_zero,
            sample_rate: SAMPLE_RATE,
            epoch: EPOCH,
        }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn sealed_reader_rejects_damage_inside_the_committed_prefix() {
    let mut bytes = Vec::new();
    let mut writer = JournalWriter::new(&mut bytes).unwrap();
    writer.pcm(Offset(0), &[0.25; BLOCK]).unwrap();
    writer.pcm(Offset(BLOCK as u64), &[0.5; BLOCK]).unwrap();
    let seal = read(&bytes, NO_CUTOFF).seal().unwrap();
    let corrupted = bytes.len() - 1;
    bytes[corrupted] ^= 1;

    let path =
        std::env::temp_dir().join(format!("koett-sealed-{}", super::super::MeetingId::new()));
    fs::write(&path, &bytes).unwrap();
    let mut reader = TrackReader::open_sealed(&path, NO_CUTOFF, seal).unwrap();
    fs::remove_file(path).unwrap();
    assert!(matches!(reader.next(), Some(Ok(TrackChunk::Pcm { .. }))));
    assert_eq!(
        reader.next().unwrap().unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn each_track_meters_what_it_receives_and_goes_flat_when_it_stops() {
    let (mut you, _you_drain) = TrackIngress::pair(SAMPLE_RATE, EPOCH);
    let (other, _other_drain) = TrackIngress::pair(SAMPLE_RATE, EPOCH);
    let (you_level, other_level) = (you.level(), other.level());
    let at = Instant::now();
    assert_eq!(you_level.rms(at), 0.0, "nothing received yet");

    assert!(you.push(&[0.5, -0.5, 0.5, -0.5], at));
    assert!((you_level.rms(at + Duration::from_millis(100)) - 0.5).abs() < 1e-6);
    assert_eq!(other_level.rms(at), 0.0, "a silent side stays flat");
    assert_ne!(you_level, other_level);
    assert_eq!(you_level, you.level());

    assert_eq!(
        you_level.rms(at + Duration::from_millis(300)),
        0.0,
        "stalled"
    );
}

#[test]
fn a_closed_track_stops_metering() {
    let (mut ingress, drain) = TrackIngress::pair(SAMPLE_RATE, EPOCH);
    let level: TrackLevel = ingress.level();
    let at = Instant::now();
    assert!(ingress.push(&[0.25; 4], at));
    drain.close();
    assert!(!ingress.push(&[1.0; 4], at + Duration::from_millis(10)));
    assert!((level.rms(at + Duration::from_millis(20)) - 0.25).abs() < 1e-6);
}
