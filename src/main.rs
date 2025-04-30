use std::ffi::{c_char, CStr, CString, OsString};
use std::fs::File;
use std::io::Write;
use std::process::Command;
use std::{env, ptr};
use syn::__private::ToTokens;

extern crate proc_macro2;
use proc_macro2::Span;

extern crate quote;
use quote::quote;

unsafe fn report_error_not_enough_args(args: *const [*const CStr]) {
    use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};

    let arg_len = (*(*args)[0]).to_bytes().len();

    let message = Level::ERROR.header("not enough arguments").group(
        Group::new().element(
            Snippet::source((*(*args)[0]).to_str().unwrap())
                .annotation(
                    AnnotationKind::Primary
                        .span(arg_len..arg_len)
                        .label("expected path to source file here")
                ),
        ),
    );

    let renderer = Renderer::styled();
    println!("{}", renderer.render(message));
}

unsafe fn report_error_failed_to_open_source_file(args: *const [*const CStr]) {
    use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};

    let mut line = String::new();
    for &arg in (*args).iter() {
        line.extend((*arg).to_str().unwrap().chars());
        line.push(' ');
    }

    let arg_start = (*(*args)[0]).to_bytes().len() + 1;
    let arg_end = arg_start + (*(*args)[1]).to_bytes().len();

    let message = Level::ERROR.header("failed to open source file").group(
        Group::new().element(
            Snippet::source(&line)
                .annotation(
                    AnnotationKind::Primary
                        .span(arg_start..arg_end)
                        .label("couldn't open this file")
                ),
        ),
    );

    let renderer = Renderer::styled();
    println!("{}", renderer.render(message));
}

unsafe fn change_fields_visibility<'a, I>(fields: I) where I: IntoIterator<Item = &'a mut syn::Field> {
    for field in fields {
        if matches!(field.vis, syn::Visibility::Inherited) {
            field.vis = syn::Visibility::Public(<syn::Token![pub]>::default());
        }
    }
}

unsafe fn start(args: *const [*const CStr]) {
    if args.len() <= 1 {
        report_error_not_enough_args(args);
        return;
    }

    let source_path = (*args)[1];

    let source_file = libc::fopen(source_path, c"r");
    if source_file == ptr::null_mut() {
        report_error_failed_to_open_source_file(args);
        return;
    }

    libc::fseek(source_file, 0, libc::SEEK_END);
    let filesz = libc::ftell(source_file);
    libc::rewind(source_file);

    let source_buf = libc::calloc((filesz+1) as usize, 1) as *const c_char;
    libc::fread(source_buf, 1, filesz as usize, source_file);

    libc::printf!(c"=== Source =========================================\n");
    let source = CStr::from_ptr(source_buf);
    libc::printf!(c"%s\n", source);

    libc::printf!(c"=== Parse ==========================================\n");
    let mut source_tree = syn::parse_file(source.to_str().unwrap()).unwrap();
    println!("{:#?}", source_tree);

    libc::printf!(c"=== Analysis =======================================\n");

    // TODO: Check if user already added `#![no_std]` attribute
    let mut no_std = syn::punctuated::Punctuated::new();
    no_std.push_value(syn::PathSegment {
        ident: syn::Ident::new("no_std", Span::call_site()), // `call_site` is a hack
        arguments: syn::PathArguments::None,
    });

    let mut derive = syn::punctuated::Punctuated::new();
    derive.push_value(syn::PathSegment {
        ident: syn::Ident::new("derive", Span::call_site()), // `call_site` is a hack
        arguments: syn::PathArguments::None,
    });

    source_tree.attrs.push(syn::Attribute {
        pound_token: <syn::Token![#]>::default(),
        style: syn::AttrStyle::Inner(<syn::Token![!]>::default()),
        bracket_token: syn::token::Bracket::default(),
        meta: syn::Meta::Path(syn::Path {
            leading_colon: None,
            segments: no_std,
        }),
    });

    for item in &mut source_tree.items {
        match item {
            syn::Item::Enum(item) => {
                item.attrs.push(syn::Attribute {
                    pound_token: <syn::Token![#]>::default(),
                    style: syn::AttrStyle::Outer,
                    bracket_token: syn::token::Bracket::default(),
                    meta: syn::Meta::List(syn::MetaList {
                        path: syn::Path {
                            leading_colon: None,
                            segments: derive.clone(),
                        },
                        delimiter: syn::MacroDelimiter::Paren(syn::token::Paren::default()),
                        tokens: quote!(Clone, Copy),
                    }),
                });
                if matches!(item.vis, syn::Visibility::Inherited) {
                    item.vis = syn::Visibility::Public(<syn::Token![pub]>::default());
                }
            }
            syn::Item::Fn(item) => {
                item.sig.unsafety.get_or_insert_default();
                if matches!(item.vis, syn::Visibility::Inherited) {
                    item.vis = syn::Visibility::Public(<syn::Token![pub]>::default());
                }
            }
            syn::Item::Struct(item) => {
                item.attrs.push(syn::Attribute {
                    pound_token: <syn::Token![#]>::default(),
                    style: syn::AttrStyle::Outer,
                    bracket_token: syn::token::Bracket::default(),
                    meta: syn::Meta::List(syn::MetaList {
                        path: syn::Path {
                            leading_colon: None,
                            segments: derive.clone(),
                        },
                        delimiter: syn::MacroDelimiter::Paren(syn::token::Paren::default()),
                        tokens: quote!(Clone, Copy),
                    }),
                });
                
                if matches!(item.vis, syn::Visibility::Inherited) {
                    item.vis = syn::Visibility::Public(<syn::Token![pub]>::default());
                }

                match &mut item.fields {
                    syn::Fields::Named(fields) => change_fields_visibility(&mut fields.named),
                    syn::Fields::Unnamed(fields) => change_fields_visibility(&mut fields.unnamed),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    libc::printf!(c"=== Code Generation ================================\n");
    let generated_path = CString::new(std::format!("{}.generated.rs", (*source_path).to_str().unwrap())).unwrap();
    libc::printf!(c"INFO: Outputting altered code to %s\n", generated_path.as_ptr());

    let source_tt = source_tree.into_token_stream();
    let mut out = File::create(generated_path.to_str().unwrap()).unwrap();
    write!(out, "{source_tt}").unwrap();

    libc::printf!(c"CMD: Formatting %s\n", generated_path.as_ptr());
    Command::new("rustfmt")
        .args([OsString::from(generated_path.to_str().unwrap())])
        .output()
        .expect("ERR: Failed to format generated code");
}

fn main() {
    let args = Box::leak(env::args()
        .map(|a| Box::leak(CString::new(a).unwrap().into_boxed_c_str()) as *const CStr)
        .collect::<Vec<_>>()
        .into_boxed_slice()) as *const [*const CStr];

    unsafe { start(args) }
}

