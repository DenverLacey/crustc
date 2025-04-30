#![no_std]
use core::ffi::CStr;
use libc::printf;
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
pub(self) unsafe fn foo(n: i32) -> i32 {
    n * 2 * (libc::rand() as f32 / libc::RAND_MAX as f32)
}
pub unsafe fn main(_argv: *const [*const CStr]) -> i32 {
    printf!("hello world\n");
    0
}
