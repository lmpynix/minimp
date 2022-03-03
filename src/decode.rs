// mod decode

use core::iter::Iterator;
use core::str;
use super::bytesize::*;


#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ArrayDecoder<'a> {
    header_size: UBytes, // Does not include first byte
    local_endian_fields: bool,
    array: &'a [u8], // First element of this needs to be the first data byte
    elements: usize,
    element_size: Option<usize>,
    next_element: usize,
    eob: bool,
}

impl<'a> ArrayDecoder<'a> {
    /// Get the array element beginning at a specific byte index
    #[inline]
    fn get_at_idx(&self, idx: usize) -> Option<DecodedElement<'a>> {
        DecodedElement::from_slice_idx(self.array, idx, self.local_endian_fields)
    }
    /// Get the array index from the element index
    fn idx_from_element(&mut self, element: usize) -> Option<usize> {
        // First, we need to calculate the element size if it hasn't been done yet
        let elsize;
        if let Some(s) = &self.element_size {
            elsize = *s;
        } else {
            if let Some(s) = self.get_at_idx(0) {
                elsize = s.byte_size();
                self.element_size = Some(elsize);
            } else {
                return None;
            };
        }
        // Next, check and see if this element is in bounds
        if element >= self.elements {
            return None;
        }
        // Use the element size to calculate the index
        let start_idx = elsize * element;
        if start_idx >= self.array.len() {
            // This isn't valid and we should not return anything, and mark eob
            self.eob = true;
            None
        } else {
            Some(start_idx)
        }
    }
    /// Get a specific element from the array
    #[inline]
    pub fn get_element(&mut self, element: usize) -> Option<DecodedElement<'a>> {
        if element >= self.elements {
            None
        } else if let Some(idx) = &self.idx_from_element(element) {
            self.get_at_idx(*idx)
        } else {
            None
        }
    }
    /// Reset the "next" element to the beginning
    #[inline]
    pub fn reset(&mut self) -> () {
        self.next_element = 0;
        self.eob = false;
    }

    pub fn byte_size(&self) -> usize {
        // Clone ourselves and iterate over the clone
        let mut new_self = self.clone();
        new_self.reset();
        let mut data_size = 0;
        for element in new_self {
            data_size += element.byte_size();
        }
        data_size + self.header_size as usize + 1
    }
}

/// We don't have to consume arrays in-order but having an iterator is convenient
impl<'a> Iterator for ArrayDecoder<'a> {
    type Item = DecodedElement<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_element < self.elements {
            self.next_element += 1;
            self.get_element(self.next_element)
        } else {
            None
        }
    }
}

#[derive(Copy, Clone)]
pub struct MapElements<'a> {
    key: DecodedElement<'a>,
    value: DecodedElement<'a>,
}

impl<'a> MapElements<'a> {
    #[inline]
    pub fn byte_size(&self) -> usize {
        self.key.byte_size() + self.value.byte_size()
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct MapDecoder<'a> {
    header_size: UBytes, // Does not include first byte
    local_endian_fields: bool,
    map: &'a [u8],
    elements: usize,
    next_idx: usize,
    next_map: usize,
    eob: bool
}

impl<'a> MapDecoder<'a> {
    /// Get the map starting at the given index
    fn get_at_idx(&self, idx: usize) -> Option<MapElements<'a>> {
        if let Some(key) = DecodedElement::from_slice_idx(self.map, idx, self.local_endian_fields) {
            // Key was decoded at the index, so determine its size and look for its value
            let value_idx = idx + key.byte_size();
            if value_idx >= self.map.len() {
                None
            } else if let Some(val) = DecodedElement::from_slice_idx(self.map, value_idx, self.local_endian_fields) {
                Some(MapElements {
                    key,
                    value: val
                })
            } else {
                None
            }
        } else {
            None
        }

    }
    /// Reset to the first element
    #[inline]
    pub fn reset(&mut self) -> () {
        self.next_map = 0;
        self.next_idx = 0;
        self.eob = false;
    }
    /// Get the total size of the map.
    /// 
    /// This operation is very (comparatively) expensive!  It requires consuming all of the map elements in order.
    pub fn byte_size(&self) -> usize {
        // Clone a new copy of ourselves such that we can reset it and use it
        let mut new_self = self.clone();
        new_self.reset();
        let mut data_size = 0;
        for map in new_self {
            data_size += map.byte_size();
        }
        data_size + self.header_size as usize + 1
    }
}

