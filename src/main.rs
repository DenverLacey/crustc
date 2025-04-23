#![no_std]
#![no_main]
// #![feature(arbitrary_self_types)]

use core::ffi::{c_char, c_int, c_double, CStr};
use core::panic::PanicInfo;

#[panic_handler]
unsafe fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

mod libc {
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
struct Foo {
    i: i32,
    f: c_double,
    s: *const CStr,
}

impl Foo {
    pub unsafe fn new(i: i32, f: c_double, s: *const CStr) -> Self {
        Self { i, f, s }
    }

    pub unsafe fn bar(me: *const Self) {
        libc::printf!(c"The Foo says '%s' with %d and %f.\n", (*(*me).s).as_ptr(), (*me).i, (*me).f);
    }
}

#[no_mangle]
unsafe extern "C" fn main(argc: c_int, argv: *const *const c_char) -> c_int {
    libc::printf!(c"hello crust!\n");

    let foo = Foo::new(52, 1.28, c"howdy do?");
    Foo::bar(&foo);

    for i in 0..argc {
        let arg = *argv.add(i as usize);
        libc::printf!(c"[%d] %s\n", i, arg);
    }

    0
}

