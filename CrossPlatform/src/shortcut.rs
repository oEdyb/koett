use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Space,
    Letter(char),
    Number(u8),
    Function(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Shortcut {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl Shortcut {
    pub fn new(modifiers: Modifiers, key: Key) -> Result<Self, String> {
        if !modifiers.control && !modifiers.alt && !modifiers.shift && !modifiers.super_key {
            return Err("a shortcut needs at least one modifier".to_string());
        }
        if key == Key::Function(12) {
            return Err("F12 is reserved by Windows and cannot be used".to_string());
        }
        Ok(Self { modifiers, key })
    }

    pub fn portal_trigger(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.control {
            parts.push("CTRL".to_string());
        }
        if self.modifiers.alt {
            parts.push("ALT".to_string());
        }
        if self.modifiers.shift {
            parts.push("SHIFT".to_string());
        }
        if self.modifiers.super_key {
            parts.push("LOGO".to_string());
        }
        parts.push(match self.key {
            Key::Space => "space".to_string(),
            Key::Letter(letter) => letter.to_ascii_lowercase().to_string(),
            Key::Number(number) => number.to_string(),
            Key::Function(number) => format!("F{number}"),
        });
        parts.join("+")
    }
}

impl FromStr for Shortcut {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut modifiers = Modifiers::default();
        let mut key = None;
        for part in value
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.control = true,
                "alt" | "option" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "super" | "win" | "meta" => modifiers.super_key = true,
                name if key.is_none() => key = Some(parse_key(name)?),
                _ => return Err(format!("shortcut has more than one key: {value}")),
            }
        }
        Shortcut::new(
            modifiers,
            key.ok_or_else(|| "shortcut is missing its key".to_string())?,
        )
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.modifiers.control {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.alt {
            parts.push("Alt".to_string());
        }
        if self.modifiers.shift {
            parts.push("Shift".to_string());
        }
        if self.modifiers.super_key {
            parts.push("Super".to_string());
        }
        parts.push(match self.key {
            Key::Space => "Space".to_string(),
            Key::Letter(letter) => letter.to_ascii_uppercase().to_string(),
            Key::Number(number) => number.to_string(),
            Key::Function(number) => format!("F{number}"),
        });
        write!(output, "{}", parts.join("+"))
    }
}

fn parse_key(name: &str) -> Result<Key, String> {
    if name == "space" {
        return Ok(Key::Space);
    }
    if let Some(number) = name.strip_prefix('f').and_then(|value| value.parse().ok())
        && (1..=24).contains(&number)
    {
        return Ok(Key::Function(number));
    }
    if name.len() == 1 {
        let character = name.chars().next().expect("one-byte key has one character");
        if character.is_ascii_alphabetic() {
            return Ok(Key::Letter(character.to_ascii_uppercase()));
        }
        if let Some(number) = character.to_digit(10) {
            return Ok(Key::Number(number as u8));
        }
    }
    Err(format!("unsupported shortcut key: {name}"))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{Key, Shortcut};

    #[test]
    fn default_shortcut_round_trips() {
        let shortcut = Shortcut::from_str("Ctrl+Shift+Space").unwrap();
        assert_eq!(shortcut.key, Key::Space);
        assert_eq!(shortcut.to_string(), "Ctrl+Shift+Space");
    }

    #[test]
    fn shortcut_needs_a_modifier() {
        assert!(Shortcut::from_str("Space").is_err());
    }

    #[test]
    fn shortcut_rejects_windows_reserved_f12() {
        assert!(Shortcut::from_str("Ctrl+F12").is_err());
    }

    #[test]
    fn shortcut_accepts_cross_platform_modifier_names() {
        assert_eq!(
            Shortcut::from_str("Win+Alt+K").unwrap().to_string(),
            "Alt+Super+K"
        );
    }

    #[test]
    fn shortcut_builds_an_xdg_preferred_trigger() {
        assert_eq!(
            Shortcut::from_str("Ctrl+Shift+Space")
                .unwrap()
                .portal_trigger(),
            "CTRL+SHIFT+space"
        );
    }
}
