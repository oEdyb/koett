use super::*;

// Labels specify speech ownership, independently of either mapping algorithm.
// Starts are recording seconds; each spoken word lasts at most 350 ms.
#[test]
fn labelled_linking_corpus() {
    type Scenario<'a> = (
        &'a str,
        &'a str,
        &'a str,
        &'a [f32],
        &'a [f32],
        &'a [&'a str],
    );
    let cases: &[Scenario<'_>] = &[
        (
            "draw then speak",
            "this button is broken",
            "this button is broken",
            &[1., 1.3, 1.6, 1.9],
            &[0.2],
            &["this button is broken"],
        ),
        (
            "speak then draw",
            "this button",
            "this button",
            &[0., 0.4],
            &[0.8],
            &["this button"],
        ),
        (
            "mid sentence",
            "move this button left",
            "move this button left",
            &[0., 0.3, 0.6, 0.9],
            &[0.5],
            &["move this button left"],
        ),
        (
            "compare",
            "compare this with that",
            "compare this with that",
            &[0., 0.3, 0.6, 0.9],
            &[0.35, 0.95],
            &["compare this with", "that"],
        ),
        (
            "long pause",
            "fix here then there",
            "fix here then there",
            &[0., 0.3, 4., 4.3],
            &[0.1, 4.1],
            &["fix here", "then there"],
        ),
        (
            "after last",
            "done.",
            "done.",
            &[0.],
            &[0.1, 3.],
            &["done.", ""],
        ),
        (
            "streaming",
            "one two three",
            "one two three",
            &[0., 0.3, 0.6],
            &[0.1, 0.4],
            &["one", "two three"],
        ),
        (
            "comma",
            "first, second item",
            "first, second item",
            &[0., 1., 1.3],
            &[0.1, 1.1],
            &["first,", "second item"],
        ),
        (
            "question",
            "why? move it",
            "why? move it",
            &[0., 1., 1.3],
            &[0.1, 1.1],
            &["why?", "move it"],
        ),
        ("empty", "", "", &[], &[1.], &[""]),
        (
            "estimated",
            "move this. fix that.",
            "move this. fix that.",
            &[],
            &[0.1, 2.5],
            &["move this.", "fix that."],
        ),
        (
            "leading fillers",
            "um uh um fix this. move that.",
            "Fix this. Move that.",
            &[0., 0.2, 0.4, 0.6, 0.9, 3., 3.3],
            &[0.7, 3.1],
            &["Fix this.", "Move that."],
        ),
        (
            "trailing fillers",
            "fix this. move that uh um uh",
            "Fix this. Move that",
            &[0., 0.3, 3., 3.3, 3.6, 3.9, 4.2],
            &[0.1, 3.1],
            &["Fix this.", "Move that"],
        ),
        (
            "middle fillers",
            "fix this. um uh um move that.",
            "Fix this. Move that.",
            &[0., 0.3, 2., 2.2, 2.4, 2.6, 2.9],
            &[0.1, 2.1],
            &["Fix this.", "Move that."],
        ),
        (
            "replacement",
            "fix red. move blue.",
            "Fix crimson. Move blue.",
            &[0., 0.3, 3., 3.3],
            &[0.1, 3.1],
            &["Fix crimson.", "Move blue."],
        ),
        (
            "three drawings",
            "one two three",
            "one two three",
            &[0., 0.3, 0.6],
            &[0.1, 0.4, 0.7],
            &["one", "two", "three"],
        ),
    ];
    for &(name, raw, final_text, starts, anchors, labels) in cases {
        let words = if starts.is_empty() {
            estimate_words(raw, 0., 4.)
        } else {
            raw.split_whitespace()
                .zip(starts)
                .map(|(text, &start)| TimedWord {
                    text: text.into(),
                    start,
                    end: start + 0.25,
                })
                .collect()
        };
        let expected = labels
            .iter()
            .zip(anchors)
            .enumerate()
            .map(|(i, (label, &a))| {
                let tail = if label.is_empty() {
                    "marked".into()
                } else {
                    format!("“{label}”")
                };
                format!("- #{} ({}) {tail}", i + 1, clock(a))
            })
            .collect::<Vec<_>>()
            .join("\n");
        let output = keyframe_summary(final_text, &words, anchors);
        assert_eq!(output, expected, "{name}");
    }
}
