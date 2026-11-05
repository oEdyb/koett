#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use std::os::raw::c_char;

pub const SHARED_CONTEXT_AVAILABLE: bool = cfg!(koett_shared_context);
pub const SHARED_CONTEXT_SHA256: Option<&str> = if SHARED_CONTEXT_AVAILABLE {
    Some("1200f3e6b686eb3a47cc5982c906ba0b0f0b7f9b0e3e69d987556c85c00137f3")
} else {
    None
};

extern "C" {
    pub fn SherpaOnnxGetVersionStr() -> *const c_char;
    pub fn SherpaOnnxGetGitSha1() -> *const c_char;
    pub fn SherpaOnnxGetGitDate() -> *const c_char;
    pub fn SherpaOnnxGetOnnxruntimeVersionStr() -> *const c_char;
    pub fn SherpaOnnxFileExists(filename: *const c_char) -> i32;
}

pub mod audio_tagging;
pub mod kws;
pub mod offline_asr;
pub mod offline_punctuation;
pub mod offline_speaker_diarization;
pub mod online_asr;
pub mod online_punctuation;
pub mod resampler;
pub mod speaker_embedding;
pub mod speech_denoiser;
pub mod spoken_language_identification;
pub mod tts;
pub mod vad;
pub mod wave;

pub use audio_tagging::*;
pub use kws::*;
pub use offline_asr::*;
pub use offline_punctuation::*;
pub use offline_speaker_diarization::*;
pub use online_asr::*;
pub use online_punctuation::*;
pub use resampler::*;
pub use speaker_embedding::*;
pub use speech_denoiser::*;
pub use spoken_language_identification::*;
pub use tts::*;
pub use vad::*;
pub use wave::*;
