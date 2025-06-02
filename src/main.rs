#![allow(unused)]
#![feature(rustc_private)]

extern crate rustc_abi;
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

use quote::{ToTokens, quote};
use rustc_driver::{Callbacks, run_compiler};
use rustc_interface::interface;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::ty::TyCtxt;
use std::{
    collections::HashMap, default, env, ffi::{c_char, CStr, CString, OsString}, io::Write, fs::File, path::Path, process::Command, ptr, str::FromStr, sync::{Arc, Mutex}, time::Duration
};

use syn::{self, token::Default, Token};

macro_rules! not_implemented {
    () => {{
        println!("{}:{}: TODO: Not yet implemented.", file!(), line!());
    }};
    ($fmt:literal $($args:tt)?) => {{
        print!("{}:{}: TODO: ", file!(), line!());
        println!($fmt $($args)?);
    }};
    ($default:expr, $fmt:literal $($args:tt)?) => {{
        not_implemented!($fmt $($args)?);
        $default
    }}
}

struct CrustCompiler {
    source: String,
    source_filename: String,
    outfile: syn::File,
    parsed_infos: HashMap<rustc_span::Span, rustc_ast::Item>,
}

unsafe impl Send for CrustCompiler {}
unsafe impl Sync for CrustCompiler {}

struct FailedToOpenSourceFile;

