//! Where the skin root sits in each HUD layout the overlay captures (pure data, host-tested).
//!
//! Since 2026-11-05 the overlay renders only through the match HUD capture, whatever other plugins are installed.
//! Before that, a plugin without Training Modpack's NRO file name (TrainingRooms) switched it to a Layout::Draw path
//! that drew into the root layout: full size at the bottom right in matches, and nothing in waiting-room practice.

/// The captured HUD layout that holds the overlay's panes.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum HudPart {
    /// The root `info_melee` layout.
    MatchRoot,
    /// P1's player parts (`p1`, four players or fewer).
    P1Parts,
    /// P1's compact player parts (`p1_2`, more than four players).
    P1AltParts,
}

/// Skin root transform in the captured layout's coordinates.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: u8,
}

const fn at(x: f32, y: f32, scale: f32) -> Placement {
    Placement {
        x,
        y,
        scale,
        opacity: 255,
    }
}

/// `practice` is native Training or waiting-room practice (P1 and the CPU sit on the left there).
pub const fn hud_placement(part: HudPart, practice: bool) -> Placement {
    match (part, practice) {
        // The root layout: bottom right in a match, beyond the CPU panel in practice.
        (HudPart::MatchRoot, false) => at(760.0, -330.0, 1.0),
        (HudPart::MatchRoot, true) => at(160.0, -420.0, 0.55),
        // P1's parts: beside P1's HUD in a match, to the right of the CPU's HUD in practice.
        (HudPart::P1Parts, false) => at(315.0, -30.0, 0.55),
        (HudPart::P1Parts, true) => at(760.0, 30.0, 0.55),
        (HudPart::P1AltParts, false) => at(160.0, -80.0, 0.5),
        (HudPart::P1AltParts, true) => at(605.0, -20.0, 0.5),
    }
}

#[cfg(test)]
mod tests {
    use super::{hud_placement, HudPart};

    #[test]
    fn a_match_keeps_the_controller_beside_p1_at_reduced_size() {
        let p1 = hud_placement(HudPart::P1Parts, false);
        assert_eq!((p1.x, p1.y, p1.scale), (315.0, -30.0, 0.55));
        let compact = hud_placement(HudPart::P1AltParts, false);
        assert_eq!((compact.x, compact.y, compact.scale), (160.0, -80.0, 0.5));
    }

    #[test]
    fn practice_moves_the_controller_right_of_the_cpu_hud() {
        let p1 = hud_placement(HudPart::P1Parts, true);
        assert_eq!((p1.x, p1.y, p1.scale), (760.0, 30.0, 0.55));
        assert!(p1.x > hud_placement(HudPart::P1Parts, false).x);
        let compact = hud_placement(HudPart::P1AltParts, true);
        assert_eq!((compact.x, compact.y, compact.scale), (605.0, -20.0, 0.5));
    }

    #[test]
    fn player_parts_never_use_the_full_size_root_transform() {
        for practice in [false, true] {
            for part in [HudPart::P1Parts, HudPart::P1AltParts] {
                let placement = hud_placement(part, practice);
                assert!(placement.scale < 1.0, "{part:?} practice={practice}");
                assert_eq!(placement.opacity, 255);
            }
        }
        assert_eq!(hud_placement(HudPart::MatchRoot, false).scale, 1.0);
    }
}
