use std::time::Duration;

use bevy::prelude::*;

use crate::{
    core::{SpellBook, SpellCooldowns, SpellId, SpellTarget},
    network::{ClientMessage, SendMessage, events::SpellCast},
};

#[derive(Event, Debug)]
pub struct CastSpellRequested {
    pub spell_id: SpellId,
    pub target: SpellTarget,
    pub param: Option<String>,
}

pub fn on_cast_spell_requested(event: On<CastSpellRequested>, mut commands: Commands) {
    commands.trigger(SendMessage(ClientMessage::CastSpell {
        spell_id: event.spell_id,
        target: event.target.clone(),
        param: event.param.clone(),
    }));
}

pub fn on_spell_cast(
    event: On<SpellCast>,
    time: Res<Time<Real>>,
    book: Res<SpellBook>,
    mut cooldowns: ResMut<SpellCooldowns>,
) {
    let now = time.elapsed();
    cooldowns.start_spell(
        event.spell_id,
        now,
        Duration::from_millis(event.spell_cooldown_ms.into()),
    );
    match book.get(event.spell_id) {
        Some(spell) => cooldowns.start_group(
            spell.group,
            now,
            Duration::from_millis(event.group_cooldown_ms.into()),
        ),
        None => warn!(
            "cast accepted for spell {}, which is not in the spell book; its group cooldown is not shown",
            event.spell_id.0
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{SpellGroup, SpellInfo};
    use crate::map::Position;

    #[derive(Resource, Default)]
    struct Sent(Vec<ClientMessage>);

    #[test]
    fn the_request_is_sent_with_its_target() {
        let mut world = World::new();
        world.init_resource::<Sent>();
        world.add_observer(on_cast_spell_requested);
        world.add_observer(|event: On<SendMessage>, mut sent: ResMut<Sent>| {
            sent.0.push(event.0.clone());
        });

        world.trigger(CastSpellRequested {
            spell_id: SpellId(4),
            target: SpellTarget::Position(Position::new(100, 101, 7)),
            param: None,
        });
        world.flush();

        assert!(matches!(
            world.resource::<Sent>().0.as_slice(),
            [ClientMessage::CastSpell {
                spell_id: SpellId(4),
                target: SpellTarget::Position(Position {
                    x: 100,
                    y: 101,
                    z: 7
                }),
                param: None,
            }]
        ));
    }

    fn a_world_knowing_spell_two() -> World {
        let mut world = World::new();
        world.init_resource::<Time<Real>>();
        world.init_resource::<SpellCooldowns>();
        world.insert_resource(SpellBook::new(vec![SpellInfo {
            id: SpellId(2),
            name: "Fire Wave".to_owned(),
            words: "exevo flam hur".to_owned(),
            level: 18,
            icon: 44,
            aimable: false,
            group: SpellGroup::Attack,
        }]));
        world.add_observer(on_spell_cast);
        world
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs(5));
        world
    }

    fn cast(world: &mut World, spell: u16, spell_ms: u32, group_ms: u32) {
        world.trigger(SpellCast {
            spell_id: SpellId(spell),
            spell_cooldown_ms: spell_ms,
            group_cooldown_ms: group_ms,
        });
        world.flush();
    }

    #[test]
    fn an_accepted_cast_starts_the_spell_and_its_group_from_now() {
        let mut world = a_world_knowing_spell_two();
        cast(&mut world, 2, 2000, 4000);

        let now = Duration::from_secs(6);
        let cooldowns = world.resource::<SpellCooldowns>();
        assert_eq!(
            cooldowns
                .spell(SpellId(2), SpellGroup::Healing, now)
                .map(|state| state.remaining),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            cooldowns
                .group(SpellGroup::Attack, now)
                .map(|state| state.remaining),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn a_spell_missing_from_the_book_starts_only_its_own_cooldown() {
        let mut world = a_world_knowing_spell_two();
        cast(&mut world, 9, 2000, 4000);

        let now = Duration::from_secs(5);
        let cooldowns = world.resource::<SpellCooldowns>();
        assert!(
            cooldowns
                .spell(SpellId(9), SpellGroup::Healing, now)
                .is_some()
        );
        for group in SpellGroup::ALL {
            assert!(cooldowns.group(group, now).is_none(), "{group:?}");
        }
    }
}
