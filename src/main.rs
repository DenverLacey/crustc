#![no_std]
#![no_main]
// #![feature(arbitrary_self_types)]

use core::ffi::{c_void, c_char, c_int, c_double};
use core::panic::PanicInfo;

#[panic_handler]
unsafe fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

mod libc {
    use core::ffi::c_void;
    use super::c_char;

    macro_rules! printf {
        ($fmt:literal $($args:tt)*) => {{
            use core::ffi::{c_char, c_int};
            extern "C" {
                #[link_name = "printf"]
                pub fn printf_raw(fmt: *const c_char, ...) -> c_int;
            }
            printf_raw($fmt.as_ptr() $($args)*)
        }};
    }
    pub(crate) use printf;
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Foo {
    i: i32,
    f: c_double,
    s: *const c_char,
}

impl Foo {
    pub unsafe fn new(i: i32, f: c_double, s: *const c_char) -> Self {
        Self { i, f, s }
    }

    pub unsafe fn bar(me: *const Self) {
        libc::printf!(c"The Foo says '%s' with %d and %f.\n", (*me).s, (*me).i, (*me).f);
    }
}

#[no_mangle]
unsafe extern "C" fn main(mut _argc: i32, mut _argv: *mut *mut c_char) -> i32 {
    libc::printf!(c"hello crust!\n");

    let foo = Foo::new(52, 1.28, c"howdy do?".as_ptr());
    Foo::bar(&foo);

    0
}

