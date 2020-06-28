pub fn is_bit_set(bitfield: u32, bit: u8) -> bool {
    let mask = 1 << bit;
    bitfield & mask == mask
}

pub fn set_bit(bitfield: &mut u32, bit: u8, value: bool) {
    let mask = 1 << bit;

    if value {
        *bitfield |= mask;
    } else {
        *bitfield &= !mask;
    }
}
