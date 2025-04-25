use std::ffi::{c_char, c_double, CStr};

extern crate quote;
use quote::quote;

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

// unsafe fn _test_ariadne() {
//     use ariadne::{Color, Label, Report, ReportKind, Source};
//
//     Report::build(ReportKind::Error, 0..0)
//         .with_message("Incompatible types")
//         // .with_config(Config::default().with_compact(true))
//         .with_label(Label::new(0..1).with_color(Color::Red))
//         .with_label(
//             Label::new(2..3)
//                 .with_color(Color::Blue)
//                 .with_message("`b` for banana")
//                 .with_order(1),
//         )
//         .with_label(Label::new(4..5).with_color(Color::Green))
//         .with_label(
//             Label::new(7..9)
//                 .with_color(Color::Cyan)
//                 .with_message("`e` for emerald"),
//         )
//         .finish()
//         .print(Source::from("a b c d e f"))
//         .unwrap();
//
//     libc::printf!(c"=== syn test ===\n");
//     let file = libc::fopen(c"examples/hello/hello.rs", c"r");
//     if file == core::ptr::null_mut() {
//         libc::printf!(c"ERROR: Failed to read 'examples/hello/main.rs'.");
//         return;
//     }
//
//     libc::fseek(file, 0, libc::SEEK_END);
//     let filesz = libc::ftell(file);
//     libc::rewind(file);
//
//     let codebuf = libc::calloc((filesz+1) as usize, 1) as *const c_char;
//     libc::fread(codebuf, 1, filesz as usize, file);
//
//     let code = CStr::from_ptr(codebuf);
//     libc::printf!(c"Source code:\n%s\n", code);
//
//     let code_str = code.to_str().unwrap();
//     let syntax = syn::parse_file(code_str).unwrap();
//     println!("{:#?}", syntax);
// }

unsafe fn _test_quote() {
    let code = quote! {
        unsafe fn foo(s: *const CStr) {
            let x = 0;
            libc::printf!(c"Foo: %s\n", (*s).as_ptr());
        }
    };
    println!("{code:#}");
}

unsafe fn _test_annotate_snippets() {
    use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};
    const SOURCE: &'static str = r#"let x = 5 + "hello";"#;

    let message = Level::ERROR.header("type mismatch!").group(
        Group::new().element(
            Snippet::source(SOURCE)
                .line_start(1)
                .origin("unknown")
                .fold(true)
                .annotation(
                    AnnotationKind::Primary
                        .span(12..19)
                        .label("`&str` cannot be added to an `i32`")
                ),
        ),
    );

    let renderer = Renderer::styled();
    println!("{}", renderer.render(message));
}

unsafe fn start() {
    _test_annotate_snippets();
}

fn main() {
    unsafe { start() }
}

