use std::collections::VecDeque;
use std::time::Duration;

use bevy::prelude::*;

use crate::agent::components::Agent;
use crate::agent::{AgentId, WalkingDirection};
use crate::map::{DrawLayer, DrawOrder, FloorEntities, Map, Position, drawn_tile};
use crate::network::events::{AgentChangedDirection, TeleportAgent};
use crate::player::components::Player;

#[derive(Component, Debug)]
pub struct Moving {
    pub start: Position,
    pub end: Position,
    pub timer: Timer,
}

#[derive(Component, Debug)]
pub struct ShouldTeleport {
    pub position: Position,
}

#[derive(Component, Debug, Default)]
pub struct MoveQueue(pub VecDeque<(Position, WalkingDirection)>);

#[derive(Event, Debug)]
pub struct StartAgentMove {
    pub agent_id: AgentId,
    pub direction: WalkingDirection,
}

#[derive(Event, Debug)]
pub struct UpdateElevation {
    pub pos: Position,
}

/// Places an agent on a tile **without touching its z**, which belongs to
/// `map::draw_order` and to nothing else.
///
/// `to_world_with_elevation` returns z = 0, so assigning the whole translation
/// zeroes the draw key — and `apply_draw_order` only rewrites entities whose
/// `DrawOrder` changed, so nothing puts it back.
fn place(transform: &mut Transform, pos: &Position, elevation: u8) {
    let world = pos.to_world_with_elevation(elevation);
    transform.translation.x = world.x;
    transform.translation.y = world.y;
}

pub fn on_start_agent_move(
    event: On<StartAgentMove>,
    mut commands: Commands,
    mut agent_q: Query<(&mut Agent, &Position, &mut Transform, &mut DrawOrder)>,
    map: Res<Map>,
) {
    let Some(entity) = map.get_agent(event.agent_id) else {
        return;
    };
    let Ok((mut agent, position, mut transform, mut order)) = agent_q.get_mut(entity) else {
        return;
    };

    let start_position = position.clone();
    let facing = event.direction.facing();
    let end_position = start_position.clone() + event.direction;
    agent.direction = facing;

    let Some(tile_modifier) = map.get_tile_friction(&end_position) else {
        // This client has no friction for the destination, so any step duration
        // would be invented — and a short one frees the send gate a frame later,
        // straight into a full server cooldown. Place the agent
        // instead, the same way `move_agent` does when a step completes.
        let elevation = map.get_elevation(&end_position);
        place(&mut transform, &end_position, elevation);
        DrawOrder::move_to(&mut order, &end_position, DrawLayer::Creature);
        commands.entity(entity).insert(end_position);
        return;
    };

    let slide_ms = agent.get_step_duration(tile_modifier, false);
    // A denied walk and a server position correction both build a `Moving`
    // without coming through here, to slide the agent back; `move_agent`'s
    // re-insert on arrival is what settles `Position` for those.
    commands
        .entity(entity)
        .insert(Moving {
            start: start_position,
            end: end_position.clone(),
            timer: Timer::new(Duration::from_millis(slide_ms as u64), TimerMode::Once),
        })
        .insert(end_position);
}

pub fn on_agent_change_direction(
    event: On<AgentChangedDirection>,
    map: Res<Map>,
    mut agent_q: Query<&mut Agent>,
) {
    if let Some(agent_entity) = map.get_agent(event.agent_id)
        && let Ok(mut agent) = agent_q.get_mut(agent_entity)
        && agent.direction != event.facing
    {
        agent.direction = event.facing;
    }
}

