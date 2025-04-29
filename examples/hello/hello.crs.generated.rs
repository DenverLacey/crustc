#![no_std]
use core::ffi::CStr;
use libc::printf;
unsafe fn main(_argv: *const [*const CStr]) -> i32 {
    printf!("hello world\n");
    0
}
