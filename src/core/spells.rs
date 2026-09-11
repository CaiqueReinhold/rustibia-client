use crate::{agent::AgentId, map::Position};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct SpellId(pub u16);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpellTarget {
    None,
    #[allow(dead_code)]
    Agent(AgentId),
    #[allow(dead_code)]
    Position(Position),
}
