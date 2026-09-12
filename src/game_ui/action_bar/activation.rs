use bevy::prelude::*;

use crate::{
    agent::AgentId,
    core::{SpellTarget, TextMessageType},
    items::{ItemId, ItemPlacement},
    map::{Map, Position},
    network::events::ShowTextMessage,
    player::{
        InteractionIntent, InteractionMode, TargetingSource, components::Player,
        spells::CastSpellRequested, target::CombatTarget,
    },
};

use super::state::{ActionBar, Aim, SlotAction};

pub const NO_TARGET: &str = "You have no target.";

#[derive(Event, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSlotActivated {
    pub slot: u16,
}

pub struct ActivationContext<'a> {
    pub player: Option<(AgentId, Position)>,
    /// The combat target and the tile it stands on.
    pub target: Option<(AgentId, Position)>,
    pub map: &'a Map,
}

#[derive(Debug)]
pub enum Activation {
    Cast(CastSpellRequested),
    Intent(InteractionIntent),
    Crosshair(TargetingSource),
    Deny(&'static str),
    Nothing,
}

pub fn activation_for(action: Option<SlotAction>, ctx: &ActivationContext) -> Activation {
    let cast = |spell_id, target| Activation::Cast(CastSpellRequested { spell_id, target });
    match action {
        None => Activation::Nothing,
        Some(SlotAction::Spell { id, aim: None }) => cast(id, SpellTarget::None),
        Some(SlotAction::Spell {
            id,
            aim: Some(Aim::Yourself),
        }) => match &ctx.player {
            Some((player, _)) => cast(id, SpellTarget::Agent(*player)),
            None => Activation::Nothing,
        },
        Some(SlotAction::Spell {
            id,
            aim: Some(Aim::Target),
        }) => match &ctx.target {
            Some((target, _)) => cast(id, SpellTarget::Agent(*target)),
            None => Activation::Deny(NO_TARGET),
        },
        Some(SlotAction::Spell {
            id,
            aim: Some(Aim::Crosshair),
        }) => Activation::Crosshair(TargetingSource::Spell(id)),
        Some(SlotAction::Item { item_id, aim: None }) => {
            Activation::Intent(InteractionIntent::UseItem {
                target: ItemPlacement::Carried,
                item_id,
                window_id: None,
            })
        }
        Some(SlotAction::Item {
            item_id,
            aim: Some(Aim::Yourself),
        }) => match &ctx.player {
            Some((player, tile)) => use_with(item_id, *player, tile, ctx.map),
            None => Activation::Nothing,
        },
        Some(SlotAction::Item {
            item_id,
            aim: Some(Aim::Target),
        }) => match &ctx.target {
            Some((target, tile)) => use_with(item_id, *target, tile, ctx.map),
            None => Activation::Deny(NO_TARGET),
        },
        Some(SlotAction::Item {
            item_id,
            aim: Some(Aim::Crosshair),
        }) => Activation::Crosshair(TargetingSource::Item {
            placement: ItemPlacement::Carried,
            item_id,
        }),
    }
}

fn use_with(item_id: ItemId, agent: AgentId, tile: &Position, map: &Map) -> Activation {
    // No item carries id 0, so an empty tile fails the id check in the server's `retrieve_item`
    // rather than matching whatever it may hold that this client has not seen.
    let (target_item_id, index) = map
        .peek_item(tile)
        .map(|(item, index)| (item.config.id, index))
        .unwrap_or((ItemId(0), 0));
    Activation::Intent(InteractionIntent::UseItemWith {
        source: ItemPlacement::Carried,
        source_item_id: item_id,
        target: ItemPlacement::Map {
            position: tile.clone(),
            index,
        },
        target_item_id,
        target_agent: Some(agent),
    })
}

pub(super) fn on_action_slot_activated(
    event: On<ActionSlotActivated>,
    mut commands: Commands,
    bar: Res<ActionBar>,
    map: Res<Map>,
    mut mode: ResMut<InteractionMode>,
    combat_target: Res<CombatTarget>,
    player_q: Query<(&Player, &Position)>,
    position_q: Query<&Position>,
) {
    let player = player_q
        .single()
        .ok()
        .map(|(player, tile)| (player.agent_id, tile.clone()));
    let target = combat_target.target.and_then(|id| {
        let entity = map.get_agent(id)?;
        Some((id, position_q.get(entity).ok()?.clone()))
    });
    let ctx = ActivationContext {
        player,
        target,
        map: &map,
    };

    if mode.is_targeting() {
        *mode = InteractionMode::Idle;
    }
    match activation_for(bar.slot(event.slot).action, &ctx) {
        Activation::Cast(cast) => commands.trigger(cast),
        Activation::Intent(intent) => commands.trigger(intent),
        Activation::Crosshair(source) => *mode = InteractionMode::Targeting(source),
        Activation::Deny(text) => commands.trigger(ShowTextMessage {
            text: text.to_string(),
            message_type: TextMessageType::ActionDenied,
        }),
        Activation::Nothing => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SpellId;
    use crate::items::{Item, ItemConfig};
    use std::sync::Arc;

    const ME: AgentId = AgentId(1);
    const FOE: AgentId = AgentId(2);

    fn here() -> Position {
        Position::new(100, 100, 7)
    }

    fn there() -> Position {
        Position::new(103, 100, 7)
    }

    fn a_map_with_an_item_under_the_foe() -> Map {
        let mut map = Map::default();
        let config = Arc::new(ItemConfig {
            id: ItemId(100),
            flags: Vec::new(),
            friction: Some(150),
            slot: None,
            minimap_color: None,
            elevation: None,
        });
        map.replace_tile(vec![Arc::new(Item::new(config, 1))], &there());
        map
    }

    fn activate(action: SlotAction, target: Option<(AgentId, Position)>, map: &Map) -> Activation {
        activation_for(
            Some(action),
            &ActivationContext {
                player: Some((ME, here())),
                target,
                map,
            },
        )
    }

    fn spell(aim: Option<Aim>) -> SlotAction {
        SlotAction::Spell {
            id: SpellId(4),
            aim,
        }
    }

    fn item(aim: Option<Aim>) -> SlotAction {
        SlotAction::Item {
            item_id: ItemId(266),
            aim,
        }
    }

    #[test]
    fn an_empty_slot_does_nothing() {
        let map = Map::default();
        let ctx = ActivationContext {
            player: Some((ME, here())),
            target: None,
            map: &map,
        };

        assert!(matches!(activation_for(None, &ctx), Activation::Nothing));
    }

    #[test]
    fn a_spell_casts_at_what_its_aim_names() {
        let map = Map::default();
        let foe = Some((FOE, there()));

        assert!(matches!(
            activate(spell(None), foe.clone(), &map),
            Activation::Cast(CastSpellRequested {
                spell_id: SpellId(4),
                target: SpellTarget::None
            })
        ));
        assert!(matches!(
            activate(spell(Some(Aim::Yourself)), foe.clone(), &map),
            Activation::Cast(CastSpellRequested {
                target: SpellTarget::Agent(ME),
                ..
            })
        ));
        assert!(matches!(
            activate(spell(Some(Aim::Target)), foe, &map),
            Activation::Cast(CastSpellRequested {
                target: SpellTarget::Agent(FOE),
                ..
            })
        ));
        assert!(matches!(
            activate(spell(Some(Aim::Crosshair)), None, &map),
            Activation::Crosshair(TargetingSource::Spell(SpellId(4)))
        ));
    }

    #[test]
    fn aiming_at_a_target_with_none_is_denied() {
        let map = Map::default();

        assert!(matches!(
            activate(spell(Some(Aim::Target)), None, &map),
            Activation::Deny(NO_TARGET)
        ));
        assert!(matches!(
            activate(item(Some(Aim::Target)), None, &map),
            Activation::Deny(NO_TARGET)
        ));
    }

    #[test]
    fn a_plain_item_use_names_no_placement_of_its_own() {
        let map = Map::default();

        assert!(matches!(
            activate(item(None), None, &map),
            Activation::Intent(InteractionIntent::UseItem {
                target: ItemPlacement::Carried,
                item_id: ItemId(266),
                window_id: None,
            })
        ));
    }

    #[test]
    fn an_item_aimed_at_the_target_uses_the_top_of_its_tile() {
        let map = a_map_with_an_item_under_the_foe();

        let Activation::Intent(InteractionIntent::UseItemWith {
            source,
            source_item_id,
            target: ItemPlacement::Map { position, index },
            target_item_id,
            target_agent,
        }) = activate(item(Some(Aim::Target)), Some((FOE, there())), &map)
        else {
            panic!("expected a use-with on a map tile");
        };

        assert!(matches!(source, ItemPlacement::Carried));
        assert_eq!(source_item_id, ItemId(266));
        assert_eq!((position, index), (there(), 0));
        assert_eq!(target_item_id, ItemId(100));
        assert_eq!(target_agent, Some(FOE));
    }

    #[test]
    fn an_item_aimed_at_yourself_names_you() {
        let map = Map::default();

        assert!(matches!(
            activate(item(Some(Aim::Yourself)), None, &map),
            Activation::Intent(InteractionIntent::UseItemWith {
                target_agent: Some(ME),
                ..
            })
        ));
    }

    #[test]
    fn an_item_with_a_crosshair_enters_it_with_the_carried_source() {
        let map = Map::default();

        assert!(matches!(
            activate(item(Some(Aim::Crosshair)), None, &map),
            Activation::Crosshair(TargetingSource::Item {
                placement: ItemPlacement::Carried,
                item_id: ItemId(266),
            })
        ));
    }
}
