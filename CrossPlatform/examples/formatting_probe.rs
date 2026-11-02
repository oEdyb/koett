use koett_engine::formatting::{Cleaner, Context, Formatting, Structure, Style};
fn main() {
    let mut cleaner =
        Cleaner::prepare(|status| eprintln!("{status}")).expect("prepare verified local model");
    let raw = "um please send the revised invoice for 42 dollars tomorrow not today";
    for style in [
        Style::Casual,
        Style::SemiCasual,
        Style::SemiFormal,
        Style::Formal,
    ] {
        let settings = Formatting {
            enabled: true,
            style,
            structure: Structure::Prose,
            context: Context::General,
        };
        let output = cleaner.clean(raw, settings).expect("real inference");
        println!(
            "{}",
            serde_json::json!({"raw":raw,"settings":settings,"final":output,"checks":{"amount":output.contains("42"),"tomorrow":output.to_lowercase().contains("tomorrow"),"negation":output.to_lowercase().contains("not today")}})
        );
    }
    for (structure, context) in [
        (Structure::Lists, Context::General),
        (Structure::Prose, Context::Email),
    ] {
        let settings = Formatting {
            enabled: true,
            structure,
            context,
            ..Formatting::default()
        };
        let raw = "hi sam please bring apples bananas and milk thanks alex";
        let output = cleaner
            .clean(raw, settings)
            .expect("real structure/context inference");
        println!(
            "{}",
            serde_json::json!({"raw":raw,"settings":settings,"final":output})
        );
    }
    for (id, raw, required) in [
        (
            "negation",
            "do not delete the file move it to the archive instead",
            vec!["do not delete", "archive"],
        ),
        (
            "uncertainty",
            "i think this might cost 42 dollars but i am not completely sure yet",
            vec!["i think", "might", "not completely sure", "42"],
        ),
        (
            "uncertain-name",
            "i think the fictional project was called maple or maybe birch",
            vec!["i think", "maple", "maybe", "birch"],
        ),
    ] {
        let output = cleaner
            .clean(raw, Formatting::default())
            .expect("synthetic meaning fixture inference");
        let lower = output.to_lowercase();
        let preserved = required.iter().all(|value| lower.contains(value));
        println!(
            "{}",
            serde_json::json!({"id":id,"raw":raw,"final":output,"required_present":preserved})
        );
    }
}
