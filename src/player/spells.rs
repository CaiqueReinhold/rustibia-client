use bevy::prelude::*;

use crate::{
    core::{SpellId, SpellTarget},
    network::{ClientMessage, SendMessage, events::SpellCast},
};

#[derive(Event, Debug)]
pub struct CastSpellRequested {
    pub spell_id: SpellId,
    pub target: SpellTarget,
}

pub fn on_cast_spell_requested(event: On<CastSpellRequested>, mut commands: Commands) {
    commands.trigger(SendMessage(ClientMessage::CastSpell {
        spell_id: event.spell_id,
        target: event.target.clone(),
    }));
}

pub fn on_spell_cast(event: On<SpellCast>) {
    info!(
        "cast accepted: spell {} on cooldown for {}ms, group for {}ms",
        event.spell_id.0, event.spell_cooldown_ms, event.group_cooldown_ms
    );
}

#[cfg(test)]
mod tests {
    use super::*;
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
            }]
        ));
    }
}
