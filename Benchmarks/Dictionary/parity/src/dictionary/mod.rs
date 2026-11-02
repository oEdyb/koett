#[derive(Clone, Debug, Default, serde::Deserialize, PartialEq)]
pub struct DictionaryEntry {
    pub term: String,
    pub sounds_like: Vec<String>,
}

#[path = "../../../../../CrossPlatform/src/dictionary/common.rs"]
mod common;
#[path = "../../../../../CrossPlatform/src/dictionary/correct.rs"]
pub mod correct;
#[path = "../../../../../CrossPlatform/src/dictionary/merge.rs"]
mod merge;

pub use common::is_common;
pub use merge::adopt_terms;
