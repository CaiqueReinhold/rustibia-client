use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// A key and the exact modifiers held with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Hotkey {
    pub key: KeyCode,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

const KEY_NAMES: &[(KeyCode, &str)] = &[
    (KeyCode::F1, "F1"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F9, "F9"),
    (KeyCode::F10, "F10"),
    (KeyCode::F11, "F11"),
    (KeyCode::F12, "F12"),
    (KeyCode::Digit0, "0"),
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    (KeyCode::KeyA, "A"),
    (KeyCode::KeyB, "B"),
    (KeyCode::KeyC, "C"),
    (KeyCode::KeyD, "D"),
    (KeyCode::KeyE, "E"),
    (KeyCode::KeyF, "F"),
    (KeyCode::KeyG, "G"),
    (KeyCode::KeyH, "H"),
    (KeyCode::KeyI, "I"),
    (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"),
    (KeyCode::KeyL, "L"),
    (KeyCode::KeyM, "M"),
    (KeyCode::KeyN, "N"),
    (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"),
    (KeyCode::KeyQ, "Q"),
    (KeyCode::KeyR, "R"),
    (KeyCode::KeyS, "S"),
    (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"),
    (KeyCode::KeyV, "V"),
    (KeyCode::KeyW, "W"),
    (KeyCode::KeyX, "X"),
    (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"),
    (KeyCode::Numpad0, "Num0"),
    (KeyCode::Numpad1, "Num1"),
    (KeyCode::Numpad2, "Num2"),
    (KeyCode::Numpad3, "Num3"),
    (KeyCode::Numpad4, "Num4"),
    (KeyCode::Numpad5, "Num5"),
    (KeyCode::Numpad6, "Num6"),
    (KeyCode::Numpad7, "Num7"),
    (KeyCode::Numpad8, "Num8"),
    (KeyCode::Numpad9, "Num9"),
    (KeyCode::Insert, "Ins"),
    (KeyCode::Delete, "Del"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::PageUp, "PgUp"),
    (KeyCode::PageDown, "PgDn"),
];

fn key_name(key: KeyCode) -> Option<&'static str> {
    KEY_NAMES
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, name)| *name)
}

fn key_from_name(name: &str) -> Option<KeyCode> {
    KEY_NAMES
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(key, _)| *key)
}

impl Hotkey {
    pub fn new(key: KeyCode, ctrl: bool, shift: bool, alt: bool) -> Self {
        debug_assert!(
            key_name(key).is_some(),
            "{key:?} is not in KEY_NAMES, so this hotkey would store as \"?\" and fail to parse back"
        );
        Self {
            key,
            ctrl,
            shift,
            alt,
        }
    }

    pub fn plain(key: KeyCode) -> Self {
        Self::new(key, false, false, false)
    }

    /// `key` with the modifiers `keyboard` holds now; `None` for a key no hotkey can use.
    pub fn from_input(key: KeyCode, keyboard: &ButtonInput<KeyCode>) -> Option<Self> {
        key_name(key)?;
        Some(Self::new(
            key,
            keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
            keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            keyboard.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
        ))
    }

    pub fn modifiers(&self) -> (bool, bool, bool) {
        (self.ctrl, self.shift, self.alt)
    }

    pub fn long_label(&self) -> String {
        let mut label = String::new();
        for (held, name) in [
            (self.ctrl, "Ctrl+"),
            (self.shift, "Shift+"),
            (self.alt, "Alt+"),
        ] {
            if held {
                label.push_str(name);
            }
        }
        label.push_str(key_name(self.key).unwrap_or("?"));
        label
    }

    pub fn short_label(&self) -> String {
        let mut label = String::new();
        for (held, letter) in [(self.ctrl, 'C'), (self.shift, 'S'), (self.alt, 'A')] {
            if held {
                label.push(letter);
            }
        }
        label.push_str(key_name(self.key).unwrap_or("?"));
        label
    }

    pub fn parse(label: &str) -> Option<Self> {
        let mut parts: Vec<&str> = label.split('+').collect();
        let mut hotkey = Self::plain(key_from_name(parts.pop()?)?);
        for part in parts {
            match part {
                "Ctrl" => hotkey.ctrl = true,
                "Shift" => hotkey.shift = true,
                "Alt" => hotkey.alt = true,
                _ => return None,
            }
        }
        Some(hotkey)
    }
}

impl From<Hotkey> for String {
    fn from(hotkey: Hotkey) -> Self {
        hotkey.long_label()
    }
}

impl TryFrom<String> for Hotkey {
    type Error = String;

    fn try_from(label: String) -> Result<Self, Self::Error> {
        Hotkey::parse(&label).ok_or_else(|| format!("not a hotkey: {label}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holding(keys: &[KeyCode]) -> ButtonInput<KeyCode> {
        let mut keyboard = ButtonInput::default();
        for key in keys {
            keyboard.press(*key);
        }
        keyboard
    }

    #[test]
    fn both_labels_name_the_modifiers_in_one_order() {
        let hotkey = Hotkey::new(KeyCode::F1, true, true, true);

        assert_eq!(hotkey.long_label(), "Ctrl+Shift+Alt+F1");
        assert_eq!(hotkey.short_label(), "CSAF1");
        assert_eq!(
            Hotkey::new(KeyCode::Digit1, true, false, false).short_label(),
            "C1"
        );
        assert_eq!(Hotkey::plain(KeyCode::Numpad4).long_label(), "Num4");
    }

    #[test]
    fn a_long_label_parses_back_to_its_hotkey() {
        for hotkey in [
            Hotkey::plain(KeyCode::F12),
            Hotkey::new(KeyCode::KeyX, false, true, false),
            Hotkey::new(KeyCode::PageDown, true, false, true),
        ] {
            assert_eq!(Hotkey::parse(&hotkey.long_label()), Some(hotkey));
        }
        assert_eq!(Hotkey::parse("Meta+F1"), None);
        assert_eq!(Hotkey::parse("Ctrl+"), None);
    }

    #[test]
    fn input_takes_the_modifiers_held_on_either_side() {
        let keyboard = holding(&[KeyCode::ShiftRight, KeyCode::ControlLeft, KeyCode::F2]);

        assert_eq!(
            Hotkey::from_input(KeyCode::F2, &keyboard),
            Some(Hotkey::new(KeyCode::F2, true, true, false))
        );
    }

    #[test]
    fn keys_the_modals_and_movement_own_cannot_be_captured() {
        let keyboard = ButtonInput::default();

        for key in [
            KeyCode::Escape,
            KeyCode::Enter,
            KeyCode::ShiftLeft,
            KeyCode::ArrowUp,
        ] {
            assert_eq!(Hotkey::from_input(key, &keyboard), None, "{key:?}");
        }
    }

    #[test]
    fn a_hotkey_is_stored_as_its_long_label() {
        let hotkey = Hotkey::new(KeyCode::F1, true, false, false);

        assert_eq!(serde_json::to_string(&hotkey).unwrap(), "\"Ctrl+F1\"");
        assert_eq!(
            serde_json::from_str::<Hotkey>("\"Ctrl+F1\"").unwrap(),
            hotkey
        );
        assert!(serde_json::from_str::<Hotkey>("\"Hyper+F1\"").is_err());
    }
}
