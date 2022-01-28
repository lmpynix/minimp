// mod encode
// no_std

type UBytes = u8;

pub enum ZeroCopyIf<'a, T: Copy> {
    Ref(&'a T),
    Val(T),
}

impl<'a, T: Copy> ZeroCopyIf<'a, T> {
    pub fn as_ref(self) -> &'a T {
        match self {
            Self::Ref(r) => r,
            Self::Val(v) => &v,
        }
    }

    pub fn as_value(self) -> T {
        match self {
            Self::Val(v) => v,
            Self::Ref(r) => *r
        }
    }
}

pub struct ArrayDecoder<'a> {
    array: &'a [u8],
    next_byte: usize,
    element_size: Option<usize>,
}

impl<'a> ArrayDecoder<'a> {
    pub fn get_next(&self) -> Option<DecodedElement<'a>> {

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