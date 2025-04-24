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

    {
        let file = libc::fopen(c"examples/hello/hello.rs", c"r");
        if file == core::ptr::null_mut() {
            libc::printf!(c"ERROR: Failed to read 'examples/hello/main.rs'.");
            return 1;
        }

        libc::fseek(file, 0, libc::SEEK_END);
        let filesz = libc::ftell(file);
        libc::rewind(file);

        let codebuf = libc::calloc(1, (filesz+1) as usize) as *const c_char;
        libc::fread(codebuf, 1, filesz as usize, file);

        let code = CStr::from_ptr(codebuf);
        libc::printf!(c"Source code:\n%s\n", code);

        let code_str = code.to_str().unwrap();
        let syntax = syn::parse_file(code_str).unwrap();
        print_syntax(syntax);
    }

    0
}

unsafe fn print_syntax(syntax: syn::File) {
    if let Some(shebang) = syntax.shebang {
        libc::printf!(c"Shebang:\n");
        libc::printf!(c"#!%.*s\n", shebang.len() as c_int, shebang.as_ptr());
    }

    if !syntax.attrs.is_empty() {
        libc::printf!(c"Attributes:\n");
    }
    for attr in syntax.attrs {
        match attr.meta {
            syn::Meta::Path(path) => {
                todo!("print Meta::Path");
            }
            syn::Meta::List(list) => {
                todo!("print Meta::List");
            }
            syn::Meta::NameValue(name_value) => {
                todo!("print Meta::NameValue");
            }
        }
    }

    if !syntax.items.is_empty() {
        libc::printf!(c"Items:\n");
    }
    for item in syntax.items {
        match item {
            syn::Item::Const(r#const) => {
                libc::printf!(c"Const\n");
            }
            syn::Item::Enum(r#enum) => {
                libc::printf!(c"Enum\n");
            }
            syn::Item::ExternCrate(extern_crate) => {
                libc::printf!(c"ExternCrate\n");
            }
            syn::Item::Fn(r#fn) => {
                libc::printf!(c"Fn\n");
            }
            syn::Item::ForeignMod(foreign_mod) => {
                libc::printf!(c"ForeignMod\n");
            }
            syn::Item::Impl(r#imlp) => {
                libc::printf!(c"Impl\n");
            }
            syn::Item::Macro(r#macro) => {
                libc::printf!(c"Macro\n");
            }
            syn::Item::Mod(r#mod) => {
                libc::printf!(c"Mod\n");
            }
            syn::Item::Static(r#static) => {
                libc::printf!(c"Static\n");
            }
            syn::Item::Struct(r#struct) => {
                libc::printf!(c"Struct\n");
            }
            syn::Item::Trait(r#trait) => {
                libc::printf!(c"Trait\n");
            }
            syn::Item::TraitAlias(trait_alias) => {
                libc::printf!(c"TraitAlias\n");
            }
            syn::Item::Type(r#type) => {
                libc::printf!(c"Type\n");
            }
            syn::Item::Union(r#union) => {
                libc::printf!(c"Union\n");
            }
            syn::Item::Use(r#use) => {
                libc::printf!(c"Use\n");
            }
            syn::Item::Verbatim(token_stream) => {
                libc::printf!(c"Verbatim\n");
            }
            _ => {
                libc::printf!(c"Unknown\n");
            }
        }
    }
}

