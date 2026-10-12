use bevy::prelude::*;
use bevy_ui_text_input::clipboard::Clipboard;

use crate::{
    agent::{Agent, AgentId, AgentKind},
    game_ui::{ContextMenu, ContextMenuEntry, ContextMenuPicked, GameUiAssets, WindowId},
    items::{ItemId, ItemPlacement},
    map::Map,
    network::{ClientMessage, SendMessage, events::RemoveAgent},
    player::target::{CombatTarget, TargetSquare, refresh_target_square},
};

use super::hover::CursorTarget;
use super::intent::InteractionIntent;
use super::mode::InteractionMode;
use super::use_kind::{UseKind, start_use, use_kind};

#[derive(Debug, Clone)]
pub struct MenuAgent {
    pub id: AgentId,
    pub name: String,
    pub kind: AgentKind,
    pub is_self: bool,
}

impl MenuAgent {
    pub(super) fn of(agent: &Agent, self_id: Option<AgentId>) -> Self {
        Self {
            id: agent.agent_id,
            name: agent.name.clone(),
            kind: agent.kind,
            is_self: Some(agent.agent_id) == self_id,
        }
    }
}

/// What was under the cursor when the menu opened. A pick acts on this, not on what is there
/// by then.
#[derive(Debug, Clone)]
pub struct ThingMenuTarget {
    pub placement: ItemPlacement,
    pub item_id: ItemId,
    pub use_kind: Option<UseKind>,
    pub window_id: Option<WindowId>,
    pub agent: Option<MenuAgent>,
}