impl CrustCompiler {
    fn new(filename: impl Into<String>) -> Result<Self, FailedToOpenSourceFile> {
        let filename = filename.into();
        Ok(Self  {
            source: std::fs::read_to_string(&filename).or(Err(FailedToOpenSourceFile))?,
            source_filename: filename,
            outfile: syn::File {
                shebang: None,
                items: vec![],
                attrs: vec![
                    syn::parse_quote! { #![no_std] },
                ],
            },
            parsed_infos: HashMap::new(),
        })
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
            IK::Use(path, _kind) => {
                let path = self.compile_path_hir(path);
                not_implemented!("Use");
            }
            IK::Static(id, ty, _mut, body_id) => {
                let attrs = self.compile_attrs(&parsed_info.attrs);
                let vis = self.compile_vis(&parsed_info.vis);
                let mutability = self.compile_mutability(*_mut);
                let ident = self.compile_ident(id);
                let ty = self.compile_type_hir(ty);

                let body = tcx.hir_body(*body_id);
                assert!(body.params.len() == 0);
                let expr = self.compile_expr_hir(tcx, body.value);

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
            IK::Const(id, ty, generics, body_id) => {
                let attrs = self.compile_attrs(&parsed_info.attrs);
                let vis = self.compile_vis(&parsed_info.vis);
                let ident = self.compile_ident(id);
                let generics = self.compile_generics(generics);
                let ty = self.compile_type_hir(ty);

                let body = tcx.hir_body(*body_id);
                assert!(body.params.len() == 0);
                let expr = self.compile_expr_hir(tcx, body.value);

                self.outfile.items.push(syn::Item::Const(syn::ItemConst {
                    attrs,
                    vis,
                    const_token: <syn::Token![const]>::default(),
                    ident,
                    generics,
                    colon_token: <syn::Token![:]>::default(),
                    ty: Box::new(ty),
                    eq_token: <syn::Token![=]>::default(),
                    expr: Box::new(expr),
                    semi_token: <syn::Token![;]>::default(),
                }));
            }
            IK::Fn { ident, sig, generics, body, has_body } => {
                let attrs = self.compile_attrs(&parsed_info.attrs);
                let vis = self.compile_vis(&parsed_info.vis);
                let ident = self.compile_ident(ident);
                let constness = if sig.header.is_const() { Some(<syn::Token![const]>::default()) } else { None };
                let asyncness = if sig.header.is_async() { Some(<syn::Token![async]>::default()) } else { None };
                let abi = match sig.header.abi {
                    rustc_abi::ExternAbi::Rust => None,
                    abi => Some(syn::Abi {
                        extern_token: <syn::Token![extern]>::default(),
                        name: Some(Self::to_lit_str(abi.as_str())),
                    }),
                };
                let generics = self.compile_generics(generics);

                let rustc_ast::ItemKind::Fn(fn_info) = &parsed_info.kind else {
                    eprintln!("{}:{}: Error: parsed_info was not a Fn", file!(), line!());
                    return;
                };

                // TODO: Acually handle all the things
                let inputs: syn::punctuated::Punctuated<_, syn::Token![,]> = fn_info.sig.decl.inputs.iter().map(|param| {
                    syn::FnArg::Typed(syn::PatType {
                        attrs: vec![],
                        pat: Box::new(self.compile_pat(&param.pat)),
                        colon_token: <syn::Token![:]>::default(),
                        ty: Box::new(self.compile_type(&param.ty)),
                    })
                }).collect();

                let output = match &fn_info.sig.decl.output {
                    rustc_ast::FnRetTy::Default(_) => syn::ReturnType::Default,
                    rustc_ast::FnRetTy::Ty(ty) => syn::ReturnType::Type(<syn::Token![->]>::default(), Box::new(self.compile_type(ty))),
                };

                let variadic = if sig.decl.c_variadic {
                    Some(syn::Variadic {
                        attrs: not_implemented!(vec![], "attrs for variadics not implemented"),
                        pat: None,
                        dots: <syn::Token![...]>::default(),
                        comma: None,
                    })
                } else {
                    None
                };

                assert!(has_body, "Function decls without bodies not implemented");
                let body = tcx.hir_body(*body);
                let rustc_hir::ExprKind::Block(block, _) = &body.value.kind else {
                    eprintln!("Error: Function body is not a block.");
                    return;
                };
                let block = self.compile_block(tcx, block);

                self.outfile.items.push(syn::Item::Fn(syn::ItemFn {
                    attrs,
                    vis,
                    sig: syn::Signature {
                        constness,
                        asyncness,
                        unsafety: Some(<syn::Token![unsafe]>::default()),
                        abi,
                        fn_token: <syn::Token![fn]>::default(),
                        ident,
                        generics,
                        paren_token: syn::token::Paren::default(),
                        inputs,
                        variadic,
                        output,
                    },
                    block: Box::new(block),
                }));
            }
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

    fn compile_ident(&self, ident: &rustc_span::Ident) -> syn::Ident {
        let id_str = Box::leak(ident.name.to_ident_string().into_boxed_str());
        syn::Ident::new(id_str, proc_macro2::Span::call_site())
    }

    fn compile_attrs(&self, attrs: &rustc_ast::AttrVec) -> Vec<syn::Attribute> {
        not_implemented!(vec![], "compile_attrs() not implemented")
    }

    fn compile_attrs_hir<'a>(&self, attrs: impl IntoIterator<Item=&'a rustc_hir::Attribute>) -> Vec<syn::Attribute> {
        not_implemented!(vec![], "compile_attrs_hir() not implemented")
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

    fn compile_pat(&self, pat: &rustc_ast::Pat) -> syn::Pat {
        use rustc_ast::PatKind as PK;
        match &pat.kind {
            PK::Missing => syn::Pat::Verbatim(proc_macro2::TokenStream::new()),
            PK::Wild => syn::Pat::Wild(syn::PatWild {
                attrs: not_implemented!(vec![], "attrs for Wild Pat in compile_pat() not implemented"),
                underscore_token: <syn::Token![_]>::default(),
            }),
            PK::Ident(rustc_ast::BindingMode(_ref, _mut), id, pat) => {
                let by_ref = if matches!(_ref, rustc_ast::ByRef::Yes(_)) {
                    Some(<syn::Token![ref]>::default())
                } else {
                    None
                };

                let _mut = if matches!(_mut, rustc_ast::Mutability::Mut) {
                    Some(<syn::Token![mut]>::default())
                } else {
                    None
                };

                let ident = self.compile_ident(id);

                let subpat = if let Some(pat) = pat {
                    Some((
                        <syn::Token![@]>::default(),
                        Box::new(self.compile_pat(pat)),
                    ))
                } else {
                    None
                };

                syn::Pat::Ident(syn::PatIdent {
                    attrs: not_implemented!(vec![], "attrs for Ident Pat in compile_pat() not implemented"),
                    by_ref,
                    mutability: _mut,
                    ident,
                    subpat,
                })
            }
            PK::Struct(qself, path, fields, rest) => {
                syn::Pat::Struct(syn::PatStruct {
                    attrs: not_implemented!(vec![], "attrs for Struct Pat not implemented"),
                    qself: if let Some(qself) = qself { Some(self.compile_qself(qself)) } else { None },
                    path: self.compile_path(path),
                    brace_token: syn::token::Brace::default(),
                    fields: fields.iter().map(|field| self.compile_field_pat(field)).collect(),
                    rest: match rest {
                        rustc_ast::PatFieldsRest::Rest => Some(syn::PatRest { attrs: not_implemented!(vec![], "PatFieldsRest"), dot2_token: <syn::Token![..]>::default() }),
                        rustc_ast::PatFieldsRest::Recovered(err) => err.raise_fatal(), // TODO: Is this what we want to do?
                        rustc_ast::PatFieldsRest::None => None,
                    },
                })
            }
            PK::TupleStruct(qself, path, pats) => {
                syn::Pat::TupleStruct(syn::PatTupleStruct {
                    attrs: not_implemented!(vec![], "attrs for TupleStruct Pat not implemented"),
                    qself: if let Some(qself) = qself { Some(self.compile_qself(qself)) } else { None },
                    path: self.compile_path(path),
                    paren_token: syn::token::Paren::default(),
                    elems: pats.iter().map(|pat| self.compile_pat(pat)).collect(),
                })
            }
            PK::Or(pats) => {
                syn::Pat::Or(syn::PatOr {
                    attrs: not_implemented!(vec![], "attrs for PatOr in compile_pat() not implemented"),
                    leading_vert: None,
                    cases: pats.iter().map(|pat| self.compile_pat(pat)).collect(),
                })
            }
            PK::Path(qself, path) => {
                syn::Pat::Path(syn::PatPath {
                    attrs: not_implemented!(vec![], "attrs for PatPath in compile_pat() not implemented"),
                    qself: if let Some(qself) = qself { Some(self.compile_qself(qself)) } else { None },
                    path: self.compile_path(path),
                })
            }
            PK::Tuple(pats) => {
                syn::Pat::Tuple(syn::PatTuple {
                    attrs: not_implemented!(vec![], "attrs for PatTuple in compile_pat() not implemented"),
                    paren_token: syn::token::Paren::default(),
                    elems: pats.iter().map(|pat| self.compile_pat(pat)).collect(),
                })
            }
            PK::Box(pat) => todo!(),
            PK::Deref(_pat) => todo!(),
            PK::Ref(_pat, _mut) => todo!(),
            PK::Expr(_expr) => todo!(),
            PK::Range(start, end, rustc_span::source_map::Spanned { node: limits, .. }) => {
                syn::Pat::Range(syn::PatRange {
                    attrs: not_implemented!(vec![], "attrs for PatRange in compile_pat() not implemented"),
                    start: if let Some(start) = start { Some(Box::new(self.compile_expr(start))) } else { None },
                    limits: match limits {
                        rustc_ast::RangeEnd::Included(_) => syn::RangeLimits::Closed(<syn::Token![..=]>::default()),
                        rustc_ast::RangeEnd::Excluded => syn::RangeLimits::HalfOpen(<syn::Token![..]>::default()),
                    },
                    end: if let Some(end) = end { Some(Box::new(self.compile_expr(end))) } else { None },
                })
            }
            PK::Slice(pats) => {
                syn::Pat::Slice(syn::PatSlice {
                    attrs: not_implemented!(vec![], "attrs for PatSlice in compile_pat() not implemented"),
                    bracket_token: syn::token::Bracket::default(),
                    elems: pats.iter().map(|pat| self.compile_pat(pat)).collect(),
                })
            }
            PK::Rest => {
                syn::Pat::Rest(syn::PatRest {
                    attrs: not_implemented!(vec![], "attrs for PatRest in compile_pat() not implemented"),
                    dot2_token: <syn::Token![..]>::default(),
                })
            }
            PK::Never => todo!(),
            PK::Guard(_pat, _expr) => todo!(),
            PK::Paren(pat) => {
                syn::Pat::Paren(syn::PatParen {
                    attrs: not_implemented!(vec![], "attrs for PatParen in compile_pat() not implemented"),
                    paren_token: syn::token::Paren::default(),
                    pat: Box::new(self.compile_pat(pat)),
                })
            }
            PK::MacCall(_call) => todo!(),
            PK::Err(err) => err.raise_fatal(),
        }
    }

    fn compile_path(&self, path: &rustc_ast::Path) -> syn::Path {
        syn::Path {
            leading_colon: not_implemented!(None, "leading_colon in compile_path_hir not implemented"),
            segments: path.segments.iter().map(|seg| {
                syn::PathSegment {
                    ident: self.compile_ident(&seg.ident),
                    arguments: not_implemented!(syn::PathArguments::None, "PathArguments in compile_path not implemented")
                }
            }).collect(),
        }
    }

    fn compile_path_hir<'hir, R>(&self, path: &'hir rustc_hir::Path<'hir, R>) -> syn::Path {
        syn::Path {
            leading_colon: not_implemented!(None, "leading_colon in compile_path_hir not implemented"),
            segments: path.segments.iter().map(|seg| {
                syn::PathSegment {
                    ident: self.compile_ident(&seg.ident),
                    arguments: not_implemented!(syn::PathArguments::None, "PathArguments in compile_path_hir not implemented"),
                }
            }).collect(),
        }
    }

    fn compile_field_pat(&self, pat: &rustc_ast::PatField) -> syn::FieldPat {
        syn::FieldPat {
            attrs: self.compile_attrs(&pat.attrs),
            member: syn::Member::Named(self.compile_ident(&pat.ident)),
            colon_token: not_implemented!(Some(<syn::Token![:]>::default()), "compiling `colon_token` for compile_field_pat() not implemented"),
            pat: Box::new(self.compile_pat(&pat.pat)),
        }
    }

    fn compile_type(&self, ty: &rustc_ast::Ty) -> syn::Type {
        match &ty.kind {
            rustc_ast::TyKind::Slice(ty) => syn::Type::Slice(syn::TypeSlice {
                bracket_token: syn::token::Bracket::default(),
                elem: Box::new(self.compile_type(ty)),
            }),
            rustc_ast::TyKind::Array(ty, _const) => syn::Type::Array(syn::TypeArray {
                bracket_token: syn::token::Bracket::default(),
                elem: Box::new(self.compile_type(ty)),
                semi_token: <syn::Token![;]>::default(),
                len: self.compile_expr(&_const.value),
            }),
            rustc_ast::TyKind::Ptr(mut_ty) => syn::Type::Ptr(syn::TypePtr {
                star_token: <syn::Token![*]>::default(),
                const_token: if matches!(mut_ty.mutbl, rustc_ast::Mutability::Not) { Some(<syn::Token![const]>::default()) } else { None },
                mutability: if matches!(mut_ty.mutbl, rustc_ast::Mutability::Mut) { Some(<syn::Token![mut]>::default()) } else { None },
                elem: Box::new(self.compile_type(&mut_ty.ty)),
            }),
            rustc_ast::TyKind::Ref(..) => {
                self.report_reference_type(ty.span.data().lo.0 as usize..ty.span.data().hi.0 as usize);
                syn::Type::Never(syn::TypeNever {
                    bang_token: <syn::Token![!]>::default(),
                })
            }
            rustc_ast::TyKind::PinnedRef(..) => todo!(),
            rustc_ast::TyKind::BareFn(fn_type) => {
                not_implemented!(syn::Type::BareFn(syn::TypeBareFn {
                    lifetimes: None,
                    unsafety: Some(<syn::Token![unsafe]>::default()),
                    abi: None,
                    fn_token: <syn::Token![fn]>::default(),
                    paren_token: syn::token::Paren::default(),
                    inputs: syn::punctuated::Punctuated::new(), // TODO: Factor out compiling params
                    variadic: None,
                    output: syn::ReturnType::Default, // TODO: Factor out compiling return type
                }), "BareFn type's not implemented for compile_type()")
            }
            rustc_ast::TyKind::UnsafeBinder(_binder) => todo!(),
            rustc_ast::TyKind::Never => syn::Type::Never(syn::TypeNever {
                bang_token: <syn::Token![!]>::default(),
            }),
            rustc_ast::TyKind::Tup(types) => syn::Type::Tuple(syn::TypeTuple {
                paren_token: syn::token::Paren::default(),
                elems: types.iter().map(|ty| self.compile_type(ty)).collect(),
            }),
            rustc_ast::TyKind::Path(qself, path) => syn::Type::Path(syn::TypePath {
                qself: if let Some(qself) = qself { Some(self.compile_qself(qself)) } else { None },
                path: self.compile_path(path),
            }),
            rustc_ast::TyKind::TraitObject(_bounds, _syntax) => not_implemented!(syn::Type::TraitObject(syn::TypeTraitObject {
                dyn_token: None,
                bounds: syn::punctuated::Punctuated::new(),
            }), "compiling trait object types not implemented in compile_type()"),
            rustc_ast::TyKind::ImplTrait(_, _bounds) => not_implemented!(syn::Type::ImplTrait(syn::TypeImplTrait {
                impl_token: <syn::Token![impl]>::default(),
                bounds: syn::punctuated::Punctuated::new(),
            }), "compiling impl trait types not implemented in compile_type()"),
            rustc_ast::TyKind::Paren(ty) => syn::Type::Paren(syn::TypeParen {
                paren_token: syn::token::Paren::default(),
                elem: Box::new(self.compile_type(ty)),
            }),
            rustc_ast::TyKind::Typeof(_const) => not_implemented!(syn::Type::Never(syn::TypeNever {
                bang_token: <syn::Token![!]>::default()
            }), "compiling typeof types not implemented in compile_type()"),
            rustc_ast::TyKind::Infer => syn::Type::Infer(syn::TypeInfer {
                underscore_token: <syn::Token![_]>::default(),
            }),
            rustc_ast::TyKind::ImplicitSelf => not_implemented!(syn::Type::Never(syn::TypeNever {
                bang_token: <syn::Token![!]>::default()
            }), "compiling implicit self not implemented in compile_type()"),
            rustc_ast::TyKind::MacCall(_call) => not_implemented!(syn::Type::Never(syn::TypeNever {
                bang_token: <syn::Token![!]>::default()
            }), "compiling mac calls not implemented in compile_type()"),
            rustc_ast::TyKind::CVarArgs => syn::Type::Verbatim(quote!{ ... }.into_token_stream()),
            rustc_ast::TyKind::Pat(_ty, _ty_pat) => todo!(),
            rustc_ast::TyKind::Dummy => todo!(),
            rustc_ast::TyKind::Err(err) => err.raise_fatal(),
        }
    }

    fn compile_type_hir<'hir>(&self, ty: &'hir rustc_hir::Ty<'hir>) -> syn::Type {
        not_implemented!(syn::Type::Never(syn::TypeNever { bang_token: <syn::Token![!]>::default() }), "compile_type_hir() not implemented")
    }

    fn compile_generics<'hir>(&self, generics: &'hir rustc_hir::Generics<'hir>) -> syn::Generics {
        not_implemented!(syn::Generics::default(), "compile_generics() not implemented")
    }