/// As we have to consume the map sequentially, it makes sense to use it as an iterator
impl<'a> Iterator for MapDecoder<'a> {
    type Item = MapElements<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if  self.next_idx < self.map.len() && 
            self.next_map < self.elements/2 &&
            !self.eob
        {
            let map_opt = self.get_at_idx(self.next_idx);
            if let Some(map) = &map_opt {
                self.next_idx += map.byte_size();
                if self.next_idx >= self.map.len() {
                    // This is the end of the map so set eob
                    self.eob = true;
                };
                self.next_map += 1;
                if self.next_map >= self.elements {
                    // This is also the end of the map so set eob
                    self.eob = true;
                };
                map_opt
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum DecodedElement<'a> {
    Nil,
    Int{size: UBytes, val: i64},
    UInt{size: UBytes, val: u64},
    Bool(bool),
    Bin{header_size: UBytes, val: &'a[u8]},
    Float(f32),
    Double(f64),
    Str{header_size: UBytes, val: &'a str},
    Array(ArrayDecoder<'a>),
    Map(MapDecoder<'a>),
    Ext{header_size: UBytes, exttype: u8, data: &'a [u8]}
}

impl<'a> DecodedElement<'a> {
    /// Decode a MessagePack element that begins at `idx` in `slice`.
    pub fn from_slice_idx(slice: &'a [u8], idx: usize, local_endian_fields: bool) -> Option<Self> {
        /* Like most binary decoders, this is one whole big match expression.
         * We take the header byte, figure out what kind of field it is, and (assuming it is valid) create
         * a DecodedElement from it.
         * One big wrinkle here is Endianness.  Normally, we would prefer to just avoid copying
         * out of the buffer, and read values directly from the memory.  However, in order to do this,
         * there would have to be some way of lazily evaluating the conversion.  There's not any real
         * benefit to this, though.  So, I have elected to just convert and copy everything that is not
         * big enough to need its own buffer.
         */
        // First, attempt to match the fixints, since they're not easy to do with the match arms 
        if slice[idx] <= 0x7f {
            // This is a positive fixint
            Some(Self::Int{size: 0, val: slice[idx] as i64})
        } else if slice[idx] > 0xE0 {
            // This is a negative fixint
            Some(Self::Int{size: 0, val: (slice[idx] as i64) - 256})
        } else if slice[idx] >= 0x80 && slice[idx] <= 0x8F {
            // Fixmap
            let elements: usize = (slice[idx] & 0x0F) as usize;
            let decoder = MapDecoder {
                header_size: 0,
                local_endian_fields,
                elements,
                eob: false,
                map: &slice[idx+1..],
                next_idx: 0,
                next_map: 0,
            };
            Some(Self::Map(decoder))
        } else if slice[idx] >= 0x90 && slice[idx] <= 0x9F {
            // Fixarray
            let elements: usize = (slice[idx] & 0x0F) as usize;
            let decoder = ArrayDecoder {
                header_size: 0,
                local_endian_fields,
                elements,
                array: &slice[idx+1..],
                next_element: 0,
                element_size: None,
                eob: false
            };
            Some(Self::Array(decoder))
        } else if slice[idx] >= 0xA0 && slice[idx] <= 0xBF {
            // Fixstr
            let length: usize = (slice[idx] & 0x1F) as usize;
            // Check that we have enough length
            if idx + length < slice.len() {
                if let Ok(s) = str::from_utf8(&slice[idx+1..idx+1+length]) {
                    Some(Self::Str{header_size: 0, val: s})
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            match slice[idx] {
                // Nil
                0xC0 => Some(Self::Nil),
                // Unsigned ints
                0xCC => {
                    // 8-bit uint
                    if idx+1 < slice.len() {
                        Some(Self::UInt{size: 1, val: slice[idx+1] as u64})
                    } else {
                        None
                    }
                },
                0xCD => {
                    // 16-bit uint
                    if idx+2 < slice.len() {
                        // Attempt to derive a u16 from this
                        if let Ok(uint_bytes) = slice[idx+1..idx+3].try_into() {
                            if local_endian_fields {
                                Some(Self::UInt{size: 2, val: u16::from_le_bytes(uint_bytes) as u64})
                            } else {
                                Some(Self::UInt{size: 2, val: u16::from_be_bytes(uint_bytes) as u64})
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xCE => {
                    // 32-bit uint
                    if idx+4 < slice.len() {
                        // Attempt to derive a u32 from this
                        if let Ok(uint_bytes) = slice[idx+1..idx+5].try_into() {
                            if local_endian_fields {
                                Some(Self::UInt{size: 4, val: u32::from_le_bytes(uint_bytes) as u64})
                            } else {
                                Some(Self::UInt{size: 4, val: u32::from_be_bytes(uint_bytes) as u64})
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xCF => {
                    // 64-bit uint
                    if idx+8 < slice.len() {
                        // Attempt to derive a u64 from this
                        if let Ok(uint_bytes) = slice[idx+1..idx+9].try_into() {
                            if local_endian_fields {
                                Some(Self::UInt{size: 8, val: u64::from_le_bytes(uint_bytes) as u64})
                            } else {
                                Some(Self::UInt{size: 8, val: u64::from_be_bytes(uint_bytes) as u64})
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                // Signed Ints
                0xD0 => {
                    // 8-bit int
                    if idx+1 < slice.len() {
                        let bytes: [u8; 2] = [0x0, slice[idx+1]];
                        Some(Self::Int{size: 1, val: i16::from_be_bytes(bytes) as i64})
                    } else {
                        None
                    }
                },
                0xD1 => {
                    // 16-bit int
                    if idx+2 < slice.len() {
                        // Attempt to derive a u16 from this
                        if let Ok(int_bytes) = slice[idx+1..idx+3].try_into() {
                            if local_endian_fields {
                                Some(Self::Int{size: 2, val: i16::from_le_bytes(int_bytes) as i64})
                            } else {
                                Some(Self::Int{size: 2, val: i16::from_be_bytes(int_bytes) as i64})
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xD2 => {
                    // 32-bit int
                    if idx+4 < slice.len() {
                        // Attempt to derive a i32 from this
                        if let Ok(int_bytes) = slice[idx+1..idx+5].try_into() {
                            if local_endian_fields {
                                Some(Self::Int{size: 4, val: i32::from_le_bytes(int_bytes) as i64})
                            } else {
                                Some(Self::Int{size: 4, val: i32::from_be_bytes(int_bytes) as i64})
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xD3 => {
                    // 64-bit int
                    if idx+8 < slice.len() {
                        // Attempt to derive a i64 from this
                        if let Ok(int_bytes) = slice[idx+1..idx+9].try_into() {
                            if local_endian_fields {
                                Some(Self::Int{size: 8, val: i64::from_le_bytes(int_bytes) as i64})
                            } else {
                                Some(Self::Int{size: 8, val: i64::from_be_bytes(int_bytes) as i64})
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                // Booleans
                0xC2 => Some(Self::Bool(false)),
                0xC3 => Some(Self::Bool(true)),
                // Floats
                0xCA => {
                    // f32
                    if idx+4 < slice.len() {
                        // Attempt to derive an f32 from this
                        if let Ok(float_bytes) = slice[idx+1..idx+5].try_into() {
                            if local_endian_fields {
                                Some(Self::Float(f32::from_le_bytes(float_bytes)))
                            } else {
                                Some(Self::Float(f32::from_be_bytes(float_bytes)))
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xCB => {
                    // f64
                    if idx+8 < slice.len() {
                        // Attempt to derive an f64 from this
                        if let Ok(float_bytes) = slice[idx+1..idx+9].try_into() {
                            if local_endian_fields {
                                Some(Self::Double(f64::from_le_bytes(float_bytes)))
                            } else {
                                Some(Self::Double(f64::from_be_bytes(float_bytes)))
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xD9 => {
                    // str 8
                    if idx+1 < slice.len() {
                        let length: usize  = slice[idx+1] as usize;
                        // Build a slice from the given information
                        if idx + 1 + length < slice.len() {
                            if let Ok(s) = str::from_utf8(&slice[idx+2..idx+2+length]) {
                                Some(Self::Str{header_size: 1, val: s})
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xDA => {
                    // str 16
                    let size: usize = if let Ok(size_bytes) = slice[idx+1..idx+3].try_into() {
                            if local_endian_fields {
                                u16::from_le_bytes(size_bytes) as usize
                            } else {
                                u16::from_be_bytes(size_bytes) as usize
                            }
                    } else {
                        return None;
                    };
                    if idx+2+size < slice.len() {
                        if let Ok(s) = str::from_utf8(&slice[idx+3..idx+3+size]) {
                            Some(Self::Str{header_size: 2, val: s})
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xDB => {
                    // str 32
                    let size: usize = if let Ok(size_bytes) = slice[idx+1..idx+5].try_into() {
                            if local_endian_fields {
                                u32::from_le_bytes(size_bytes) as usize
                            } else {
                                u32::from_be_bytes(size_bytes) as usize
                            }
                    } else {
                        return None;
                    };
                    if idx+4+size < slice.len() {
                        if let Ok(s) = str::from_utf8(&slice[idx+5..idx+5+size]) {
                            Some(Self::Str{header_size: 4, val: s})
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xC4 => {
                    // bin 8
                    if idx+1 < slice.len() {
                        let length: usize  = slice[idx+1] as usize;
                        // Build a slice from the given information
                        if idx + 1 + length < slice.len() {
                            Some(Self::Bin{header_size: 1, val: &slice[idx+2..idx+2+length]})
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xC5 => {
                    // bin 16
                    let size: usize = if let Ok(size_bytes) = slice[idx+1..idx+3].try_into() {
                            if local_endian_fields {
                                u16::from_le_bytes(size_bytes) as usize
                            } else {
                                u16::from_be_bytes(size_bytes) as usize
                            }
                    } else {
                        return None;
                    };
                    if idx+2+size < slice.len() {
                        Some(Self::Bin{header_size: 2, val: &slice[idx+3..idx+3+size]})
                    } else {
                        None
                    }
                },
                0xC6 => {
                    // bin 32
                    let size: usize = if let Ok(size_bytes) = slice[idx+1..idx+5].try_into() {
                            if local_endian_fields {
                                u32::from_le_bytes(size_bytes) as usize
                            } else {
                                u32::from_be_bytes(size_bytes) as usize
                            }
                    } else {
                        return None;
                    };
                    if idx+4+size < slice.len() {
                        Some(Self::Bin{header_size: 4, val: &slice[idx+5..idx+5+size]})
                    } else {
                        None
                    }
                },
                // EXT fields: like bin except they have a 1 byte tag that comes with them
                0xC7 => {
                    // ext 8
                    if idx+2 < slice.len() {
                        let length: usize  = slice[idx+1] as usize;
                        let t: u8 = slice[idx+2];
                        // Build a slice from the given information
                        if idx + 1 + length < slice.len() {
                            Some(Self::Ext{header_size: 1, exttype: t, data: &slice[idx+3..idx+3+length]})
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                0xC8 => {
                    // ext 16
                    let size: usize = if let Ok(size_bytes) = slice[idx+1..idx+3].try_into() {
                            if local_endian_fields {
                                u16::from_le_bytes(size_bytes) as usize
                            } else {
                                u16::from_be_bytes(size_bytes) as usize
                            }
                    } else {
                        return None;
                    };
                    if idx+3+size < slice.len() {
                        let t: u8 = slice[idx+3];
                        Some(Self::Ext{header_size: 2, exttype: t, data: &slice[idx+4..idx+4+size]})
                    } else {
                        None
                    }
                },
                0xC9 => {
                    // ext 32
                    let size: usize = if let Ok(size_bytes) = slice[idx+1..idx+5].try_into() {
                            if local_endian_fields {
                                u32::from_le_bytes(size_bytes) as usize
                            } else {
                                u32::from_be_bytes(size_bytes) as usize
                            }
                    } else {
                        return None;
                    };
                    if idx+5+size < slice.len() {
                        let t: u8 = slice[idx+4];
                        Some(Self::Ext{header_size: 4, exttype: t, data: &slice[idx+5..idx+5+size]})
                    } else {
                        None
                    }
                },
                // Fixext
                0xD4 => {
                    // fixext 1
                    if idx+2 < slice.len() {
                        Some(Self::Ext{header_size: 0, exttype: slice[idx+1], data: &slice[idx+2..idx+3]})
                    } else {
                        None
                    }
                },
                0xD5 => {
                    // fixext 2
                    if idx+4 < slice.len() {
                        Some(Self::Ext{header_size: 0, exttype: slice[idx+1], data: &slice[idx+2..idx+4]})
                    } else {
                        None
                    }
                },
                0xD6 => {
                    // fixext 4
                    if idx+6 < slice.len() {
                        Some(Self::Ext{header_size: 0, exttype: slice[idx+1], data: &slice[idx+2..idx+6]})
                    } else {
                        None
                    }
                },
                0xD7 => {
                    // fixext 8
                    if idx+10 < slice.len() {
                        Some(Self::Ext{header_size: 0, exttype: slice[idx+1], data: &slice[idx+2..idx+10]})
                    } else {
                        None
                    }
                },
                0xD8 => {
                    // fixext 8
                    if idx+18 < slice.len() {
                        Some(Self::Ext{header_size: 0, exttype: slice[idx+1], data: &slice[idx+2..idx+18]})
                    } else {
                        None
                    }
                },
                _ => None
            }
        }
    }
    /// Get the size, in bytes, of the MesagePack representation this element was decoded from
    pub fn byte_size(&self) -> usize {
        /* We cannot assume that the item was expressed in the most compact form,
         * so we saved the size of the decoded element when we decoded it. */
        match self {
            Self::Nil => 1,
            Self::Int{size: s, val: _} => *s as usize + 1, // Always one overhead byte for Int and Uint, because 0 for size is an option (fixint)
            Self::UInt{size: s, val: _} => *s as usize + 1,
            Self::Bool(_) => 1,
            Self::Bin{header_size: hs, val: v} => *hs as usize + v.len() as usize + 1,
            Self::Float(_) => 5,
            Self::Double(_) => 9,
            Self::Str{header_size: hs, val: v} => *hs as usize + v.len() as usize + 1,
            Self::Ext{header_size: hs, data: d, ..} => *hs as usize + d.len() as usize + 2,
            Self::Array(a) => a.byte_size(),
            Self::Map(m) => m.byte_size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nil_decode() {
        let t: [u8; 1] = [0xC0];
        if let Some(DecodedElement::Nil) = DecodedElement::from_slice_idx(&t, 0, false) {
            
        } else {
            panic!()
        }
    }

    #[test]
    fn int_decode() {
        let a: [u8; 1] = [0b00001000]; // fixint positive 8
        let b: [u8; 1] = [0b11111110]; // fixint negative 2
        let c: [u8; 3] = [0xCD, 0x27, 0x3A]; // uint16 10042
        let d: [u8; 5] = [0xD2, 0xFF, 0xFF, 0xFF, 0xFC]; // int32 -4
        assert_eq!(Some(DecodedElement::Int{size: 0, val: 8}), DecodedElement::from_slice_idx(&a, 0, false));
        assert_eq!(Some(DecodedElement::Int{size: 0, val: -2}), DecodedElement::from_slice_idx(&b, 0, false));
        assert_eq!(Some(DecodedElement::UInt{size: 2, val: 10042}), DecodedElement::from_slice_idx(&c, 0, false));
        assert_eq!(Some(DecodedElement::Int{size: 4, val: -4}), DecodedElement::from_slice_idx(&d, 0, false));
    }

    #[test]
    fn float_decode() {
        let a_num: [u8; 4] = 3.1415926535_f32.to_be_bytes();
        let a: [u8; 5] = [0xCA, a_num[0], a_num[1], a_num[2], a_num[3]];
        let b_num: [u8; 8] = (22_f64/7_f64).to_be_bytes();
        let b: [u8; 9] = [0xCB, b_num[0], b_num[1], b_num[2], b_num[3], b_num[4], b_num[5], b_num[6], b_num[7]];
        assert_eq!(Some(DecodedElement::Float(3.1415926535_f32)), DecodedElement::from_slice_idx(&a, 0, false));
        assert_eq!(Some(DecodedElement::Double(22_f64/7_f64)), DecodedElement::from_slice_idx(&b, 0, false));
    }

    #[test]
    fn overrun_safety() {
        let a: [u8; 2] = [0xCD, 0x00]; // too short int
        let b: [u8; 4] = [0xCB, 0xFF, 0xEC, 0xEB]; // too short float
        // None of these should panic
        assert_eq!(None, DecodedElement::from_slice_idx(&a, 0, false));
        assert_eq!(None, DecodedElement::from_slice_idx(&b, 0, false));
    }
}
