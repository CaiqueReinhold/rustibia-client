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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpellInfo {
    pub id: SpellId,
    pub name: String,
    pub words: String,
    pub level: u16,
    /// 1-based cell in `ui/spells.png`.
    pub icon: u16,
    pub aimable: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn a_spell(id: u16) -> SpellInfo {
        SpellInfo {
            id: SpellId(id),
            name: format!("Spell {id}"),
            words: format!("words {id}"),
            level: 8,
            icon: 6,
            aimable: false,
        }
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