    fn compile_expr(&self, expr: &rustc_ast::Expr) -> syn::Expr {
        not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
            attrs: vec![],
            paren_token: syn::token::Paren::default(),
            elems: syn::punctuated::Punctuated::new(),
        }), "compile_expr() not implemented")
    }

    fn compile_expr_hir<'tcx, 'hir>(&self, tcx: TyCtxt<'tcx>, expr: &'hir rustc_hir::Expr<'hir>) -> syn::Expr {
        let attrs = self.compile_attrs_hir(tcx.hir_attrs(expr.hir_id));
        match &expr.kind {
            rustc_hir::ExprKind::ConstBlock(block) => {
                let block = tcx.hir_body(block.body);
                let rustc_hir::ExprKind::Block(block, _) = &block.value.kind else {
                    eprintln!("Error: Function body is not a block.");
                    todo!("error handling")
                };
                let mut _block = self.compile_block(tcx, block);
                if let Some(expr) = block.expr {
                    _block.stmts.push(syn::Stmt::Expr(self.compile_expr_hir(tcx, expr), None));
                }
                syn::Expr::Const(syn::ExprConst {
                    attrs,
                    const_token: <syn::Token![const]>::default(),
                    block: _block,
                })
            }
            rustc_hir::ExprKind::Array(exprs) => syn::Expr::Array(syn::ExprArray {
                attrs,
                bracket_token: syn::token::Bracket::default(),
                elems: exprs.iter().map(|expr| self.compile_expr_hir(tcx, expr)).collect(),
            }),
            rustc_hir::ExprKind::Call(callee, args) => syn::Expr::Call(syn::ExprCall {
                attrs,
                func: Box::new(self.compile_expr_hir(tcx, callee)),
                paren_token: syn::token::Paren::default(),
                args: args.iter().map(|arg| self.compile_expr_hir(tcx, arg)).collect(),
            }),
            rustc_hir::ExprKind::MethodCall(_path_segment, _callee, _args, _) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "MethodCall not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Use(_expr, _) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Use not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Tup(exprs) => syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: exprs.iter().map(|expr| self.compile_expr_hir(tcx, expr)).collect(),
            }),
            rustc_hir::ExprKind::Binary(op, lhs, rhs) => syn::Expr::Binary(syn::ExprBinary {
                attrs,
                left: Box::new(self.compile_expr_hir(tcx, lhs)),
                op: self.compile_binop_hir(op),
                right: Box::new(self.compile_expr_hir(tcx, rhs)),
            }),
            rustc_hir::ExprKind::Unary(op, expr) => syn::Expr::Unary(syn::ExprUnary {
                attrs,
                op: self.compile_unop_hir(*op),
                expr: Box::new(self.compile_expr_hir(tcx, expr)),
            }),
            rustc_hir::ExprKind::Lit(lit) => syn::Expr::Lit(syn::ExprLit {
                attrs,
                lit: match &lit.node {
                    rustc_ast::LitKind::Str(_sym, _style) => todo!(),
                    rustc_ast::LitKind::ByteStr(_bytes, _style) => todo!(),
                    rustc_ast::LitKind::CStr(_bytes, _style) => todo!(),
                    rustc_ast::LitKind::Byte(value) => syn::Lit::new(proc_macro2::Literal::byte_character(*value)),
                    rustc_ast::LitKind::Char(value) => syn::Lit::new(proc_macro2::Literal::character(*value)),
                    rustc_ast::LitKind::Int(value, ty) => match ty {
                        rustc_ast::LitIntType::Signed(ty) => todo!(),
                        rustc_ast::LitIntType::Unsigned(ty) => todo!(),
                        rustc_ast::LitIntType::Unsuffixed => syn::Lit::new(proc_macro2::Literal::u128_unsuffixed(value.0)), // TODO: Handle other types
                    },
                    rustc_ast::LitKind::Float(_sym, _ty) => todo!(),
                    rustc_ast::LitKind::Bool(value) => syn::Lit::Bool(syn::LitBool {
                        value: *value,
                        span: proc_macro2::Span::call_site()
                    }),
                    rustc_ast::LitKind::Err(err) => err.raise_fatal(),
                },
            }),
            rustc_hir::ExprKind::Cast(_expr, _ty) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Cast not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Type(_expr, _ty) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Type not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::DropTemps(_expr) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "DropTemps not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Let(_let) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Let not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::If(_cond, _then, _else) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "If not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Loop(_cond, _label, _source, _) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Loop not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Match(_cond, _arms, _source) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Match not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Closure(_closure) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Closure not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Block(_block, _label) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Block not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Assign(_lhs, _rhs, _) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Assign not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::AssignOp(_op, _lhs, _rhs) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "AssignOp not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Field(_expr, _id) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Field not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Index(_expr, _idx, _) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Index not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Path(qpath) => syn::Expr::Path(syn::ExprPath {
                attrs,
                qself: not_implemented!(None, "compiling qself for ExprPath in compile_expr_hir() not implemented"),
                path: match qpath {
                    rustc_hir::QPath::Resolved(_ty, path) => self.compile_path_hir(path),
                    rustc_hir::QPath::TypeRelative(_ty, _segment) => todo!(),
                    rustc_hir::QPath::LangItem(_lang_item, _) => todo!(),
                },
            }),
            rustc_hir::ExprKind::AddrOf(_borrow_kind, _mutbl, _expr) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "AddrOf not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Break(_dst, _expr) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Break not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Continue(_dst) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Continue not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Ret(_expr) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Ret not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Become(_expr) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Become not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::InlineAsm(_asm) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "InlineAsm not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::OffsetOf(_ty, _ids) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "OffsetOf not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Struct(_qpath, _fields, _tail) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Struct not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Repeat(_expr, _const_arg) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Repeat not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Yield(_expr, _source) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "Yield not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::UnsafeBinderCast(_kind, _expr, _ty) => not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
                attrs,
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }), "UnsafeBinderCast not implemented for compile_exir_hir()"),
            rustc_hir::ExprKind::Err(err) => err.raise_fatal(),
        }
    }

    fn compile_stmt<'hir>(&self, stmt: &'hir rustc_hir::Stmt<'hir>) -> syn::Stmt {
        not_implemented!(syn::Stmt::Expr(syn::Expr::Tuple(syn::ExprTuple {
            attrs: vec![],
            paren_token: syn::token::Paren::default(),
            elems: syn::punctuated::Punctuated::new()
        }), None), "compile_stmt() not implemented")
    }

    fn compile_block<'tcx, 'hir>(&self, tcx: TyCtxt<'tcx>, block: &'hir rustc_hir::Block<'hir>) -> syn::Block {
        let mut stmts: Vec<_> = block.stmts.iter().map(|stmt| self.compile_stmt(stmt)).collect();

        if let Some(expr) = block.expr {
            let expr = self.compile_expr_hir(tcx, expr);
            stmts.push(syn::Stmt::Expr(expr, None));
        }

        syn::Block { brace_token: syn::token::Brace::default(), stmts }
    }

    fn compile_qself(&self, qself: &rustc_ast::QSelf) -> syn::QSelf {
        syn::QSelf {
            lt_token: <syn::Token![<]>::default(),
            ty: Box::new(self.compile_type(&qself.ty)),
            position: qself.position,
            as_token: not_implemented!(None, "compiling as_token for QSelf not implemented"),
            gt_token: <syn::Token![>]>::default(),
        }
    }

    fn compile_unop_hir(&self, unop: rustc_hir::UnOp) -> syn::UnOp {
        match unop {
            rustc_hir::UnOp::Deref => syn::UnOp::Deref(<syn::Token![*]>::default()),
            rustc_hir::UnOp::Not => syn::UnOp::Not(<syn::Token![!]>::default()),
            rustc_hir::UnOp::Neg => syn::UnOp::Neg(<syn::Token![-]>::default()),
        }
    }

    fn compile_binop_hir(&self, binop: &rustc_hir::BinOp) -> syn::BinOp {
        match &binop.node {
            rustc_hir::BinOpKind::Add => syn::BinOp::Add(<syn::Token![+]>::default()),
            rustc_hir::BinOpKind::Sub => syn::BinOp::Sub(<syn::Token![-]>::default()),
            rustc_hir::BinOpKind::Mul => syn::BinOp::Mul(<syn::Token![*]>::default()),
            rustc_hir::BinOpKind::Div => syn::BinOp::Div(<syn::Token![/]>::default()),
            rustc_hir::BinOpKind::Rem => syn::BinOp::Rem(<syn::Token![%]>::default()),
            rustc_hir::BinOpKind::And => syn::BinOp::And(<syn::Token![&&]>::default()),
            rustc_hir::BinOpKind::Or => syn::BinOp::Or(<syn::Token![||]>::default()),
            rustc_hir::BinOpKind::BitXor => syn::BinOp::BitXor(<syn::Token![^]>::default()),
            rustc_hir::BinOpKind::BitAnd => syn::BinOp::BitAnd(<syn::Token![&]>::default()),
            rustc_hir::BinOpKind::BitOr => syn::BinOp::BitOr(<syn::Token![|]>::default()),
            rustc_hir::BinOpKind::Shl => syn::BinOp::Shl(<syn::Token![<<]>::default()),
            rustc_hir::BinOpKind::Shr => syn::BinOp::Shr(<syn::Token![>>]>::default()),
            rustc_hir::BinOpKind::Eq => syn::BinOp::Eq(<syn::Token![==]>::default()),
            rustc_hir::BinOpKind::Lt => syn::BinOp::Lt(<syn::Token![<]>::default()),
            rustc_hir::BinOpKind::Le => syn::BinOp::Le(<syn::Token![<=]>::default()),
            rustc_hir::BinOpKind::Ne => syn::BinOp::Ne(<syn::Token![!=]>::default()),
            rustc_hir::BinOpKind::Ge => syn::BinOp::Ge(<syn::Token![>=]>::default()),
            rustc_hir::BinOpKind::Gt => syn::BinOp::Gt(<syn::Token![>]>::default()),
        }
    }

    fn to_lit_str(s: impl AsRef<str>) -> syn::LitStr {
        let s = Box::leak(s.as_ref().to_owned().into_boxed_str());
        syn::LitStr::new(s, proc_macro2::Span::call_site())
    }
}

