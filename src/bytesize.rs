// mod bytesize

pub type UBytes = u8;

/// Get the minimum number of bytes needed to represent the given integer
#[inline]
pub fn get_min_size_signed(i: i64) -> UBytes {
    if i > i32::MAX as i64 || i < i32::MIN as i64 {
        8 // Need a 64 bit
    } else if i > i16::MAX as i64 || i < i16::MIN as i64 {
        4 // Need a 32 bit
    } else if i > i8::MAX as i64 || i < i8::MIN as i64 {
        2 // Need a 16 bit
    } else {
        1
    }
}
/// Get the minimum number of bytes needed to represent the given integer
#[inline]
pub fn get_min_size_unsigned(i: u64) -> UBytes {
    if i > u32::MAX as u64 {
        8
    } else if i > u16::MAX as u64 {
        4
    } else if i > u8::MAX as u64 {
        2
    } else {
        1
    }
}