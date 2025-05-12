#![no_std]
#[derive(Clone, Copy)]
pub enum Enum {
    None,
    One,
    Two,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Struct {
    pub x: i32,
    pub y: i32,
    pub(self) hidden: (),
}
#[derive(Clone, Copy)]
pub struct Tuple(pub i32, pub i32);
