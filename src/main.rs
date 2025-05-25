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
    collections::HashMap, default, env, ffi::{c_char, CStr, CString, OsString}, fmt::Write, fs::File, process::Command, ptr, sync::{Arc, Mutex}, time::Duration
};

use syn::{self, Token};

macro_rules! not_implemented {
    () => {{
        println!("{}:{}: TODO: Not yet implemented.", file!(), line!());
    }};
    ($fmt:literal $($args:tt)?) => {{
        print!("{}:{}: TODO: ", file!(), line!());
        println!($fmt $($args)?);
    }};
}

struct CrustCompiler {
    outfile: syn::File,
    parsed_infos: HashMap<rustc_span::Span, rustc_ast::Item>,
}

unsafe impl Send for CrustCompiler {}
unsafe impl Sync for CrustCompiler {}

impl CrustCompiler {
    fn new() -> Self {
        Self  {
            outfile: syn::File {
                shebang: None,
                items: vec![],
                attrs: vec![
                    syn::parse_quote! { #![no_std] },
                ],
            },
            parsed_infos: HashMap::new(),
        }
    }
}

impl Callbacks for CrustCompiler {
    fn after_crate_root_parsing(
        &mut self,
        compiler: &interface::Compiler,
        krate: &mut rustc_ast::Crate
    ) -> rustc_driver::Compilation {
        use rustc_ast::ItemKind;
        for item in &krate.items {
            if let ItemKind::Fn(f) = &item.kind {
                let ident = f.ident.name.to_ident_string();
                println!("[{:?}] {}", item.span, ident);
                self.parsed_infos.insert(item.span, (**item).clone());
            }
        }
        rustc_driver::Compilation::Continue
    }

    fn after_expansion<'tcx>(
        &mut self,
        compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        for item_id in tcx.hir_free_items() {
            let item = &tcx.hir_item(item_id);
            if let Some(item) = self.parsed_infos.get(&item.span) {
                println!("We found {:?} at {:?}", item, item.span);
            } else {
                println!("We didn't find item at {:?}", item.span);
            }
            self.compile_item(tcx, item.span, &item.kind);
        }

        rustc_driver::Compilation::Stop
    }
}

impl CrustCompiler {
    fn compile_item<'tcx, 'hir>(&mut self, tcx: TyCtxt<'tcx>, span: rustc_span::Span, item: &rustc_hir::ItemKind<'hir>) {
        let Some(parsed_info) = self.parsed_infos.get(&span) else {
            return;
        };

        use rustc_hir::ItemKind as IK;
        match item {
            IK::ExternCrate(_sym, _id) => not_implemented!("ExternCrate"),
            IK::Use(_path, _kind) => not_implemented!("Use"),
            IK::Static(id, ty, _mut, body_id) => {
                let attrs = self.compile_attrs(&parsed_info.attrs);
                let vis = self.compile_vis(&parsed_info.vis);
                let mutability = self.compile_mutability(*_mut);
                let ident = syn::Ident::new(id.name.as_str(), proc_macro2::Span::call_site());
                let ty = self.compile_type(ty);

                let body = tcx.hir_body(*body_id);
                assert!(body.params.len() == 0);
                let expr = self.compile_expr(body.value);

                self.outfile.items.push(syn::Item::Static(syn::ItemStatic {
                    attrs,
                    vis,
                    static_token: <syn::Token![static]>::default(),
                    mutability,
                    ident,
                    colon_token: <syn::Token![:]>::default(),
                    ty: Box::new(ty),
                    eq_token: <syn::Token![=]>::default(),
                    expr: Box::new(expr),
                    semi_token: <syn::Token![;]>::default(),
                }));
            }
            IK::Const(_id, _ty, _generics, _body_id) => not_implemented!("Const"),
            IK::Fn { ident: _, sig, generics: _, body: _, has_body: _ } => not_implemented!("Fn"),
            IK::Macro(_id, _def, _kind) => not_implemented!("Macro"),
            IK::Mod(_id, _mod) => not_implemented!("Mod"),
            IK::ForeignMod { abi: _, items: _ } => not_implemented!("ForeignMod"),
            IK::GlobalAsm { asm: _, fake_body: _ } => not_implemented!("GlobalAsm"),
            IK::TyAlias(_id, _ty, _generics) => not_implemented!("TyAlias"),
            IK::Enum(_id, _def, _generics) => not_implemented!("Enum"),
            IK::Struct(_id, _var, _generics) => not_implemented!("Struct"),
            IK::Union(_id, _var, _generics) => not_implemented!("Union"),
            IK::Trait(_is_auto, _safety, _id, _generics, _generic_bounds, _item_refs) => not_implemented!("Trait"),
            IK::TraitAlias(_id, _generics, _generic_bounds) => not_implemented!("TraitAlias"),
            IK::Impl(_impl) => not_implemented!("Impl"),
        }
    }

    fn compile_attrs(&self, attrs: &rustc_ast::AttrVec) -> Vec<syn::Attribute> {
        todo!()
    }

    fn compile_vis(&self, vis: &rustc_ast::Visibility) -> syn::Visibility {
        use rustc_ast::VisibilityKind as VK;
        match &vis.kind {
            VK::Public => syn::Visibility::Public(<syn::Token![pub]>::default()),
            VK::Restricted { path: _, id: _, shorthand: _ }  => todo!(),
            VK::Inherited => syn::Visibility::Public(<syn::Token![pub]>::default()),
        }
    }

    fn compile_mutability(&self, mutbl: rustc_hir::Mutability) -> syn::StaticMutability {
        if matches!(mutbl, rustc_hir::Mutability::Mut) {
            syn::StaticMutability::Mut(<Token![mut]>::default())
        } else {
            syn::StaticMutability::None
        }
    }

    fn compile_type<'hir>(&self, ty: &'hir rustc_hir::Ty<'hir>) -> syn::Type {
        todo!()
    }

    fn compile_expr<'hir>(&self, expr: &'hir rustc_hir::Expr<'hir>) -> syn::Expr {
        todo!()
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

    let mut cbs = CrustCompiler::new();
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

