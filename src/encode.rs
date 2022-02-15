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