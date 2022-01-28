#![no_std]

use core::iter::Iterator;

type UBytes = u8;

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
        8
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

#[derive(Copy, Clone)]
pub enum ZeroCopyIf<'a, T: Copy> {
    Ref(&'a T),
    Val(T),
}

impl<'a, T: Copy> ZeroCopyIf<'a, T> {
    #[inline]
    pub fn as_value(self) -> T {
        match self {
            Self::Val(v) => v,
            Self::Ref(r) => *r
        }
    }
}

#[derive(Copy, Clone)]
pub struct ArrayDecoder<'a> {
    header_size: UBytes, // Does not include first byte
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
        DecodedElement::from_slice_idx(self.array, idx)
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

#[derive(Copy, Clone)]
pub struct MapDecoder<'a> {
    header_size: UBytes, // Does not include first byte
    map: &'a [u8],
    elements: usize,
    next_idx: usize,
    next_map: usize,
    eob: bool
}

impl<'a> MapDecoder<'a> {
    /// Get the map starting at the given index
    fn get_at_idx(&self, idx: usize) -> Option<MapElements<'a>> {
        if let Some(key) = DecodedElement::from_slice_idx(self.map, idx) {
            // Key was decoded at the index, so determine its size and look for its value
            let value_idx = idx + key.byte_size();
            if value_idx >= self.map.len() {
                None
            } else if let Some(val) = DecodedElement::from_slice_idx(self.map, value_idx) {
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
            self.next_map < self.elements &&
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

#[derive(Copy, Clone)]
pub enum DecodedElement<'a> {
    Int{size: UBytes, val: ZeroCopyIf<'a, i64>},
    UInt{size: UBytes, val: ZeroCopyIf<'a, u64>},
    Bool(bool),
    Bin{header_size: UBytes, val: &'a[u8]},
    Float(ZeroCopyIf<'a, f32>),
    Double(ZeroCopyIf<'a, f64>),
    Str{header_size: UBytes, val: &'a str},
    Array(ArrayDecoder<'a>),
    Map(MapDecoder<'a>),
    Ext{header_size: UBytes, exttype: u8, data: &'a [u8]}
}

impl<'a> DecodedElement<'a> {
    pub fn from_slice_idx(slice: &'a [u8], idx: usize) -> Option<Self> {
        // TODO: Write me
        None
    }
    pub fn byte_size(&self) -> usize {
        /* We cannot assume that the item was expressed in the most compact form,
         * so we saved the size of the decoded element when we decoded it. */
        match self {
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
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
