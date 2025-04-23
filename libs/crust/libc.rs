#![no_std]

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