impl ThingMenuTarget {
    pub(super) fn new(cursor: &CursorTarget, agent: Option<MenuAgent>) -> Self {
        Self {
            placement: cursor.placement.clone(),
            item_id: cursor.item.config.id,
            use_kind: use_kind(&cursor.item.config),
            window_id: cursor.window_id,
            agent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThingAction {
    Look,
    Use(UseKind),
    Attack(AgentId),
    StopAttack(AgentId),
    MessageTo(String),
    CopyName(String),
}

impl ThingAction {
    fn label(&self) -> String {
        match self {
            ThingAction::Look => "Look".to_string(),
            ThingAction::Use(UseKind::UseWith) => "Use with ...".to_string(),
            ThingAction::Use(UseKind::Open) => "Open".to_string(),
            ThingAction::Use(UseKind::Use) => "Use".to_string(),
            ThingAction::Attack(_) => "Attack".to_string(),
            ThingAction::StopAttack(_) => "Stop Attack".to_string(),
            ThingAction::MessageTo(name) => format!("Message to {name}"),
            ThingAction::CopyName(_) => "Copy Name".to_string(),
        }
    }
}

/// The menu top to bottom; `None` is a separator.
fn thing_menu_rows(
    target: &ThingMenuTarget,
    current_target: Option<AgentId>,
) -> Vec<Option<ThingAction>> {
    let mut rows = vec![Some(ThingAction::Look)];
    rows.extend(target.use_kind.map(|kind| Some(ThingAction::Use(kind))));

    let Some(agent) = &target.agent else {
        return rows;
    };
    rows.push(None);
    let before_agent_rows = rows.len();

    if current_target == Some(agent.id) {
        rows.push(Some(ThingAction::StopAttack(agent.id)));
    } else if !agent.is_self && agent.kind != AgentKind::Npc {
        rows.push(Some(ThingAction::Attack(agent.id)));
    }
    if !agent.is_self && agent.kind == AgentKind::Player {
        rows.push(Some(ThingAction::MessageTo(agent.name.clone())));
    }
    if rows.len() > before_agent_rows {
        rows.push(None);
    }
    rows.push(Some(ThingAction::CopyName(agent.name.clone())));
    rows
}

#[derive(Event, Debug, Clone)]
pub struct OpenThingMenu {
    pub target: ThingMenuTarget,
    /// A logical cursor position.
    pub at: Vec2,
}

#[derive(Event, Debug, Clone)]
pub struct CopyToClipboard(pub String);

#[derive(Component, Debug)]
pub struct ThingMenu {
    target: ThingMenuTarget,
    rows: Vec<Option<ThingAction>>,
}

pub fn on_open_thing_menu(
    event: On<OpenThingMenu>,
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    combat_target: Res<CombatTarget>,
) {
    let rows = thing_menu_rows(&event.target, combat_target.target);
    let entries: Vec<ContextMenuEntry> = rows
        .iter()
        .map(|row| match row {
            Some(action) => ContextMenuEntry::new(action.label(), true),
            None => ContextMenuEntry::separator(),
        })
        .collect();
    let menu = ContextMenu::new(entries).spawn(&mut commands, &ui_assets, event.at);
    commands.entity(menu).insert(ThingMenu {
        target: event.target.clone(),
        rows,
    });
}

pub fn on_thing_menu_picked(
    event: On<ContextMenuPicked>,
    mut commands: Commands,
    menus: Query<&ThingMenu>,
    mut mode: ResMut<InteractionMode>,
    mut combat_target: ResMut<CombatTarget>,
    map: Res<Map>,
    square_q: Query<Entity, With<TargetSquare>>,
) {
    let Ok(menu) = menus.get(event.menu) else {
        return;
    };
    let Some(Some(action)) = menu.rows.get(event.index) else {
        return;
    };
    let target = &menu.target;
    match action {
        ThingAction::Look => {
            commands.trigger(InteractionIntent::Look(target.placement.clone()));
        }
        ThingAction::Use(kind) => start_use(
            *kind,
            target.placement.clone(),
            target.item_id,
            target.window_id,
            &mut commands,
            &mut mode,
        ),
        ThingAction::Attack(agent_id) => {
            let seq = combat_target.set(*agent_id);
            refresh_target_square(&mut commands, &combat_target, &map, &square_q);
            commands.trigger(InteractionIntent::SetTarget(Some(*agent_id), seq));
        }
        ThingAction::StopAttack(agent_id) => {
            if combat_target.target != Some(*agent_id) {
                return;
            }
            let seq = combat_target.clear_locally();
            refresh_target_square(&mut commands, &combat_target, &map, &square_q);
            commands.trigger(InteractionIntent::SetTarget(None, seq));
        }
        ThingAction::MessageTo(name) => {
            commands.trigger(SendMessage(ClientMessage::OpenPmChat {
                name: name.clone(),
            }));
        }
        ThingAction::CopyName(name) => {
            commands.trigger(CopyToClipboard(name.clone()));
        }
    }
}

pub fn close_thing_menu_when_its_agent_leaves(
    event: On<RemoveAgent>,
    menus: Query<(Entity, &ThingMenu)>,
    mut commands: Commands,
) {
    for (entity, menu) in &menus {
        if menu
            .target
            .agent
            .as_ref()
            .is_some_and(|agent| agent.id == event.agent_id)
        {
            commands.entity(entity).despawn();
        }
    }
}

pub fn on_copy_to_clipboard(event: On<CopyToClipboard>, mut clipboard: ResMut<Clipboard>) {
    if let Err(error) = clipboard.set_text(event.0.as_str()) {
        warn!("could not copy to the clipboard: {error:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Position;

    fn a_target(use_kind: Option<UseKind>, agent: Option<MenuAgent>) -> ThingMenuTarget {
        ThingMenuTarget {
            placement: ItemPlacement::Map {
                position: Position::new(100, 100, 7),
                index: 0,
            },
            item_id: ItemId(100),
            use_kind,
            window_id: None,
            agent,
        }
    }

    fn an_agent(kind: AgentKind, is_self: bool) -> Option<MenuAgent> {
        Some(MenuAgent {
            id: AgentId(7),
            name: "Bob".to_string(),
            kind,
            is_self,
        })
    }

    fn name() -> String {
        "Bob".to_string()
    }

    #[test]
    fn a_bare_tile_offers_only_a_look() {
        assert_eq!(
            thing_menu_rows(&a_target(None, None), None),
            vec![Some(ThingAction::Look)]
        );
    }

    #[test]
    fn an_item_offers_the_one_use_it_has() {
        for kind in [UseKind::UseWith, UseKind::Open, UseKind::Use] {
            assert_eq!(
                thing_menu_rows(&a_target(Some(kind), None), None),
                vec![Some(ThingAction::Look), Some(ThingAction::Use(kind))]
            );
        }
    }

    #[test]
    fn a_creature_can_be_attacked_and_named() {
        assert_eq!(
            thing_menu_rows(&a_target(None, an_agent(AgentKind::Creature, false)), None),
            vec![
                Some(ThingAction::Look),
                None,
                Some(ThingAction::Attack(AgentId(7))),
                None,
                Some(ThingAction::CopyName(name())),
            ]
        );
    }

    #[test]
    fn the_current_target_offers_to_stop() {
        assert_eq!(
            thing_menu_rows(
                &a_target(None, an_agent(AgentKind::Creature, false)),
                Some(AgentId(7))
            ),
            vec![
                Some(ThingAction::Look),
                None,
                Some(ThingAction::StopAttack(AgentId(7))),
                None,
                Some(ThingAction::CopyName(name())),
            ]
        );
    }

    #[test]
    fn targeting_someone_else_still_offers_attack() {
        let rows = thing_menu_rows(
            &a_target(None, an_agent(AgentKind::Creature, false)),
            Some(AgentId(9)),
        );

        assert!(rows.contains(&Some(ThingAction::Attack(AgentId(7)))));
    }

    #[test]
    fn another_player_can_also_be_messaged() {
        assert_eq!(
            thing_menu_rows(&a_target(None, an_agent(AgentKind::Player, false)), None),
            vec![
                Some(ThingAction::Look),
                None,
                Some(ThingAction::Attack(AgentId(7))),
                Some(ThingAction::MessageTo(name())),
                None,
                Some(ThingAction::CopyName(name())),
            ]
        );
    }

    #[test]
    fn an_npc_can_only_be_named() {
        assert_eq!(
            thing_menu_rows(&a_target(None, an_agent(AgentKind::Npc, false)), None),
            vec![
                Some(ThingAction::Look),
                None,
                Some(ThingAction::CopyName(name())),
            ]
        );
    }

    #[test]
    fn your_own_tile_reaches_the_item_under_you() {
        assert_eq!(
            thing_menu_rows(
                &a_target(Some(UseKind::Use), an_agent(AgentKind::Player, true)),
                None
            ),
            vec![
                Some(ThingAction::Look),
                Some(ThingAction::Use(UseKind::Use)),
                None,
                Some(ThingAction::CopyName(name())),
            ]
        );
    }

    #[test]
    fn every_action_has_its_reference_label() {
        let labels: Vec<String> = [
            ThingAction::Look,
            ThingAction::Use(UseKind::UseWith),
            ThingAction::Use(UseKind::Open),
            ThingAction::Use(UseKind::Use),
            ThingAction::Attack(AgentId(7)),
            ThingAction::StopAttack(AgentId(7)),
            ThingAction::MessageTo(name()),
            ThingAction::CopyName(name()),
        ]
        .iter()
        .map(ThingAction::label)
        .collect();

        assert_eq!(
            labels,
            [
                "Look",
                "Use with ...",
                "Open",
                "Use",
                "Attack",
                "Stop Attack",
                "Message to Bob",
                "Copy Name",
            ]
        );
    }

    use std::sync::Arc;

    use crate::items::{ContainerId, Item, ItemConfig, ItemFlag};

    #[derive(Resource, Default)]
    struct Seen {
        intents: Vec<InteractionIntent>,
        messaged: Vec<String>,
        copied: Vec<String>,
    }

    fn a_world(target: ThingMenuTarget, current: Option<AgentId>) -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<InteractionMode>();
        world.init_resource::<Map>();
        world.init_resource::<Seen>();
        let mut combat_target = CombatTarget::default();
        if let Some(agent_id) = current {
            combat_target.set(agent_id);
        }
        world.insert_resource(combat_target);
        world.add_observer(on_thing_menu_picked);
        world.add_observer(close_thing_menu_when_its_agent_leaves);
        world.add_observer(|event: On<InteractionIntent>, mut seen: ResMut<Seen>| {
            seen.intents.push((*event).clone())
        });
        world.add_observer(|event: On<SendMessage>, mut seen: ResMut<Seen>| {
            if let ClientMessage::OpenPmChat { name } = &event.0 {
                seen.messaged.push(name.clone());
            }
        });
        world.add_observer(|event: On<CopyToClipboard>, mut seen: ResMut<Seen>| {
            seen.copied.push(event.0.clone())
        });
        let rows = thing_menu_rows(&target, current);
        let menu = world.spawn(ThingMenu { target, rows }).id();
        world.flush();
        (world, menu)
    }

    fn pick(world: &mut World, menu: Entity, action: ThingAction) {
        let index = world
            .get::<ThingMenu>(menu)
            .unwrap()
            .rows
            .iter()
            .position(|row| row.as_ref() == Some(&action))
            .expect("the menu offers this action");
        world.trigger(ContextMenuPicked { menu, index });
        world.flush();
    }

    #[test]
    fn look_looks_at_the_captured_placement() {
        let (mut world, menu) = a_world(a_target(None, None), None);

        pick(&mut world, menu, ThingAction::Look);

        assert!(matches!(
            world.resource::<Seen>().intents.as_slice(),
            [InteractionIntent::Look(ItemPlacement::Map { index: 0, .. })]
        ));
    }

    #[test]
    fn use_sends_the_captured_item() {
        let (mut world, menu) = a_world(a_target(Some(UseKind::Use), None), None);

        pick(&mut world, menu, ThingAction::Use(UseKind::Use));

        assert!(matches!(
            world.resource::<Seen>().intents.as_slice(),
            [InteractionIntent::UseItem {
                item_id: ItemId(100),
                window_id: None,
                ..
            }]
        ));
    }

    #[test]
    fn use_with_enters_the_crosshair() {
        let (mut world, menu) = a_world(a_target(Some(UseKind::UseWith), None), None);

        pick(&mut world, menu, ThingAction::Use(UseKind::UseWith));

        assert!(world.resource::<InteractionMode>().is_targeting());
        assert!(world.resource::<Seen>().intents.is_empty());
    }

    #[test]
    fn attack_targets_the_agent() {
        let (mut world, menu) = a_world(a_target(None, an_agent(AgentKind::Creature, false)), None);

        pick(&mut world, menu, ThingAction::Attack(AgentId(7)));

        assert_eq!(world.resource::<CombatTarget>().target, Some(AgentId(7)));
        assert!(matches!(
            world.resource::<Seen>().intents.as_slice(),
            [InteractionIntent::SetTarget(Some(AgentId(7)), _)]
        ));
    }

    /// The agent became the target after the menu opened, by a plain right-click. A toggle
    /// would turn this pick into a clear.
    #[test]
    fn attack_on_an_agent_targeted_meanwhile_keeps_it_targeted() {
        let (mut world, menu) = a_world(a_target(None, an_agent(AgentKind::Creature, false)), None);
        world.resource_mut::<CombatTarget>().set(AgentId(7));

        pick(&mut world, menu, ThingAction::Attack(AgentId(7)));

        assert_eq!(world.resource::<CombatTarget>().target, Some(AgentId(7)));
    }

    #[test]
    fn stop_attack_clears_the_target() {
        let (mut world, menu) = a_world(
            a_target(None, an_agent(AgentKind::Creature, false)),
            Some(AgentId(7)),
        );

        pick(&mut world, menu, ThingAction::StopAttack(AgentId(7)));

        assert_eq!(world.resource::<CombatTarget>().target, None);
        assert!(matches!(
            world.resource::<Seen>().intents.as_slice(),
            [InteractionIntent::SetTarget(None, _)]
        ));
    }

    #[test]
    fn stop_attack_leaves_a_newer_target_alone() {
        let (mut world, menu) = a_world(
            a_target(None, an_agent(AgentKind::Creature, false)),
            Some(AgentId(7)),
        );
        world.resource_mut::<CombatTarget>().set(AgentId(9));

        pick(&mut world, menu, ThingAction::StopAttack(AgentId(7)));

        assert_eq!(world.resource::<CombatTarget>().target, Some(AgentId(9)));
        assert!(world.resource::<Seen>().intents.is_empty());
    }

    #[test]
    fn message_to_opens_a_private_chat_with_the_name() {
        let (mut world, menu) = a_world(a_target(None, an_agent(AgentKind::Player, false)), None);

        pick(&mut world, menu, ThingAction::MessageTo(name()));

        assert_eq!(world.resource::<Seen>().messaged, vec![name()]);
    }

    #[test]
    fn copy_name_asks_for_the_name_on_the_clipboard() {
        let (mut world, menu) = a_world(a_target(None, an_agent(AgentKind::Npc, false)), None);

        pick(&mut world, menu, ThingAction::CopyName(name()));

        assert_eq!(world.resource::<Seen>().copied, vec![name()]);
    }

    #[test]
    fn a_pick_on_someone_elses_menu_is_ignored() {
        let (mut world, _) = a_world(a_target(None, None), None);
        let other = world.spawn_empty().id();

        world.trigger(ContextMenuPicked {
            menu: other,
            index: 0,
        });
        world.flush();

        assert!(world.resource::<Seen>().intents.is_empty());
    }

    #[test]
    fn the_menu_closes_when_its_agent_leaves() {
        let (mut world, menu) = a_world(a_target(None, an_agent(AgentKind::Creature, false)), None);

        world.trigger(RemoveAgent {
            agent_id: AgentId(9),
        });
        world.flush();
        assert!(world.get_entity(menu).is_ok());

        world.trigger(RemoveAgent {
            agent_id: AgentId(7),
        });
        world.flush();
        assert!(world.get_entity(menu).is_err());
    }

    #[test]
    fn a_menu_with_no_agent_outlives_any_departure() {
        let (mut world, menu) = a_world(a_target(None, None), None);

        world.trigger(RemoveAgent {
            agent_id: AgentId(7),
        });
        world.flush();

        assert!(world.get_entity(menu).is_ok());
    }

    #[test]
    fn opening_spawns_a_menu_that_remembers_its_rows() {
        let mut world = World::new();
        world.insert_resource(GameUiAssets {
            font: Handle::default(),
            name_font: Handle::default(),
            window: Default::default(),
            inventory: Default::default(),
            background_dark: Handle::default(),
            background_light: Handle::default(),
            bar_overlay: Handle::default(),
            title_background: Handle::default(),
            buttons: Default::default(),
        });
        world.init_resource::<CombatTarget>();
        world.add_observer(on_open_thing_menu);

        world.trigger(OpenThingMenu {
            target: a_target(Some(UseKind::Use), an_agent(AgentKind::Player, false)),
            at: Vec2::new(10.0, 20.0),
        });
        world.flush();

        let mut menus = world.query::<&ThingMenu>();
        let menu = menus.single(&world).unwrap();
        assert_eq!(menu.rows.len(), 7);
        assert_eq!(menu.rows[1], Some(ThingAction::Use(UseKind::Use)));
    }

    #[test]
    fn the_snapshot_keeps_the_window_a_nested_container_sits_in() {
        let cursor = CursorTarget {
            placement: ItemPlacement::Container {
                container_id: ContainerId(1),
                slot: 2,
            },
            item: Arc::new(Item::new(
                Arc::new(ItemConfig {
                    id: ItemId(2854),
                    name: None,
                    flags: vec![ItemFlag::Usable, ItemFlag::Container],
                    friction: None,
                    slot: None,
                    minimap_color: None,
                    elevation: None,
                }),
                1,
            )),
            window_id: Some(WindowId(9)),
        };

        let target = ThingMenuTarget::new(&cursor, None);

        assert_eq!(target.item_id, ItemId(2854));
        assert_eq!(target.use_kind, Some(UseKind::Open));
        assert_eq!(target.window_id, Some(WindowId(9)));
    }

    #[test]
    fn a_snapshot_of_an_agent_knows_whether_it_is_you() {
        let agent = Agent {
            agent_id: AgentId(7),
            name: name(),
            kind: AgentKind::Player,
            ..Default::default()
        };

        assert!(MenuAgent::of(&agent, Some(AgentId(7))).is_self);
        assert!(!MenuAgent::of(&agent, Some(AgentId(1))).is_self);
        assert_eq!(MenuAgent::of(&agent, None).name, "Bob");
    }
}
