/// What a splash or fluid container holds. The discriminants **are** the wire
/// values, matching OTClient's `FluidsType`; the server repeats them.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FluidType {
    None = 0,
    Water = 1,
    Mana = 2,
    Beer = 3,
    Oil = 4,
    Blood = 5,
    Slime = 6,
    Mud = 7,
    Lemonade = 8,
    Milk = 9,
    Wine = 10,
    Health = 11,
    Urine = 12,
    Rum = 13,
    FruitJuice = 14,
    CoconutMilk = 15,
    Tea = 16,
    Mead = 17,
    Ink = 18,
    Candy = 19,
    Chocolate = 20,
}

/// Colour indices, in the order the sprite grid lays them out. Eleven of them,
/// which is why a pool's grid is 4x3 and not the 4x2 a stackable uses.
const TRANSPARENT: u32 = 0;
const BLUE: u32 = 1;
const RED: u32 = 2;
const ORANGE: u32 = 3;
const GREEN: u32 = 4;
const YELLOW: u32 = 5;
const WHITE: u32 = 6;
const PURPLE: u32 = 7;
const BLACK: u32 = 8;
const BROWN: u32 = 9;
const PINK: u32 = 10;

/// The fluid's colour, reproducing OTClient's switch. Many-to-one: blood and
/// health are both red, and six different fluids are orange. An unrecognised
/// byte is transparent, matching that switch's `default:` -- a server naming a
/// fluid this client does not know draws nothing rather than guessing.
fn fluid_colour(fluid: u8) -> u32 {
    match fluid {
        // Explicit, and separate from the catch-all below: byte 0 means "no
        // fluid", which is not the same statement as "a fluid this client does
        // not recognise". OTClient draws both transparent but distinguishes them.
        f if f == FluidType::None as u8 => TRANSPARENT,
        f if f == FluidType::Water as u8 => BLUE,
        f if f == FluidType::Mana as u8 => PURPLE,
        f if f == FluidType::Beer as u8 => ORANGE,
        f if f == FluidType::Oil as u8 => ORANGE,
        f if f == FluidType::Blood as u8 => RED,
        f if f == FluidType::Slime as u8 => GREEN,
        f if f == FluidType::Mud as u8 => ORANGE,
        f if f == FluidType::Lemonade as u8 => YELLOW,
        f if f == FluidType::Milk as u8 => WHITE,
        f if f == FluidType::Wine as u8 => PURPLE,
        f if f == FluidType::Health as u8 => RED,
        f if f == FluidType::Urine as u8 => YELLOW,
        f if f == FluidType::Rum as u8 => ORANGE,
        f if f == FluidType::FruitJuice as u8 => YELLOW,
        f if f == FluidType::CoconutMilk as u8 => WHITE,
        f if f == FluidType::Tea as u8 => ORANGE,
        f if f == FluidType::Mead as u8 => ORANGE,
        f if f == FluidType::Ink as u8 => BLACK,
        f if f == FluidType::Candy as u8 => PINK,
        f if f == FluidType::Chocolate as u8 => BROWN,
        _ => TRANSPARENT,
    }
}

/// What a container holding `fluid` is "of"; `None` for no fluid and for one this client does not
/// know. The server's look text names fluids the same way.
pub fn fluid_name(fluid: u8) -> Option<&'static str> {
    Some(match fluid {
        f if f == FluidType::Water as u8 => "water",
        f if f == FluidType::Mana as u8 => "manafluid",
        f if f == FluidType::Beer as u8 => "beer",
        f if f == FluidType::Oil as u8 => "oil",
        f if f == FluidType::Blood as u8 => "blood",
        f if f == FluidType::Slime as u8 => "slime",
        f if f == FluidType::Mud as u8 => "mud",
        f if f == FluidType::Lemonade as u8 => "lemonade",
        f if f == FluidType::Milk as u8 => "milk",
        f if f == FluidType::Wine as u8 => "wine",
        f if f == FluidType::Health as u8 => "lifefluid",
        f if f == FluidType::Urine as u8 => "urine",
        f if f == FluidType::Rum as u8 => "rum",
        f if f == FluidType::FruitJuice as u8 => "fruit juice",
        f if f == FluidType::CoconutMilk as u8 => "coconut milk",
        f if f == FluidType::Tea as u8 => "tea",
        f if f == FluidType::Mead as u8 => "mead",
        f if f == FluidType::Ink as u8 => "ink",
        f if f == FluidType::Candy as u8 => "candyfluid",
        f if f == FluidType::Chocolate as u8 => "chocolate",
        _ => return None,
    })
}

/// The pattern cell a fluid draws: `(colour % 4, colour / 4)`, bounded by the
/// appearance's own grid.
///
/// Three numbers are in play and they are not the same: the FLUID is what
/// arrives on the wire (blood is 5), the COLOUR indexes the grid (red is 2),
/// and the CELL is what comes out (blood draws at (2, 0)).
pub fn fluid_cell(fluid: u8, pattern_x: u32, pattern_y: u32) -> (u32, u32) {
    let colour = fluid_colour(fluid);
    (
        (colour % 4) % pattern_x.max(1),
        (colour / 4) % pattern_y.max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fluid_has_a_name_and_no_fluid_has_none() {
        assert_eq!(fluid_name(FluidType::Milk as u8), Some("milk"));
        assert_eq!(fluid_name(FluidType::None as u8), None);
        assert_eq!(fluid_name(200), None);
    }

    /// The server repeats this enum with the same discriminants, and nothing
    /// links the two -- they are separate repositories. This literal is the pin:
    /// the matching assertion lives in the server's `entities/items.rs`, and if
    /// the two ever disagree every fluid in the game silently recolours.
    ///
    /// One sample is enough only because the discriminants are written
    /// explicitly rather than left positional.
    #[test]
    fn blood_is_five_on_the_wire() {
        assert_eq!(FluidType::Blood as u8, 5);
    }

    /// Blood is fluid 5, red is colour 2, and the cell is (2, 0) -- three
    /// different numbers for one fluid. Indexing the grid with the fluid instead
    /// of the colour gives (1, 1), which looks entirely plausible on screen.
    ///
    /// Slime is the second case for a reason: every colour in the grid's first
    /// row has `colour / 4 == 0`, so a blood-only test passes against a formula
    /// that ignores the row entirely.
    #[test]
    fn a_fluid_lands_on_its_colours_cell() {
        assert_eq!(fluid_cell(FluidType::Blood as u8, 4, 3), (2, 0));
        assert_eq!(fluid_cell(FluidType::Slime as u8, 4, 3), (0, 1));
    }
}
