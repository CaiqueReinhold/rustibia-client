pub trait Capitalize: AsRef<str> {
    fn capitalize_words(&self) -> String;
}

impl<T: AsRef<str>> Capitalize for T {
    fn capitalize_words(&self) -> String {
        let text = self.as_ref();
        let mut out = String::with_capacity(text.len());
        for (i, word) in text.split(' ').enumerate() {
            if i > 0 {
                out.push(' ');
            }
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.extend(chars.flat_map(char::to_lowercase));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::Capitalize;

    #[test]
    fn capitalizes_each_word() {
        assert_eq!("magic level".capitalize_words(), "Magic Level");
    }

    #[test]
    fn lowercases_the_rest_of_each_word() {
        assert_eq!("mAGIC lEVEL".capitalize_words(), "Magic Level");
    }

    #[test]
    fn single_word() {
        assert_eq!("sword".capitalize_words(), "Sword");
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!("".capitalize_words(), "");
    }

    #[test]
    fn keeps_every_space() {
        assert_eq!("  two   handed ".capitalize_words(), "  Two   Handed ");
        assert_eq!("   ".capitalize_words(), "   ");
    }

    #[test]
    fn only_spaces_separate_words() {
        assert_eq!("two-handed\tsword".capitalize_words(), "Two-handed\tsword");
    }

    #[test]
    fn leaves_non_letters_alone() {
        assert_eq!("2 gold coins".capitalize_words(), "2 Gold Coins");
    }

    #[test]
    fn handles_non_ascii_and_multi_char_case_mappings() {
        assert_eq!("água élfica".capitalize_words(), "Água Élfica");
        assert_eq!("ßtone".capitalize_words(), "SStone");
    }

    #[test]
    fn works_on_owned_strings() {
        assert_eq!(
            String::from("fist fighting").capitalize_words(),
            "Fist Fighting"
        );
    }
}
