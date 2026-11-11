//! Shared streaming CRC; no temporary payload copy is needed.
#[inline(never)]
pub(crate) fn crc16_xmodem_update(mut crc: u16, data: &[u8]) -> u16 {
    for &byte in data {
        // Table-free CCITT/XMODEM byte update, equivalent to eight bit steps.
        let x = (crc >> 8) ^ byte as u16;
        let x = x ^ (x >> 4);
        crc = (crc << 8) ^ (x << 12) ^ (x << 5) ^ x;
    }
    crc
}

pub(crate) fn crc16_xmodem(data: &[u8]) -> u16 {
    crc16_xmodem_update(0, data)
}
