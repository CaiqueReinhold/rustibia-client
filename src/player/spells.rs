use bevy::prelude::*;

use crate::{
    core::{SpellId, SpellTarget},
    network::{ClientMessage, SendMessage, events::SpellCast},
};

#[derive(Event, Debug)]
pub struct CastSpellRequested {
    pub spell_id: SpellId,
}

pub fn on_cast_spell_requested(event: On<CastSpellRequested>, mut commands: Commands) {
    commands.trigger(SendMessage(ClientMessage::CastSpell {
        spell_id: event.spell_id,
        target: SpellTarget::None,
    }));
}

pub fn on_spell_cast(event: On<SpellCast>) {
    info!(
        "cast accepted: spell {} on cooldown for {}ms, group for {}ms",
        event.spell_id.0, event.spell_cooldown_ms, event.group_cooldown_ms
    );
}
