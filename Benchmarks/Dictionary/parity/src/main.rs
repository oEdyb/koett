mod dictionary;
#[path = "../../../../CrossPlatform/src/corrections/learn.rs"] mod learn;

#[derive(serde::Deserialize)]
struct Input { dictionary: Vec<dictionary::DictionaryEntry>, spell: Vec<String>, adopt: Vec<(String, String)>, learn: Vec<(String, String)> }

fn main() {
    let input: Input = serde_json::from_reader(std::io::stdin()).unwrap();
    let spell: Vec<String> = input.spell.iter().map(|t| dictionary::correct::apply(t, &input.dictionary)).collect();
    let adopt: Vec<String> = input.adopt.iter().map(|(p, b)| dictionary::adopt_terms(p, b, &input.dictionary)).collect();
    let known: Vec<String> = input.dictionary.iter().map(|e| e.term.clone()).collect();
    let learn: Vec<Vec<(String, String, usize)>> = input.learn.iter().map(|(a, b)| learn::learn(&[(a.as_str(), b.as_str())], &known).into_iter().map(|s| (s.heard, s.term, s.count)).collect()).collect();
    println!("{}", serde_json::json!({"spell": spell, "adopt": adopt, "learn": learn}));
}