impl CrustCompiler {
    fn report_reference_type(&self, span: std::ops::Range<usize>) {
        use annotate_snippets::{Level, Renderer, Snippet};

        let message = Level::Error.title("reference type used").snippet(
            Snippet::source(self.source.as_str())
                .origin(self.source_filename.as_str())
                .annotation(Level::Error
                    .span(span.clone())
                    .label("reference types are not allowed in crust"))
        )
        .footer(Level::Help.title("try using pointers"));

        let renderer = Renderer::styled();
        println!("{}", renderer.render(message));
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

fn main() {
    let Some(file) = env::args().nth(1) else {
        let args = Box::leak(env::args()
            .collect::<Vec<_>>()
            .into_boxed_slice());
        unsafe { report_error_not_enough_args(args) };
        return;
    };

    let mut compiler = CrustCompiler::new(file.clone()).unwrap_or_else(|_| {
        let args = Box::leak(env::args()
            .collect::<Vec<_>>()
            .into_boxed_slice());
        report_error_failed_to_open_source_file(args);
        std::process::exit(1);
    });

    run_compiler(
        &[
            "ignored".to_string(),
            "--edition=2021".to_string(),
            "--extern=libc=target/debug/deps/liblibc-10ee459ca4890310.rlib".to_string(), // WARN: hardcoded path to libc in our own deps is whack
            file.clone(),
        ],
        &mut compiler,
    );

    let file_tokens = compiler.outfile.into_token_stream();

    let generated_filepath = format!("{}.generated.rs", file);
    let mut out = File::create(&generated_filepath).unwrap();
    write!(out, "{file_tokens}");

    Command::new("rustfmt")
        .args([OsString::from(generated_filepath)])
        .output()
        .expect("Error: Failed to format code.");
}

