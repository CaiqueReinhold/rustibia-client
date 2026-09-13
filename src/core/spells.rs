use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;

use crate::{agent::AgentId, map::Position, network::events::SpellListReceived};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct SpellId(pub u16);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpellTarget {
    None,
    Agent(AgentId),
    Position(Position),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpellGroup {
    Attack = 0,
    Healing = 1,
    Support = 2,
}

impl SpellGroup {
    pub const COUNT: usize = 3;
    pub const ALL: [SpellGroup; Self::COUNT] =
        [SpellGroup::Attack, SpellGroup::Healing, SpellGroup::Support];

    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(SpellGroup::Attack),
            1 => Some(SpellGroup::Healing),
            2 => Some(SpellGroup::Support),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpellInfo {
    pub id: SpellId,
    pub name: String,
    pub words: String,
    pub level: u16,
    /// 1-based cell in `ui/spells.png`.
    pub icon: u16,
    pub aimable: bool,
    pub group: SpellGroup,
}

/// `None` until the server's list arrives, which is not the same as an empty list.
#[derive(Resource, Debug, Default)]
pub struct SpellBook(Option<Vec<SpellInfo>>);

impl SpellBook {
    pub fn new(spells: Vec<SpellInfo>) -> Self {
        Self(Some(spells))
    }

    pub fn spells(&self) -> Option<&[SpellInfo]> {
        self.0.as_deref()
    }

    pub fn get(&self, id: SpellId) -> Option<&SpellInfo> {
        self.spells()?.iter().find(|spell| spell.id == id)
    }
}

pub fn on_spell_list(event: On<SpellListReceived>, mut book: ResMut<SpellBook>) {
    *book = SpellBook::new(event.spells.clone());
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CooldownState {
    pub recovered: f32,
    pub remaining: Duration,
}

#[derive(Clone, Copy, Debug)]
struct Cooldown {
    start: Duration,
    length: Duration,
}

impl Cooldown {
    fn state(&self, now: Duration) -> Option<CooldownState> {
        let remaining = (self.start + self.length)
            .checked_sub(now)
            .filter(|remaining| !remaining.is_zero())?;
        Some(CooldownState {
            recovered: 1.0 - remaining.as_secs_f32() / self.length.as_secs_f32(),
            remaining,
        })
    }
}

/// Instants are `Time<Real>::elapsed()`.
#[derive(Resource, Debug, Default)]
pub struct SpellCooldowns {
    spells: HashMap<SpellId, Cooldown>,
    groups: [Option<Cooldown>; SpellGroup::COUNT],
}

impl SpellCooldowns {
    pub fn start_spell(&mut self, id: SpellId, now: Duration, length: Duration) {
        if !length.is_zero() {
            self.spells.insert(id, Cooldown { start: now, length });
        }
    }

    pub fn start_group(&mut self, group: SpellGroup, now: Duration, length: Duration) {
        if !length.is_zero() {
            self.groups[group.index()] = Some(Cooldown { start: now, length });
        }
    }

    pub fn group(&self, group: SpellGroup, now: Duration) -> Option<CooldownState> {
        self.groups[group.index()]?.state(now)
    }

    /// The later-ending of the spell's own cooldown and its group's.
    pub fn spell(&self, id: SpellId, group: SpellGroup, now: Duration) -> Option<CooldownState> {
        let own = self
            .spells
            .get(&id)
            .and_then(|cooldown| cooldown.state(now));
        own.into_iter()
            .chain(self.group(group, now))
            .max_by_key(|state| state.remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: Duration = Duration::from_secs(10);

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn a_cooldown_reports_what_has_recovered_and_what_remains() {
        let mut cooldowns = SpellCooldowns::default();
        cooldowns.start_group(SpellGroup::Attack, START, ms(2000));
        let at = |elapsed| cooldowns.group(SpellGroup::Attack, START + ms(elapsed));

        assert_eq!(
            at(0),
            Some(CooldownState {
                recovered: 0.0,
                remaining: ms(2000)
            })
        );
        assert_eq!(
            at(500),
            Some(CooldownState {
                recovered: 0.25,
                remaining: ms(1500)
            })
        );
        assert_eq!(at(1999).map(|state| state.remaining), Some(ms(1)));
        assert_eq!(at(2000), None);
        assert_eq!(at(2500), None);
        assert_eq!(cooldowns.group(SpellGroup::Healing, START), None);
    }

    #[test]
    fn a_slot_follows_whichever_of_spell_and_group_ends_later() {
        let mut spell_later = SpellCooldowns::default();
        spell_later.start_spell(SpellId(2), START, ms(4000));
        spell_later.start_group(SpellGroup::Attack, START, ms(2000));
        assert_eq!(
            spell_later.spell(SpellId(2), SpellGroup::Attack, START + ms(1000)),
            Some(CooldownState {
                recovered: 0.25,
                remaining: ms(3000)
            })
        );

        let mut group_later = SpellCooldowns::default();
        group_later.start_spell(SpellId(2), START, ms(1500));
        group_later.start_group(SpellGroup::Attack, START, ms(2000));
        assert_eq!(
            group_later.spell(SpellId(2), SpellGroup::Attack, START + ms(1000)),
            Some(CooldownState {
                recovered: 0.5,
                remaining: ms(1000)
            })
        );
    }

    #[test]
    fn a_slot_is_shaded_by_either_cooldown_alone() {
        let mut only_spell = SpellCooldowns::default();
        only_spell.start_spell(SpellId(2), START, ms(2000));
        assert!(
            only_spell
                .spell(SpellId(2), SpellGroup::Attack, START)
                .is_some()
        );
        assert!(
            only_spell
                .spell(SpellId(3), SpellGroup::Attack, START)
                .is_none()
        );

        let mut only_group = SpellCooldowns::default();
        only_group.start_group(SpellGroup::Attack, START, ms(2000));
        assert!(
            only_group
                .spell(SpellId(2), SpellGroup::Attack, START)
                .is_some()
        );
        assert!(
            only_group
                .spell(SpellId(2), SpellGroup::Healing, START)
                .is_none()
        );
    }

    #[test]
    fn a_zero_length_starts_nothing() {
        let mut cooldowns = SpellCooldowns::default();
        cooldowns.start_spell(SpellId(2), START, Duration::ZERO);
        cooldowns.start_group(SpellGroup::Attack, START, Duration::ZERO);

        assert_eq!(cooldowns.spell(SpellId(2), SpellGroup::Attack, START), None);
    }

    #[test]
    fn a_new_cooldown_replaces_the_old_one() {
        let mut cooldowns = SpellCooldowns::default();
        cooldowns.start_group(SpellGroup::Attack, START, ms(2000));
        cooldowns.start_group(SpellGroup::Attack, START + ms(500), ms(1000));

        assert_eq!(
            cooldowns
                .group(SpellGroup::Attack, START + ms(1000))
                .map(|state| state.remaining),
            Some(ms(500))
        );
    }

    fn a_spell(id: u16) -> SpellInfo {
        SpellInfo {
            id: SpellId(id),
            name: format!("Spell {id}"),
            words: format!("words {id}"),
            level: 8,
            icon: 6,
            aimable: false,
            group: SpellGroup::Healing,
        }
    }

    /// The server's `entities/spells.rs` pins the same numbers; nothing links the two.
    #[test]
    fn group_ids_match_the_server() {
        assert_eq!(SpellGroup::from_id(0), Some(SpellGroup::Attack));
        assert_eq!(SpellGroup::from_id(1), Some(SpellGroup::Healing));
        assert_eq!(SpellGroup::from_id(2), Some(SpellGroup::Support));
        assert_eq!(SpellGroup::from_id(3), None);
    }

    #[test]
    fn a_book_that_has_not_arrived_is_not_an_empty_one() {
        assert!(SpellBook::default().spells().is_none());
        assert_eq!(SpellBook::new(Vec::new()).spells(), Some(&[][..]));
    }

    #[test]
    fn the_list_fills_the_book() {
        let mut world = World::new();
        world.init_resource::<SpellBook>();
        world.add_observer(on_spell_list);

        world.trigger(SpellListReceived {
            spells: vec![a_spell(1), a_spell(4)],
        });
        world.flush();

        let book = world.resource::<SpellBook>();
        assert_eq!(book.get(SpellId(4)), Some(&a_spell(4)));
        assert!(book.get(SpellId(2)).is_none());
    }
}
