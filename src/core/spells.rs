use crate::{agent::AgentId, map::Position};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct SpellId(pub u16);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpellTarget {
    None,
    Agent(AgentId),
    #[allow(dead_code)]
    Position(Position),
}
