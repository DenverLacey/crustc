#![no_std]
#![no_main]
// #![feature(arbitrary_self_types)]

use core::ffi::{c_char, c_int, c_double, CStr};

#[derive(Clone, Copy)]
struct Foo {
    pub i: i32,
    pub f: c_double,
    pub s: *const CStr,
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

    {
        use ariadne::{Color, Label, Report, ReportKind, Source};

        Report::build(ReportKind::Error, 0..0)
            .with_message("Incompatible types")
            // .with_config(Config::default().with_compact(true))
            .with_label(Label::new(0..1).with_color(Color::Red))
            .with_label(
                Label::new(2..3)
                    .with_color(Color::Blue)
                    .with_message("`b` for banana")
                    .with_order(1),
            )
            .with_label(Label::new(4..5).with_color(Color::Green))
            .with_label(
                Label::new(7..9)
                    .with_color(Color::Cyan)
                    .with_message("`e` for emerald"),
            )
            .finish()
            .print(Source::from("a b c d e f"))
            .unwrap();
    }

    0
}

