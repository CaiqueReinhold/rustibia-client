use std::time::Duration;

use bevy::prelude::*;

use crate::{
    core::{Cooldowns, SpellId, SpellTarget},
    network::{
        ClientMessage, SendMessage,
        events::{SpellCast, UseCooldownStarted},
    },
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
    mut cooldowns: ResMut<Cooldowns>,
) {
    let now = time.elapsed();
    cooldowns.start_spell(
        event.spell_id,
        now,
        Duration::from_millis(event.spell_cooldown_ms.into()),
    );
    cooldowns.start_group(
        event.group,
        now,
        Duration::from_millis(event.group_cooldown_ms.into()),
    );
}

pub fn on_use_cooldown(
    event: On<UseCooldownStarted>,
    time: Res<Time<Real>>,
    mut cooldowns: ResMut<Cooldowns>,
) {
    cooldowns.start_use(time.elapsed(), Duration::from_millis(event.ms.into()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SpellGroup;
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

    fn a_world_at_five_seconds() -> World {
        let mut world = World::new();
        world.init_resource::<Time<Real>>();
        world.init_resource::<Cooldowns>();
        world.add_observer(on_spell_cast);
        world.add_observer(on_use_cooldown);
        world
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs(5));
        world
    }

    fn cast(world: &mut World, spell: u16, spell_ms: u32, group_ms: u32, group: SpellGroup) {
        world.trigger(SpellCast {
            spell_id: SpellId(spell),
            spell_cooldown_ms: spell_ms,
            group_cooldown_ms: group_ms,
            group,
        });
        world.flush();
    }

    #[test]
    fn an_accepted_cast_starts_the_spell_and_its_group_from_now() {
        let mut world = a_world_at_five_seconds();
        cast(&mut world, 2, 2000, 4000, SpellGroup::Attack);

        let now = Duration::from_secs(6);
        let cooldowns = world.resource::<Cooldowns>();
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
    fn a_rune_cast_starts_the_group_the_server_named() {
        let mut world = a_world_at_five_seconds();
        cast(&mut world, 41, 2000, 2000, SpellGroup::Attack);

        assert!(
            world
                .resource::<Cooldowns>()
                .group(SpellGroup::Attack, Duration::from_secs(5))
                .is_some()
        );
    }

    #[test]
    fn a_use_cooldown_starts_from_now() {
        let mut world = a_world_at_five_seconds();
        world.trigger(UseCooldownStarted { ms: 1000 });
        world.flush();

        assert_eq!(
            world
                .resource::<Cooldowns>()
                .item(Duration::from_millis(5500))
                .map(|state| state.remaining),
            Some(Duration::from_millis(500))
        );
    }
}
