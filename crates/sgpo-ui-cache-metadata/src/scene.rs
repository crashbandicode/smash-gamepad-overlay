//! Pure placement policy for the native waiting-room practice scene.

/// PracticeScene selects melee 0x0d; the normal match mapper never selects it.
/// Restrict the mode families as well so menus and unrelated modes keep their
/// existing placement. This does not enable Training Modpack or change gameplay.
pub const fn is_waiting_practice(initialized: bool, melee: u8, mode: u32) -> bool {
    initialized && melee == 0x0d && (mode == 0x02010000 || mode & 0x7fff0000 == 0x07010000)
}

#[cfg(test)]
mod tests {
    use super::is_waiting_practice;

    #[test]
    fn local_and_online_waiting_rooms_use_practice_placement() {
        for mode in [0x02010000, 0x07010000, 0x07010001, 0x87010000] {
            assert!(is_waiting_practice(true, 0x0d, mode));
            assert!(!is_waiting_practice(false, 0x0d, mode));
        }
    }

    #[test]
    fn real_matches_in_the_same_mode_family_keep_match_placement() {
        for melee in 0..=u8::MAX {
            if melee != 0x0d {
                for mode in [0x02010000, 0x07010000, 0x87010000] {
                    assert!(!is_waiting_practice(true, melee, mode));
                }
            }
        }
    }

    #[test]
    fn unrelated_modes_do_not_match_a_stale_practice_melee_value() {
        for mode in [
            0, 0x02010001, 0x02020000, 0x07000000, 0x07020000, 0xffffffff,
        ] {
            assert!(!is_waiting_practice(true, 0x0d, mode));
        }
    }
}
