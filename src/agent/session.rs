use bevy::prelude::*;

use crate::agent::components::Agent;
use crate::agent::material::AgentInstance;
use crate::core::InstanceManager;

/// Despawns the session's agents and frees their GPU instance slots.
pub(super) fn cleanup_session(mut commands: Commands, agents: Query<Entity, With<Agent>>) {
    for entity in &agents {
        commands.entity(entity).despawn();
    }
    commands.insert_resource(InstanceManager::<AgentInstance>::default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// Agents are root entities in the world, not children of anything the UI
    /// despawn sweep reaches — if this doesn't despawn them, the previous
    /// session's characters are still standing there in the next one.
    #[test]
    fn cleanup_despawns_every_agent() {
        let mut world = World::new();
        world.init_resource::<InstanceManager<AgentInstance>>();
        let agent = world.spawn(Agent::default()).id();

        world.run_system_once(cleanup_session).unwrap();

        assert!(world.get_entity(agent).is_err());
    }
}
