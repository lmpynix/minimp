#![no_std]

type UBytes = u8;

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

pub struct ArrayDecoder<'a> {
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
    /// Get the next element from the array, as recorded by the internal counter
    pub fn get_next(&mut self) -> Option<DecodedElement<'a>> {
        if self.next_element < self.elements {
            self.next_element += 1;
            self.get_element(self.next_element)
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
}

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

pub struct MapDecoder<'a> {
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
    /// Get the next element in the list of mappings
    pub fn get_next(&mut self) -> Option<MapElements<'a>> {
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
    /// Reset to the first element
    #[inline]
    pub fn reset(&mut self) -> () {
        self.next_map = 0;
        self.next_idx = 0;
        self.eob = false;
    }
}

pub enum DecodedElement<'a> {
    Int{size: UBytes, val: ZeroCopyIf<'a, i64>},
    UInt{size: UBytes, val: ZeroCopyIf<'a, u64>},
    Bool(bool),
    Bin{size: usize, val: &'a[u8]},
    Float(ZeroCopyIf<'a, f32>),
    Double(ZeroCopyIf<'a, f64>),
    Str(&'a str),
    Array(ArrayDecoder<'a>),
    Map(MapDecoder<'a>),
    Ext{size: usize, exttype: u8, data: &'a [u8]}
}

impl<'a> DecodedElement<'a> {
    pub fn from_slice_idx(slice: &'a [u8], idx: usize) -> Option<Self> {
        // TODO: Write me
        None
    }
    pub fn byte_size(&self) -> usize {
        // TODO: Write me
        0
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
