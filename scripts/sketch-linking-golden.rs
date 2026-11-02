// Writes Tests/KoettTests/Fixtures/sketch-linking-golden.txt from the Linux
// linker so the Swift port can prove byte-for-byte equal bullets.
// Run: rustc --edition 2024 -O -o /tmp/sketch-golden scripts/sketch-linking-golden.rs
//      /tmp/sketch-golden > Tests/KoettTests/Fixtures/sketch-linking-golden.txt
#![allow(dead_code)]
#[path = "../CrossPlatform/src/annotation.rs"]
mod annotation;

use annotation::{TimedWord, estimate_words, keyframe_summary};

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> f32 {
        (self.next() % 10_000) as f32 / 10_000.0
    }
}

const VOCABULARY: &[&str] = &[
    "this", "button", "is", "broken.", "move", "that", "here,", "there", "and", "then",
    "fix", "the", "header;", "why?", "look!", "colour", "“quoted”", "it’s", "(aside)", "wait…",
    "Koett", "Parakeet", "42", "x2", "one:", "two", "três", "naïve", "über,", "end.",
];
const FILLERS: &[&str] = &["um", "uh", "like", "you know,"];

fn hex(text: &str) -> String {
    let bytes: String = text.bytes().map(|byte| format!("{byte:02x}")).collect();
    format!("x{bytes}")
}

fn main() {
    let mut random = Random(0x6b6f657474);
    for case in 0..300 {
        let count = random.below(24) as usize;
        let mut time = random.unit() * 2.0;
        let mut words = Vec::new();
        for _ in 0..count {
            let filler = random.below(6) == 0;
            let text = if filler {
                FILLERS[random.below(FILLERS.len() as u64) as usize]
            } else {
                VOCABULARY[random.below(VOCABULARY.len() as u64) as usize]
            };
            let start = time;
            let end = start + 0.1 + random.unit() * 0.4;
            time = end + if random.below(5) == 0 { 0.5 + random.unit() * 3.0 } else { random.unit() * 0.2 };
            for piece in text.split_whitespace() {
                words.push((TimedWord { text: piece.to_owned(), start, end }, filler));
            }
        }
        let raw_owned: Vec<String> = words.iter().map(|(word, _)| word.text.clone()).collect();
        let raw: Vec<&str> = raw_owned.iter().map(String::as_str).collect();
        let mut output: Vec<String> = match case % 6 {
            0 => raw.iter().map(|word| (*word).to_owned()).collect(),
            1 => words.iter().filter(|(_, filler)| !filler).map(|(word, _)| word.text.clone()).collect(),
            2 => raw.iter().map(|word| word.to_uppercase()).collect(),
            3 => {
                let mut changed: Vec<String> = raw.iter().map(|word| (*word).to_owned()).collect();
                changed.insert(0, "Dear".into());
                changed.insert(1, "team,".into());
                changed.push("Thanks!".into());
                changed
            }
            4 => raw.iter().map(|word| if word.len() > 4 { "replaced".into() } else { (*word).to_owned() }).collect(),
            _ => Vec::new(),
        };
        if case % 17 == 0 {
            output.clear();
        }
        let final_text = output.join(" ");
        let anchor_count = 1 + random.below(5) as usize;
        let mut anchors: Vec<f32> = (0..anchor_count).map(|_| random.unit() * (time + 2.0)).collect();
        anchors.sort_by(f32::total_cmp);
        if case % 11 == 0 && anchors.len() > 1 {
            anchors[1] = anchors[0];
        }
        let timed: Vec<TimedWord> = if case % 13 == 0 {
            estimate_words(&raw.join(" "), 0.0, time)
        } else if case % 29 == 0 {
            Vec::new()
        } else {
            words.into_iter().map(|(word, _)| word).collect()
        };
        println!("case {case}");
        println!("final {}", hex(&final_text));
        for word in &timed {
            println!("word {} {:08x} {:08x}", hex(&word.text), word.start.to_bits(), word.end.to_bits());
        }
        for anchor in &anchors {
            println!("anchor {:08x}", anchor.to_bits());
        }
        println!("expect {}", hex(&keyframe_summary(&final_text, &timed, &anchors)));
        let estimated = estimate_words(&raw.join(" "), 0.25, time);
        for word in &estimated {
            println!("estimate {} {:08x} {:08x}", hex(&word.text), word.start.to_bits(), word.end.to_bits());
        }
        println!("span {:08x} {:08x} {}", 0.25f32.to_bits(), time.to_bits(), hex(&raw.join(" ")));
    }
}
