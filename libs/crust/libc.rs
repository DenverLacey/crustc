#![no_std]

pub use core::ffi::{c_void, c_char, c_int, c_long, c_float, c_double, CStr};

#[macro_export]
macro_rules! printf {
    ($fmt:literal $($args:tt)*) => {{
        use ::core::ffi::{c_char, c_int};
        extern "C" {
            #[link_name = "printf"]
            pub fn printf_raw(fmt: *const c_char, ...) -> c_int;
        }
        printf_raw($fmt.as_ptr() $($args)*)
    }};
}

pub type FILE = c_void;

pub const SEEK_END: c_int = 2;

extern "C" {
    #[link_name = "fopen"]
    fn fopen_raw(pathname: *const c_char, mode: *const c_char) -> *mut FILE;

    #[link_name = "fseek"]
    fn fseek_raw(stream: *mut FILE, offset: c_long, whence: c_int) -> c_int;

    #[link_name = "ftell"]
    fn ftell_raw(stream: *mut FILE) -> c_long;

    #[link_name = "fread"]
    fn fread_raw(ptr: *const c_char, size: usize, n: usize, stream: *mut FILE) -> usize;

    #[link_name = "rewind"]
    fn rewind_raw(stream: *mut FILE) -> c_void;

    #[link_name = "malloc"]
    fn malloc_raw(n: usize) -> *mut c_void;

    #[link_name = "calloc"]
    fn calloc_raw(size: usize, n: usize) -> *mut c_void;
}

pub unsafe fn fopen(pathname: *const CStr, mode: *const CStr) -> *mut FILE {
    fopen_raw((*pathname).as_ptr(), (*mode).as_ptr())
}

pub unsafe fn fseek(stream: *mut FILE, offset: c_long, whence: c_int) -> c_int {
    fseek_raw(stream, offset, whence)
}

pub unsafe fn ftell(stream: *mut FILE) -> c_long {
    ftell_raw(stream)
}

pub unsafe fn fread(ptr: *const c_char, size: usize, n: usize, stream: *mut FILE) -> usize {
    fread_raw(ptr, size, n, stream)
}

pub unsafe fn rewind(stream: *mut FILE) -> c_void {
    rewind_raw(stream)
}

pub unsafe fn malloc(n: usize) -> *mut c_void {
    malloc_raw(n)
}

pub unsafe fn calloc(size: usize, n: usize) -> *mut c_void {
    calloc_raw(size, n)
}

