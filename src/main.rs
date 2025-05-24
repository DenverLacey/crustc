#![allow(unused)]
#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_error_codes;
extern crate rustc_errors;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use rustc_driver::{Callbacks, run_compiler};
use rustc_interface::interface;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::ty::TyCtxt;
use std::{
    env, ffi::{c_char, CStr, CString, OsString}, fmt::Write, fs::File, process::Command, ptr, sync::{Arc, Mutex}, time::Duration
};

macro_rules! not_implemented {
    () => {{
        println!("{}:{}: TODO: Not yet implemented.", file!(), line!());
    }};
    ($fmt:literal $($args:tt)?) => {{
        print!("{}:{}: TODO: ", file!(), line!());
        println!($fmt $($args)?);
    }};
}

struct CrustCallbacks {
    outfile: syn::File,
}

unsafe impl Send for CrustCallbacks {}
unsafe impl Sync for CrustCallbacks {}

impl CrustCallbacks {
    fn new() -> Self {
        Self  {
            outfile: syn::File {
                shebang: None,
                items: vec![],
                attrs: vec![
                    syn::parse_quote! { #![no_std] },
                ],
            }
        }
    }
}

impl Callbacks for CrustCallbacks {
    fn after_expansion<'tcx>(
        &mut self,
        compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        for item_id in tcx.hir_free_items() {
            let item = &tcx.hir_item(item_id);
            compile_item(&item.kind);
        }

        rustc_driver::Compilation::Stop
    }
}

fn compile_item<'hir>(item: &rustc_hir::ItemKind<'hir>) {
    use rustc_hir::ItemKind as IK;
    match item {
        IK::ExternCrate(_sym, _id) => not_implemented!("ExternCrate"),
        IK::Use(_path, _kind) => not_implemented!("Use"),
        IK::Static(_id, _ty, _mut, _body_id) => not_implemented!("Static"),
        IK::Const(_id, _ty, _generics, _body_id) => not_implemented!("Const"),
        IK::Fn { ident: _, sig, generics: _, body: _, has_body: _ } => not_implemented!("Fn"),
        IK::Macro(_id, _def, _kind) => not_implemented!("Macro"),
        IK::Mod(_id, _mod) => not_implemented!("Mod"),
        IK::ForeignMod { abi: _, items: _ } => not_implemented!("ForeignMod "),
        IK::GlobalAsm { asm: _, fake_body: _ } => not_implemented!("GlobalAsm "),
        IK::TyAlias(_id, _ty, _generics) => not_implemented!("TyAlias"),
        IK::Enum(_id, _def, _generics) => not_implemented!("Enum"),
        IK::Struct(_id, _var, _generics) => not_implemented!("Struct"),
        IK::Union(_id, _var, _generics) => not_implemented!("Union"),
        IK::Trait(_is_auto, _safety, _id, _generics, _generic_bounds, _item_refs) => not_implemented!("Trait"),
        IK::TraitAlias(_id, _generics, _generic_bounds) => not_implemented!("TraitAlias"),
        IK::Impl(_impl) => not_implemented!("Impl"),
    }
}

fn report_error_not_enough_args(args: &[impl AsRef<str>]) {
    use annotate_snippets::{Level, Renderer, Snippet};

    let arg_len = args[0].as_ref().as_bytes().len();

    let message = Level::Error.title("not enough arguments").snippet(
        Snippet::source(args[0].as_ref())
            .annotation(
                Level::Error
                    .span(arg_len..arg_len)
                    .label("expected path to source file here")
            ),
    );

    let renderer = Renderer::styled();
    println!("{}", renderer.render(message));
}

fn report_error_failed_to_open_source_file(args: &[impl AsRef<str>]) {
    use annotate_snippets::{Level, Renderer, Snippet};

    let mut line = String::new();
    for arg in args {
        line.extend(arg.as_ref().chars());
        line.push(' ');
    }

    let arg_start = args[0].as_ref().as_bytes().len() + 1;
    let arg_end = arg_start + args[1].as_ref().as_bytes().len();

    let message = Level::Error.title("failed to open source file").snippet(
        Snippet::source(&line)
            .annotation(
                Level::Error
                    .span(arg_start..arg_end)
                    .label("couldn't open this file")
            ),
    );

    let renderer = Renderer::styled();
    println!("{}", renderer.render(message));
}

unsafe fn report_error_reference_type() {
    libc::printf!(c"UNIMPLEMENTED report_error_reference_type\n");
}

fn start(args: &[&str]) {
    if args.len() <= 1 {
        report_error_not_enough_args(args);
        return;
    }

    let source_path = args[1];

    let Ok(source) = std::fs::read_to_string(source_path) else {
        report_error_failed_to_open_source_file(args);
        return;
    };

    println!("=== Source =========================================");
    println!("{}\n", source);

    println!("=== Analysis =======================================\n");

    println!("=== Code Generation ================================\n");
}

fn main() {
    let Some(file) = env::args().nth(1) else {
        let args = Box::leak(env::args()
            .collect::<Vec<_>>()
            .into_boxed_slice());
        unsafe { report_error_not_enough_args(args) };
        return;
    };

    let mut cbs = CrustCallbacks::new();
    println!("Attrs: {:#?}", cbs.outfile.attrs);

    run_compiler(
        &[
            "ignored".to_string(),
            "--edition=2021".to_string(),
            "--extern=libc=target/debug/deps/liblibc-10ee459ca4890310.rlib".to_string(), // WARN: hardcoded path to libc in our own deps is whack
            file,
        ],
        &mut cbs,
    );

}

fn main2() {
    let args = Box::leak(env::args()
        .map(|a| Box::leak(a.into_boxed_str()) as &_)
        .collect::<Vec<_>>()
        .into_boxed_slice());
    start(args);
}

