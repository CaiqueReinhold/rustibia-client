/// Ids assigned by the server, which stores them in `player_skills.skill_type`
/// as well as sending them. Explicit, so reordering the variants cannot change
/// one; the matching assertion is in the server's `entities/skills.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkillType {
    Level = 0,
    Axe = 1,
    Club = 2,
    Sword = 3,
    Distance = 4,
    Magic = 5,
    Shielding = 6,
}

impl SkillType {
    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(SkillType::Level),
            1 => Some(SkillType::Axe),
            2 => Some(SkillType::Club),
            3 => Some(SkillType::Sword),
            4 => Some(SkillType::Distance),
            5 => Some(SkillType::Magic),
            6 => Some(SkillType::Shielding),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SkillType::Level => "Level",
            SkillType::Axe => "Axe",
            SkillType::Club => "Club",
            SkillType::Sword => "Sword",
            SkillType::Distance => "Distance",
            SkillType::Magic => "Magic Level",
            SkillType::Shielding => "Shielding",
        }
    }
}

/// Top to bottom in the window. Not id order: this is what a player expects to
/// read, and a skill missing from the server's snapshot draws no row at all.
pub const DISPLAY_ORDER: [SkillType; 7] = [
    SkillType::Level,
    SkillType::Magic,
    SkillType::Sword,
    SkillType::Axe,
    SkillType::Club,
    SkillType::Distance,
    SkillType::Shielding,
];

/// `percent_bp` is hundredths of a percent, `0..=10_000`, toward the next `level`; `bonus` is
/// what worn equipment adds on top and does not move the bar.
#[derive(Clone, Copy, Debug)]
pub struct SkillProgress {
    pub level: u16,
    pub percent_bp: u16,
    pub bonus: i16,
}

impl SkillProgress {
    pub fn ratio(&self) -> f32 {
        self.percent_bp as f32 / 10_000.0
    }

    pub fn shown_level(&self) -> u16 {
        self.level.saturating_add_signed(self.bonus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the discriminants the enum's own comment promises are fixed.
    #[test]
    fn the_wire_ids_match_the_server() {
        assert_eq!(SkillType::Level as u8, 0);
        assert_eq!(SkillType::Axe as u8, 1);
        assert_eq!(SkillType::Club as u8, 2);
        assert_eq!(SkillType::Sword as u8, 3);
        assert_eq!(SkillType::Distance as u8, 4);
        assert_eq!(SkillType::Magic as u8, 5);
        assert_eq!(SkillType::Shielding as u8, 6);
    }

    #[test]
    fn every_id_round_trips_and_unknown_ids_are_rejected() {
        for skill in DISPLAY_ORDER {
            assert_eq!(SkillType::from_id(skill as u8), Some(skill));
        }
        assert_eq!(SkillType::from_id(7), None);
    }

    #[test]
    fn the_shown_level_adds_the_bonus_and_stops_at_zero() {
        let at = |level, bonus| SkillProgress {
            level,
            percent_bp: 0,
            bonus,
        };
        assert_eq!(at(12, 4).shown_level(), 16);
        assert_eq!(at(12, -3).shown_level(), 9);
        assert_eq!(at(5, -10).shown_level(), 0);
    }

    #[test]
    fn a_ratio_is_the_bar_width() {
        assert_eq!(
            SkillProgress {
                level: 1,
                percent_bp: 0,
                bonus: 0,
            }
            .ratio(),
            0.0
        );
        assert_eq!(
            SkillProgress {
                level: 1,
                percent_bp: 5_000,
                bonus: 0,
            }
            .ratio(),
            0.5
        );
        assert_eq!(
            SkillProgress {
                level: 1,
                percent_bp: 10_000,
                bonus: 0,
            }
            .ratio(),
            1.0
        );
    }
}