pub fn move_agent(
    mut commands: Commands,
    mut moving_q: Query<(Entity, &mut Transform, &mut Moving, &mut DrawOrder), With<Agent>>,
    mut agent_q: Query<&mut Agent>,
    map: Res<Map>,
    time: Res<Time>,
) {
    for (entity, mut transform, mut moving, mut order) in moving_q.iter_mut() {
        moving.timer.tick(time.delta());
        if moving.timer.is_finished() {
            let mut agent = agent_q.get_mut(entity).unwrap();
            agent.set_changed();

            commands
                .entity(entity)
                .try_insert(moving.end.clone())
                .try_remove::<Moving>();

            let elevation = map.get_elevation(&moving.end);
            place(&mut transform, &moving.end, elevation);
            // Not left to `sync_agent_draw_order`: a frame long enough to skip
            // the end of the step would strand the creature on a
            // `PassingThrough` corner until the next `PostUpdate`.
            DrawOrder::move_to(&mut order, &moving.end, DrawLayer::Creature);
            continue;
        }

        let start = moving.start.to_world();
        let elevation = if moving.timer.fraction() > 0.5 {
            let end_elevation = map.get_elevation(&moving.end);
            vec3(-(end_elevation as f32), end_elevation as f32, 0.0)
        } else {
            let start_elevation = map.get_elevation(&moving.start);
            vec3(-(start_elevation as f32), start_elevation as f32, 0.0)
        };
        let end = moving.end.to_world();
        let slide = start.lerp(end, moving.timer.fraction());
        let interpolated = slide + elevation;
        // Only x and y; z belongs to `map::draw_order`.
        transform.translation.x = interpolated.x.round();
        transform.translation.y = interpolated.y.round();
        // `slide` and not `interpolated`: elevation shifts a sprite up and left,
        // which would only ever pick an earlier tile.
        let tile = drawn_tile(slide.truncate(), moving.end.z);
        // Only a north-east or south-west step can reach a tile that is neither
        // of its endpoints, so the test is on the tile rather than the direction.
        let layer = if tile == moving.start || tile == moving.end {
            DrawLayer::Creature
        } else {
            DrawLayer::PassingThrough
        };
        DrawOrder::move_to(&mut order, &tile, layer);
    }
}

/// `Without<Moving>` because this places an agent outright, and `Position` names
/// the destination from the moment a step begins — snapping a sliding agent here
/// would jump it a whole tile.
pub fn on_update_elevation(
    event: On<UpdateElevation>,
    mut moving_q: Query<(&mut Transform, &Position), (With<Agent>, Without<Moving>)>,
    map: Res<Map>,
) {
    let elevation = map.get_elevation(&event.pos);
    for (mut transform, position) in moving_q.iter_mut() {
        if *position == event.pos {
            place(&mut transform, position, elevation);
        }
    }
}

pub fn on_teleport_agent(event: On<TeleportAgent>, mut commands: Commands, map: Res<Map>) {
    if let Some(agent) = map.get_agent(event.agent_id) {
        commands.entity(agent).insert(ShouldTeleport {
            position: event.position.clone(),
        });
    }
}

pub fn teleport_agents(
    mut commands: Commands,
    mut agents_q: Query<(
        Entity,
        &ShouldTeleport,
        &mut Transform,
        Option<&Moving>,
        Option<&Player>,
    )>,
    map: Res<Map>,
    floor_ents: Res<FloorEntities>,
) {
    for (entity, teleport, mut transform, moving, player) in agents_q.iter_mut() {
        if moving.is_none() {
            let elevation = map.get_elevation(&teleport.position);
            place(&mut transform, &teleport.position, elevation);
            commands
                .entity(entity)
                .try_insert(teleport.position.clone())
                .try_remove::<ShouldTeleport>();

            if player.is_none() {
                commands
                    .entity(entity)
                    .try_insert(ChildOf(floor_ents.floors[teleport.position.z as usize]));
            }
        }
    }
}

pub fn process_agent_move_queues(
    mut commands: Commands,
    mut queue_q: Query<(Entity, &Agent, &Position, &mut MoveQueue), Without<Moving>>,
) {
    for (entity, agent, position, mut queue) in &mut queue_q {
        let Some((move_from, direction)) = queue.0.pop_front() else {
            continue;
        };

        if *position != move_from {
            if queue.0.is_empty() {
                commands.entity(entity).try_insert(move_from);
            } else {
                continue;
            }
        }

        commands.trigger(StartAgentMove {
            agent_id: agent.agent_id,
            direction,
        });
    }
}

