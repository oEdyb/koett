use crate::settings::{PasteShortcut, Settings};
use std::collections::BTreeMap;

pub fn parse(text: &str) -> Result<BTreeMap<String, PasteShortcut>, String> {
    let mut rules = BTreeMap::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let (app, chord) = line
            .split_once('=')
            .ok_or("Use app ID = Ctrl+V or Ctrl+Shift+V")?;
        let app = app.trim();
        if app.is_empty() || app.len() > 256 || rules.len() >= 128 {
            return Err("Use up to 128 nonempty app IDs, each at most 256 bytes".into());
        }
        let chord = match chord.trim() {
            "Ctrl+V" => PasteShortcut::CtrlV,
            "Ctrl+Shift+V" => PasteShortcut::CtrlShiftV,
            _ => return Err("Choose Ctrl+V or Ctrl+Shift+V for each app".into()),
        };
        if rules.insert(app.to_owned(), chord).is_some() {
            return Err(format!("Duplicate app ID: {app}"));
        }
    }
    Ok(rules)
}

pub fn format(rules: &BTreeMap<String, PasteShortcut>) -> String {
    rules
        .iter()
        .map(|(app, chord)| {
            format!(
                "{app} = {}",
                match chord {
                    PasteShortcut::CtrlV => "Ctrl+V",
                    PasteShortcut::CtrlShiftV => "Ctrl+Shift+V",
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn resolve(app: Option<&str>, rules: &BTreeMap<String, PasteShortcut>) -> PasteShortcut {
    app.filter(|app| !app.is_empty())
        .and_then(|app| rules.get(app))
        .copied()
        .unwrap_or(PasteShortcut::CtrlV)
}

pub fn for_target(target: Option<u32>) -> Result<PasteShortcut, String> {
    let settings = Settings::load()?;
    let focused = super::output::focused_identity();
    chord_for_focus(target, focused, &settings.image_paste_shortcuts)
}

fn chord_for_focus(
    target: Option<u32>,
    focused: Option<(u32, String)>,
    rules: &BTreeMap<String, PasteShortcut>,
) -> Result<PasteShortcut, String> {
    if target != Some(super::output::PASTE_NOW) {
        let expected = target.ok_or("target window unavailable")?;
        if focused.as_ref().map(|(id, _)| *id) != Some(expected) {
            return Err("target window changed or unavailable".into());
        }
    }
    Ok(resolve(
        focused.as_ref().map(|(_, app)| app.as_str()),
        rules,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_rules_require_the_same_live_window_and_keep_exact_app_matching() {
        let rules = parse("org.example.Editor = Ctrl+Shift+V").unwrap();
        let focused = Some((7, "org.example.Editor".into()));
        assert_eq!(
            chord_for_focus(Some(7), focused.clone(), &rules),
            Ok(PasteShortcut::CtrlShiftV)
        );
        assert!(chord_for_focus(Some(8), focused, &rules).is_err());
        assert!(chord_for_focus(Some(7), None, &rules).is_err());
        assert!(chord_for_focus(None, None, &rules).is_err());
        assert_eq!(
            chord_for_focus(Some(super::super::output::PASTE_NOW), None, &rules),
            Ok(PasteShortcut::CtrlV)
        );
    }

    #[test]
    fn exact_app_overrides_round_trip_and_unknown_keeps_default() {
        let rules = parse("kitty = Ctrl+V\norg.example.Editor = Ctrl+Shift+V").unwrap();
        assert_eq!(parse(&format(&rules)).unwrap(), rules);
        assert_eq!(
            resolve(Some("org.example.Editor"), &rules),
            PasteShortcut::CtrlShiftV
        );
        assert_eq!(
            resolve(Some("org.example.Editor.preview"), &rules),
            PasteShortcut::CtrlV
        );
        assert_eq!(resolve(None, &rules), PasteShortcut::CtrlV);
        assert!(parse("kitty=Ctrl+V\nkitty=Ctrl+Shift+V").is_err());
        assert!(parse("kitty=Alt+V").is_err());
    }
}
