use bevy::prelude::*;

use crate::network::events::PlayerStatusUpdated;

/// Discriminants are the server's status bits, `entities/conditions.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PlayerStatus {
    LogoutBlock = 0,
    Hungry = 1,
    Paralysed = 2,
    Burning = 3,
    Poisoned = 4,
    Electrified = 5,
    Hasted = 6,
    MagicShield = 7,
}

impl PlayerStatus {
    pub const ALL: [PlayerStatus; 8] = [
        PlayerStatus::LogoutBlock,
        PlayerStatus::Hungry,
        PlayerStatus::Paralysed,
        PlayerStatus::Burning,
        PlayerStatus::Poisoned,
        PlayerStatus::Electrified,
        PlayerStatus::Hasted,
        PlayerStatus::MagicShield,
    ];

    pub fn bit(self) -> u32 {
        1 << self as u8
    }
}

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerStatuses(pub u32);

impl PlayerStatuses {
    pub fn contains(self, status: PlayerStatus) -> bool {
        self.0 & status.bit() != 0
    }
}

pub fn on_player_status_updated(
    event: On<PlayerStatusUpdated>,
    mut statuses: ResMut<PlayerStatuses>,
) {
    *statuses = PlayerStatuses(event.status);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_status_is_the_bit_the_server_sends() {
        assert_eq!(PlayerStatus::LogoutBlock.bit(), 1);
        assert_eq!(PlayerStatus::Hungry.bit(), 1 << 1);
        assert_eq!(PlayerStatus::Paralysed.bit(), 1 << 2);
        assert_eq!(PlayerStatus::Burning.bit(), 1 << 3);
        assert_eq!(PlayerStatus::Poisoned.bit(), 1 << 4);
        assert_eq!(PlayerStatus::Electrified.bit(), 1 << 5);
        assert_eq!(PlayerStatus::Hasted.bit(), 1 << 6);
        assert_eq!(PlayerStatus::MagicShield.bit(), 1 << 7);
    }

    #[test]
    fn all_is_in_bit_order() {
        for (index, status) in PlayerStatus::ALL.into_iter().enumerate() {
            assert_eq!(status as usize, index);
        }
    }

    #[test]
    fn contains_only_the_statuses_whose_bits_are_set() {
        let statuses = PlayerStatuses(PlayerStatus::Hungry.bit() | PlayerStatus::Burning.bit());

        let present: Vec<_> = PlayerStatus::ALL
            .into_iter()
            .filter(|status| statuses.contains(*status))
            .collect();

        assert_eq!(present, [PlayerStatus::Hungry, PlayerStatus::Burning]);
    }

    #[test]
    fn a_bit_the_client_does_not_know_is_no_status() {
        let statuses = PlayerStatuses(1 << 8);

        assert!(
            PlayerStatus::ALL
                .into_iter()
                .all(|status| !statuses.contains(status))
        );
    }

    #[test]
    fn an_update_replaces_the_previous_statuses() {
        let mut world = World::new();
        world.init_resource::<PlayerStatuses>();
        world.add_observer(on_player_status_updated);

        world.trigger(PlayerStatusUpdated {
            status: PlayerStatus::Burning.bit(),
        });
        world.trigger(PlayerStatusUpdated {
            status: PlayerStatus::Hungry.bit(),
        });

        assert_eq!(
            *world.resource::<PlayerStatuses>(),
            PlayerStatuses(PlayerStatus::Hungry.bit())
        );
    }
}
