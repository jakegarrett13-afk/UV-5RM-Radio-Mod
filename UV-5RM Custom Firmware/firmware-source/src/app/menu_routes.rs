//! Menu positions are separate from persisted channel-mode IDs and flash slots.
pub const AIRBAND_SLOT: u8 = 5;
pub const MODE_OFFSET: usize = cfg!(feature = "airband") as usize;
pub fn app_slot(index: usize) -> u8 {
    (index
        + usize::from(
            cfg!(feature = "airband") && index >= AIRBAND_SLOT as usize,
        )) as u8
}
pub fn channel_mode(row: usize) -> Option<usize> {
    row.checked_sub(MODE_OFFSET)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn channel_ids_do_not_shift_in_storage() {
        for id in 0..13 {
            assert_eq!(channel_mode(id + MODE_OFFSET), Some(id));
        }
        if MODE_OFFSET != 0 {
            assert_eq!(channel_mode(0), None);
        }
    }
    #[test]
    fn launcher_preserves_every_other_flash_slot() {
        let slots: std::vec::Vec<_> =
            (0..9 - MODE_OFFSET).map(app_slot).collect();
        let expected: std::vec::Vec<_> = (0u8..9)
            .filter(|i| MODE_OFFSET == 0 || *i != AIRBAND_SLOT)
            .collect();
        assert_eq!(slots, expected);
    }
}
