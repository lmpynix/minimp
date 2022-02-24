// mod encode

use super::bytesize::*;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EncodedElement<'a> {
    Nil,
    Int(i64),
    UInt(u64),
    Bool(bool),
    Bin(&'a [u8]),
    Float(f32),
    Double(f64),
    Str(&'a str),
    Ext{exttype: u8, data: &'a [u8]},
    Array(&'a [EncodedElement<'a>]),
    Map(&'a [[EncodedElement<'a>; 2]]),
}

impl<'a> EncodedElement<'a> {
    /// Write a MessagePack element into `slice` beginning at `idx`, using the most efficient representation
    pub fn write_to(self, slice: &'a mut [u8], idx: usize, local_endian_fields: bool) -> usize {
        if idx >= slice.len() {
            return 0
        };
        let (_, write_slice) = slice.split_at_mut(idx);
        // Switch depending on what kind of element this is
        match self {
            Self::Nil => {
                write_slice[0] = 0xC0;
                1
            },
            Self::Int(i) => {
                // Get the smallest representation
                match get_min_size_signed(i) {
                    1 => {
                        // This number fits in one byte.  Question is, does it fit in a fixint?
                        // for positive fixint, there is no restriction other than having it be positive
                        if i > 0 {
                            write_slice[0] = i as u8;
                            1
                        } else if i < 0 && i > -32 {
                            // Use negative fixint
                            write_slice[0] = (256_i16 + i as i16) as u8; // turns negative i8 into positive u8
                            1
                        } else {
                            // No fixint
                            if write_slice.len() >= 2 {
                                write_slice[0] = 0xD0;
                                write_slice[1] = (256_i16 + i as i16) as u8; // If we got here the number has to be negative
                                2
                            } else {
                                0
                            }
                        }
                    },
                    2 => {
                        if write_slice.len() >= 3 {
                            write_slice[0] = 0xD1;
                            let bytes = if local_endian_fields {
                                (i as i16).to_ne_bytes()
                            } else {
                                (i as i16).to_be_bytes()
                            };
                            write_slice[1] = bytes[0];
                            write_slice[2] = bytes[1];
                            3
                        } else {
                            0
                        }
                    },
                    4 => {
                        if write_slice.len() >= 5 {
                            write_slice[0] = 0xD2;
                            let bytes = if local_endian_fields {
                                (i as i32).to_ne_bytes()
                            } else {
                                (i as i32).to_be_bytes()
                            };
                            write_slice[1] = bytes[0];
                            write_slice[2] = bytes[1];
                            write_slice[3] = bytes[2];
                            write_slice[4] = bytes[3];
                            5
                        } else {
                            0
                        }
                    },
                    _ => {
                        if write_slice.len() >= 9 {
                            write_slice[0] = 0xD3;
                            let bytes = if local_endian_fields {
                                (i as i64).to_ne_bytes()
                            } else {
                                (i as i64).to_be_bytes()
                            };
                            write_slice[1] = bytes[0];
                            write_slice[2] = bytes[1];
                            write_slice[3] = bytes[2];
                            write_slice[4] = bytes[3];
                            write_slice[5] = bytes[4];
                            write_slice[6] = bytes[5];
                            write_slice[7] = bytes[6];
                            write_slice[8] = bytes[7];
                            9
                        } else {
                            0
                        }
                    }
                }
            },
            Self::UInt(i) => {
                match get_min_size_unsigned(i) {
                    1 => {
                        // No fixint
                        if write_slice.len() >= 2 {
                            write_slice[0] = 0xCC;
                            write_slice[1] = i as u8;
                            2
                        } else {
                            0
                        }
                    },
                    2 => {
                        if write_slice.len() >= 3 {
                            write_slice[0] = 0xCD;
                            let bytes = if local_endian_fields {
                                (i as u16).to_ne_bytes()
                            } else {
                                (i as u16).to_be_bytes()
                            };
                            write_slice[1] = bytes[0];
                            write_slice[2] = bytes[1];
                            3
                        } else {
                            0
                        }
                    },
                    4 => {
                        if write_slice.len() >= 5 {
                            write_slice[0] = 0xCE;
                            let bytes = if local_endian_fields {
                                (i as u32).to_ne_bytes()
                            } else {
                                (i as u32).to_be_bytes()
                            };
                            write_slice[1] = bytes[0];
                            write_slice[2] = bytes[1];
                            write_slice[3] = bytes[2];
                            write_slice[4] = bytes[3];
                            5
                        } else {
                            0
                        }
                    },
                    _ => {
                        if write_slice.len() >= 9 {
                            write_slice[0] = 0xCF;
                            let bytes = if local_endian_fields {
                                (i as u64).to_ne_bytes()
                            } else {
                                (i as u64).to_be_bytes()
                            };
                            write_slice[1] = bytes[0];
                            write_slice[2] = bytes[1];
                            write_slice[3] = bytes[2];
                            write_slice[4] = bytes[3];
                            write_slice[5] = bytes[4];
                            write_slice[6] = bytes[5];
                            write_slice[7] = bytes[6];
                            write_slice[8] = bytes[7];
                            9
                        } else {
                            0
                        }
                    }
                }
            },
            Self::Bool(i) => {
                if write_slice.len() >= 1 {
                    write_slice[0] = if i { 0xC3 } else { 0xC2 };
                    1
                } else {
                    0
                }
            },
            Self::Bin(i) => {
                // Determine how big the size field needs to be first
                let size_n = get_min_size_unsigned(i.len() as u64) as usize;
                if write_slice.len() >= 1_usize + size_n + i.len() {
                    match size_n {
                        1 => {
                            write_slice[0] = 0xC4;
                            write_slice[1] = size_n as u8;
                            write_slice[2..i.len()].copy_from_slice(&i);
                        },
                        2 => {
                            write_slice[0] = 0xC5;
                            let size_bytes = (size_n as u16).to_be_bytes();
                            write_slice[1..3].copy_from_slice(&size_bytes);
                            write_slice[3..i.len()].copy_from_slice(&i);
                        },
                        4 => {
                            write_slice[0] = 0xC6;
                            let size_bytes = (size_n as u32).to_be_bytes();
                            write_slice[1..5].copy_from_slice(&size_bytes);
                            write_slice[5..i.len()].copy_from_slice(&i);
                        },
                        _ => {
                            // Too big
                            return 0;
                        }
                    };
                    1_usize + size_n + i.len()
                } else {
                    0
                }
            },
            Self::Float(i) => {
                if write_slice.len() >= 5 {
                    write_slice[0] = 0xCA;
                    let bytes = if local_endian_fields {
                        i.to_ne_bytes()
                    } else {
                        i.to_be_bytes()
                    };
                    write_slice[1..5].copy_from_slice(&bytes);
                    5
                } else {
                    0
                }
            },
            Self::Double(i) => {
                if write_slice.len() >= 9 {
                    write_slice[0] = 0xCB;
                    let bytes = if local_endian_fields {
                        i.to_ne_bytes()
                    } else {
                        i.to_be_bytes()
                    };
                    write_slice[1..9].copy_from_slice(&bytes);
                    5
                } else {
                    0
                }
            },
        }
    }
}