/// Keeps a standing agent's `DrawOrder` on the tile its `Position` names.
///
/// `move_agent` owns the tile of an agent mid-step, so this skips `Moving` and
/// covers what moves an agent without one: a teleport, a snap onto a tile with
/// no friction, and the re-insert `process_agent_move_queues` does when a queued
/// step disagrees with where the agent is.
pub fn sync_agent_draw_order(
    mut agents_q: Query<
        (&Position, &mut DrawOrder),
        (With<Agent>, Without<Moving>, Changed<Position>),
    >,
) {
    for (position, mut order) in &mut agents_q {
        DrawOrder::move_to(&mut order, position, DrawLayer::Creature);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::FacingDirection;
    use crate::items::ItemId;
    use crate::items::{Item, ItemConfig, ItemFlag};
    use crate::map::DrawRank;
    use bevy::ecs::schedule::{ExecutorKind, ScheduleBuildSettings};
    use bevy::ecs::system::{RunSystemOnce, ScheduleSystem};
    use std::sync::Arc;

    fn at(x: u16, y: u16) -> Position {
        Position { x, y, z: 7 }
    }

    /// An agent on (100, 100) with ground under it and its eight neighbours, so
    /// any step has the friction it needs.
    fn walkable_world() -> (World, Entity) {
        let ground = Arc::new(ItemConfig {
            id: ItemId(1),
            flags: vec![ItemFlag::Ground],
            friction: Some(150),
            slot: None,
            minimap_color: None,
            elevation: None,
        });
        let mut world = World::new();
        let mut map = Map::default();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let pos = at((100 + dx) as u16, (100 + dy) as u16);
                map.replace_tile(vec![Arc::new(Item::new(ground.clone(), 1))], &pos);
            }
        }
        let entity = world
            .spawn((
                Agent {
                    agent_id: AgentId(1),
                    speed: 120,
                    ..Default::default()
                },
                at(100, 100),
                DrawOrder::new(at(100, 100), DrawRank::Standing, DrawLayer::Creature, 0),
                Transform::default(),
            ))
            .id();
        map.add_agent(AgentId(1), entity);
        world.insert_resource(map);
        world.insert_resource(Time::<()>::default());
        (world, entity)
    }

    /// An agent can be told to walk onto a tile this client has never been sent —
    /// it happens at the viewport edge. There is no honest step duration for that,
    /// so the agent is placed rather than glided at a made-up speed.
    #[test]
    fn an_unknown_destination_friction_snaps_instead_of_gliding() {
        let mut world = World::new();
        let mut map = Map::default();
        let entity = world
            .spawn((
                Agent {
                    agent_id: AgentId(1),
                    speed: 120,
                    ..Default::default()
                },
                at(100, 100),
                DrawOrder::new(at(100, 100), DrawRank::Standing, DrawLayer::Creature, 0),
                Transform::default(),
            ))
            .id();
        map.add_agent(AgentId(1), entity);
        // No tile is inserted anywhere, so the destination has no friction.
        world.insert_resource(map);
        world.add_observer(on_start_agent_move);

        world.trigger(StartAgentMove {
            agent_id: AgentId(1),
            direction: WalkingDirection::East,
        });
        world.flush();

        assert!(
            world.get::<Moving>(entity).is_none(),
            "no interpolation without a real step duration"
        );
        assert_eq!(
            world.get::<Position>(entity),
            Some(&at(101, 100)),
            "the agent still arrives"
        );
        assert_eq!(
            world.get::<Agent>(entity).unwrap().direction,
            FacingDirection::East,
            "and still turns"
        );
    }

    /// The agent occupies its destination from the moment the step begins, not
    /// when it lands. `Moving` still carries both tiles, so the slide is
    /// unaffected; what changes is that the tile index, targeting and the
    /// viewport anchor all agree with the server a step earlier.
    #[test]
    fn a_step_sets_the_position_when_it_starts() {
        let (mut world, entity) = walkable_world();
        world.add_observer(on_start_agent_move);

        world.trigger(StartAgentMove {
            agent_id: AgentId(1),
            direction: WalkingDirection::South,
        });
        world.flush();

        assert!(
            world.get::<Moving>(entity).is_some(),
            "still sliding, so the animation is unaffected"
        );
        assert_eq!(
            world.get::<Position>(entity),
            Some(&at(100, 101)),
            "and already standing on the destination"
        );
        let moving = world.get::<Moving>(entity).unwrap();
        assert_eq!(
            moving.start,
            at(100, 100),
            "the slide still knows both tiles"
        );
        assert_eq!(moving.end, at(100, 101));
    }

    /// A walking creature is re-homed onto the tile holding its sprite's
    /// bottom-right corner, every frame — OTClient's `updateWalkingTile`. Two
    /// things are asserted that a per-step rule cannot produce: mid-step a
    /// north-east walker draws with the tile EAST of its start, on neither end
    /// of the step, and it has let that tile go again by the time it arrives.
    #[test]
    fn a_walking_agent_rehomes_onto_the_tile_its_sprite_reaches() {
        let (mut world, entity) = walkable_world();
        world.entity_mut(entity).insert(Moving {
            start: at(100, 100),
            end: at(101, 99),
            timer: Timer::new(Duration::from_millis(100), TimerMode::Once),
        });

        let advance = |world: &mut World, ms: u64| {
            world
                .resource_mut::<Time<()>>()
                .advance_by(Duration::from_millis(ms));
            world.run_system_once(move_agent).unwrap();
            world.flush();
            world.get::<DrawOrder>(entity).unwrap().pos.clone()
        };

        assert_eq!(
            advance(&mut world, 50),
            at(101, 100),
            "mid-step, the tile east of the start — on neither end of the step"
        );
        assert_eq!(
            advance(&mut world, 49),
            at(101, 99),
            "released before arrival, so it stops covering that tile's items"
        );
    }

    /// Crossing a tile it never stands on, the creature draws UNDER that tile's
    /// walls — the door case: walking north-east off a door square, the player
    /// appeared over the wall beside it. On the tiles it does occupy it is a
    /// creature like any other, so the layer has to change back.
    #[test]
    fn a_creature_crossing_a_tile_passes_behind_its_walls() {
        use crate::map::DrawOrigin;

        let (mut world, entity) = walkable_world();
        world.entity_mut(entity).insert(Moving {
            start: at(100, 100),
            end: at(101, 99),
            timer: Timer::new(Duration::from_millis(100), TimerMode::Once),
        });

        let advance = |world: &mut World, ms: u64| {
            world
                .resource_mut::<Time<()>>()
                .advance_by(Duration::from_millis(ms));
            world.run_system_once(move_agent).unwrap();
            world.flush();
            world.get::<DrawOrder>(entity).unwrap().clone()
        };

        let origin = DrawOrigin::around(&at(100, 100));
        let crossing = advance(&mut world, 50);
        assert_eq!(crossing.pos, at(101, 100), "the tile it is crossing");

        let wall = DrawOrder::new(at(101, 100), DrawRank::Standing, DrawLayer::Bottom, 15);
        let ground = DrawOrder::new(at(101, 100), DrawRank::Standing, DrawLayer::Ground, 0);
        assert!(
            crossing.key(&origin) < wall.key(&origin),
            "behind the wall on the tile it is crossing"
        );
        assert!(
            crossing.key(&origin) > ground.key(&origin),
            "and still above that tile's ground, so it is not cut"
        );

        // Arriving, it is a creature again and draws over its own tile's walls.
        let arrived = advance(&mut world, 60);
        assert_eq!(arrived.pos, at(101, 99));
        assert!(
            arrived.key(&origin)
                > DrawOrder::new(at(101, 99), DrawRank::Standing, DrawLayer::Bottom, 15)
                    .key(&origin),
            "on the tile it occupies it stands in front of what is on it"
        );
    }

    /// Only the two diagonals that cross a tile on neither end of the step get
    /// the passing layer. A cardinal walker is on a tile it occupies for the
    /// whole step, and demoting it under that tile's walls would be wrong.
    #[test]
    fn a_cardinal_step_never_uses_the_passing_layer() {
        use crate::map::DrawOrigin;

        let origin = DrawOrigin::around(&at(100, 100));
        let creature_at =
            |pos| DrawOrder::new(pos, DrawRank::Standing, DrawLayer::Creature, 0).key(&origin);

        for (dir, end) in [
            (WalkingDirection::North, at(100, 99)),
            (WalkingDirection::South, at(100, 101)),
            (WalkingDirection::East, at(101, 100)),
            (WalkingDirection::West, at(99, 100)),
        ] {
            let (mut world, entity) = walkable_world();
            world.entity_mut(entity).insert(Moving {
                start: at(100, 100),
                end: end.clone(),
                timer: Timer::new(Duration::from_millis(100), TimerMode::Once),
            });

            for _ in 0..3 {
                world
                    .resource_mut::<Time<()>>()
                    .advance_by(Duration::from_millis(25));
                world.run_system_once(move_agent).unwrap();
                world.flush();

                let order = world.get::<DrawOrder>(entity).unwrap();
                assert_eq!(
                    order.key(&origin),
                    creature_at(order.pos.clone()),
                    "walking {dir:?} must stay on the creature layer"
                );
            }
        }
    }

    /// Placing an agent must never touch its z. `map::draw_order` writes z only
    /// for entities whose `DrawOrder` changed that frame, so a step that arrives
    /// on the tile it was already keyed to leaves a wholesale `translation`
    /// write here uncorrected: the creature sits at z = 0, behind the ground,
    /// for as long as it stands still.
    #[test]
    fn finishing_a_step_leaves_the_draw_key_alone() {
        let ground = Arc::new(ItemConfig {
            id: ItemId(1),
            flags: vec![ItemFlag::Ground],
            friction: Some(150),
            slot: None,
            minimap_color: None,
            elevation: None,
        });
        let mut world = World::new();
        let mut map = Map::default();
        for pos in [at(100, 100), at(101, 100)] {
            map.replace_tile(vec![Arc::new(Item::new(ground.clone(), 1))], &pos);
        }
        world.insert_resource(map);
        world.insert_resource(Time::<()>::default());

        const KEY: f32 = 123_456.0;
        let entity = world
            .spawn((
                Agent {
                    agent_id: AgentId(1),
                    speed: 120,
                    ..Default::default()
                },
                at(100, 100),
                DrawOrder::new(at(101, 100), DrawRank::Standing, DrawLayer::Creature, 0),
                Transform::from_xyz(0.0, 0.0, KEY),
                Moving {
                    start: at(100, 100),
                    end: at(101, 100),
                    timer: Timer::new(Duration::from_millis(10), TimerMode::Once),
                },
            ))
            .id();

        world
            .resource_mut::<Time<()>>()
            .advance_by(Duration::from_millis(20));
        world.run_system_once(move_agent).unwrap();
        world.flush();

        assert!(
            world.get::<Moving>(entity).is_none(),
            "the step completed, which is the case that broke"
        );
        assert_eq!(
            world.get::<Transform>(entity).unwrap().translation.z,
            KEY,
            "arriving must not clear the draw key"
        );
    }

    /// Runs `system` in one frame with a despawn of `entity` whose buffer is
    /// applied before the system's — what happens when `receive_messages`
    /// removes or re-spawns a creature that an agent system also touched.
    fn run_after_a_same_frame_despawn<M>(
        world: &mut World,
        entity: Entity,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) {
        let mut schedule = Schedule::default();
        schedule.set_executor_kind(ExecutorKind::SingleThreaded);
        schedule.set_build_settings(ScheduleBuildSettings {
            auto_insert_apply_deferred: false,
            ..default()
        });
        schedule.add_systems(
            (
                move |mut commands: Commands| commands.entity(entity).despawn(),
                system,
            )
                .chain(),
        );
        schedule.run(world);
    }

    #[test]
    fn a_step_ending_on_a_despawned_agent_is_dropped() {
        let (mut world, entity) = walkable_world();
        world.entity_mut(entity).insert(Moving {
            start: at(100, 100),
            end: at(100, 101),
            timer: Timer::new(Duration::from_millis(10), TimerMode::Once),
        });
        world
            .resource_mut::<Time<()>>()
            .advance_by(Duration::from_millis(20));

        run_after_a_same_frame_despawn(&mut world, entity, move_agent);

        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn a_queued_step_on_a_despawned_agent_is_dropped() {
        let (mut world, entity) = walkable_world();
        world.entity_mut(entity).insert(MoveQueue(VecDeque::from([(
            at(99, 100),
            WalkingDirection::East,
        )])));

        run_after_a_same_frame_despawn(&mut world, entity, process_agent_move_queues);

        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn a_teleport_of_a_despawned_agent_is_dropped() {
        let (mut world, entity) = walkable_world();
        let floors = std::array::from_fn(|_| world.spawn_empty().id());
        world.insert_resource(FloorEntities { floors });
        world.entity_mut(entity).insert(ShouldTeleport {
            position: Position {
                x: 100,
                y: 100,
                z: 6,
            },
        });

        run_after_a_same_frame_despawn(&mut world, entity, teleport_agents);

        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn a_teleport_across_floors_moves_the_agent_and_keeps_its_children() {
        let (mut world, entity) = walkable_world();
        let floors: [Entity; _] = std::array::from_fn(|_| world.spawn_empty().id());
        world.insert_resource(FloorEntities { floors });
        world.entity_mut(floors[7]).add_child(entity);
        let square = world.spawn(ChildOf(entity)).id();
        world.entity_mut(entity).insert(ShouldTeleport {
            position: Position {
                x: 100,
                y: 100,
                z: 6,
            },
        });

        world.run_system_once(teleport_agents).unwrap();

        assert_eq!(world.get::<ChildOf>(entity).unwrap().parent(), floors[6]);
        assert!(
            world
                .get::<Children>(floors[7])
                .is_none_or(|c| c.is_empty())
        );
        assert_eq!(world.get::<ChildOf>(square).unwrap().parent(), entity);
    }

    /// OTClient's `updateWalk` drives the slide from `getStepDuration(true)` and only
    /// terminates the walk on the full duration, so a diagonal is drawn at the same speed
    /// as a cardinal and the extra time is spent standing still on the new tile.
    #[test]
    fn a_diagonal_is_drawn_at_cardinal_speed() {
        let ground = Arc::new(ItemConfig {
            id: ItemId(1),
            flags: vec![ItemFlag::Ground],
            friction: Some(150),
            slot: None,
            minimap_color: None,
            elevation: None,
        });
        let mut world = World::new();
        let mut map = Map::default();
        for pos in [at(100, 100), at(101, 101), at(101, 100)] {
            map.replace_tile(vec![Arc::new(Item::new(ground.clone(), 1))], &pos);
        }
        let entity = world
            .spawn((
                Agent {
                    agent_id: AgentId(1),
                    speed: 120,
                    ..Default::default()
                },
                at(100, 100),
                DrawOrder::new(at(100, 100), DrawRank::Standing, DrawLayer::Creature, 0),
                Transform::default(),
            ))
            .id();
        map.add_agent(AgentId(1), entity);
        world.insert_resource(map);
        world.add_observer(on_start_agent_move);

        world.trigger(StartAgentMove {
            agent_id: AgentId(1),
            direction: WalkingDirection::SouthEast,
        });
        world.flush();

        let diagonal = world.get::<Moving>(entity).unwrap().timer.duration();

        world.trigger(StartAgentMove {
            agent_id: AgentId(1),
            direction: WalkingDirection::East,
        });
        world.flush();

        let cardinal = world.get::<Moving>(entity).unwrap().timer.duration();

        assert_eq!(diagonal, Duration::from_millis(500));
        assert_eq!(
            diagonal, cardinal,
            "a diagonal is drawn in the same time as a cardinal, not 2.5x it"
        );
    }
}
