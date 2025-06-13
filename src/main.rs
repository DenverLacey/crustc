use quote::ToTokens;
use ra_ap_base_db::{
    salsa::Durability, BuiltCrateData, Crate, CrateOrigin, CrateWorkspaceData, Env, ExtraCrateData,
    FileSet, RootQueryDb, SourceDatabase, SourceRoot, SourceRootId,
};
use ra_ap_hir::{CfgOptions, EditionedFileId, Semantics, Symbol};
use ra_ap_ide_db::RootDatabase;
use ra_ap_paths::AbsPathBuf;
use ra_ap_syntax::{
    ast::{
        BlockExpr, Expr, HasModuleItem, HasName, HasVisibility, Item, Pat, Stmt, Type, Visibility,
        VisibilityKind,
    },
    AstNode, AstToken, TextRange,
};
use ra_ap_vfs::{Vfs, VfsPath};
use std::{
    env,
    ffi::{CString, OsString},
    fs::File,
    io::Write,
    process::Command,
};

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
        })
    }
}

impl CrustCompiler {
    fn compile_item<'a>(&self, sem: &'a Semantics<'a, RootDatabase>, item: &Item) -> syn::Item {
        match item {
            Item::Const(konst) => {
                let name = konst.name().unwrap().text().to_string();
                let Some(body) = konst.body() else {
                    todo!("bodiless const items");
                };

                let vis = self.compile_vis(konst.visibility());
                let expr = self.compile_expr(sem, &body);

                syn::Item::Const(syn::ItemConst {
                    attrs: not_implemented!(vec![], "attrs for const item"),
                    vis,
                    const_token: <syn::Token![const]>::default(),
                    ident: syn::Ident::new(&name, proc_macro2::Span::call_site()),
                    generics: not_implemented!(syn::Generics::default(), "generics for const item"),
                    colon_token: <syn::Token![:]>::default(),
                    ty: not_implemented!(Box::new(syn::Type::Never(syn::TypeNever { bang_token: <syn::Token![!]>::default() })), "ty for const item"),
                    eq_token: <syn::Token![=]>::default(),
                    expr: Box::new(expr),
                    semi_token: <syn::Token![;]>::default(),
                })
            }
            Item::Enum(_) => todo!(),
            Item::ExternBlock(_extern_block) => todo!(),
            Item::ExternCrate(_extern_crate) => todo!(),
            Item::Fn(func) => {
                let Some(body) = func.body() else {
                    todo!("bodiless functions");
                };
                syn::Item::Fn(syn::ItemFn {
                    attrs: not_implemented!(vec![], "attrs for fn items"),
                    vis: self.compile_vis(func.visibility()),
                    sig: syn::Signature {
                        constness: func.const_token().map(|_| <syn::Token![const]>::default()),
                        asyncness: func.async_token().map(|_| <syn::Token![async]>::default()),
                        unsafety: Some(<syn::Token![unsafe]>::default()),
                        abi: func.abi().map(|abi| syn::Abi {
                            extern_token: <syn::Token![extern]>::default(),
                            name: abi.abi_string().map(|name| syn::LitStr::new(&name.to_string(), proc_macro2::Span::call_site())),
                        }),
                        fn_token: <syn::Token![fn]>::default(),
                        ident: syn::Ident::new(&func.name().unwrap().text().to_string(), proc_macro2::Span::call_site()),
                        generics: not_implemented!(syn::Generics::default(), "generics for fn items"),
                        paren_token: syn::token::Paren::default(),
                        inputs: func.param_list().unwrap().params().map(|p| syn::FnArg::Typed(syn::PatType {
                            attrs: not_implemented!(vec![], "attrs for FnArg"),
                            pat: Box::new(self.compile_pat(p.pat().unwrap())),
                            colon_token: <syn::Token![:]>::default(),
                            ty: Box::new(self.compile_type(sem, p.ty().unwrap())),
                        })).collect(),
                        variadic: not_implemented!(None, "variadic arguments"),
                        output: func.ret_type().map_or(syn::ReturnType::Default, |ret| syn::ReturnType::Type(
                            <syn::Token![->]>::default(),
                            Box::new(self.compile_type(sem, ret.ty().unwrap()))
                        )),
                    },
                    block: Box::new(self.compile_block(sem, &body)),
                })
            }
            Item::Impl(_) => todo!(),
            Item::MacroCall(_macro_call) => todo!(),
            Item::MacroDef(_macro_def) => todo!(),
            Item::MacroRules(_macro_rules) => todo!(),
            Item::Module(_module) => todo!(),
            Item::Static(_) => todo!(),
            Item::Struct(strukt) => {
                let ident = syn::Ident::new(Box::leak(strukt.name().unwrap().text().to_string().into_boxed_str()), proc_macro2::Span::call_site());
                match strukt.kind() {
                    ra_ap_syntax::ast::StructKind::Record(fields) => syn::Item::Struct(syn::ItemStruct {
                        attrs: not_implemented!(vec![], "attrs for struct item"),
                        vis: self.compile_vis(strukt.visibility()),
                        struct_token: <syn::Token![struct]>::default(),
                        ident,
                        generics: not_implemented!(syn::Generics::default(), "generics for struct item"),
                        fields: syn::Fields::Named(syn::FieldsNamed {
                            brace_token: syn::token::Brace::default(),
                            named: fields.fields().map(|f| syn::Field {
                                attrs: not_implemented!(vec![], "attrs for named fields"),
                                vis: self.compile_vis(f.visibility()),
                                mutability: not_implemented!(syn::FieldMutability::None, "mutability of named fields"),
                                ident: f.name().map(|name| syn::Ident::new(
                                    Box::leak(name.text().to_string().into_boxed_str()),
                                    proc_macro2::Span::call_site(),
                                )),
                                colon_token: Some(<syn::Token![:]>::default()),
                                ty: self.compile_type(sem, f.ty().unwrap()),
                            }).collect(),
                        }) ,
                        semi_token: None,
                    }),
                    ra_ap_syntax::ast::StructKind::Tuple(fields) => syn::Item::Struct(syn::ItemStruct {
                        attrs: not_implemented!(vec![], "attrs for struct item"),
                        vis: self.compile_vis(strukt.visibility()),
                        struct_token: <syn::Token![struct]>::default(),
                        ident,
                        generics: not_implemented!(syn::Generics::default(), "generics for struct item"),
                        fields: syn::Fields::Unnamed(syn::FieldsUnnamed {
                            paren_token: syn::token::Paren::default(),
                            unnamed: fields.fields().map(|f| syn::Field {
                                attrs: not_implemented!(vec![], "attrs for unnamed fields"),
                                vis: self.compile_vis(f.visibility()),
                                mutability: not_implemented!(syn::FieldMutability::None, "mutability of unnamed fields"),
                                ident: None,
                                colon_token: None,
                                ty: self.compile_type(sem, f.ty().unwrap()),
                            }).collect(),
                        }),
                        semi_token: None,
                    }),
                    ra_ap_syntax::ast::StructKind::Unit => syn::Item::Struct(syn::ItemStruct {
                        attrs: not_implemented!(vec![], "attrs for struct item"),
                        vis: self.compile_vis(strukt.visibility()),
                        struct_token: <syn::Token![struct]>::default(),
                        ident,
                        generics: not_implemented!(syn::Generics::default(), "generics for struct item"),
                        fields: syn::Fields::Unit,
                        semi_token: None,
                    })
                }
            }
            Item::Trait(_) => todo!(),
            Item::TraitAlias(_trait_alias) => todo!(),
            Item::TypeAlias(_type_alias) => todo!(),
            Item::Union(_union) => todo!(),
            Item::Use(_) => todo!(),
        }
    }

    fn compile_vis(&self, vis: Option<Visibility>) -> syn::Visibility {
        let Some(vis) = vis else {
            return syn::Visibility::Public(<syn::Token![pub]>::default());
        };

        fn make_restricted(ident: &'static str) -> syn::Visibility {
            syn::Visibility::Restricted(syn::VisRestricted {
                pub_token: <syn::Token![pub]>::default(),
                paren_token: syn::token::Paren::default(),
                in_token: None,
                path: Box::new(syn::Path {
                    leading_colon: None,
                    segments: [syn::PathSegment {
                        ident: syn::Ident::new(ident, proc_macro2::Span::call_site()),
                        arguments: syn::PathArguments::None
                    }].into_iter().collect(),
                }),
            })
        }

        match vis.kind() {
            VisibilityKind::In(path) => syn::Visibility::Restricted(syn::VisRestricted {
                pub_token: <syn::Token![pub]>::default(),
                paren_token: syn::token::Paren::default(),
                in_token: Some(<syn::Token![in]>::default()),
                path: Box::new(self.compile_path(path)),
            }),
            VisibilityKind::PubCrate => make_restricted("crate"),
            VisibilityKind::PubSuper => make_restricted("super"),
            VisibilityKind::PubSelf => make_restricted("self"),
            VisibilityKind::Pub => syn::Visibility::Public(<syn::Token![pub]>::default()),
        }
    }

    fn compile_block<'a>(&self, _sem: &'a Semantics<'a, RootDatabase>, block: &BlockExpr) -> syn::Block {
        syn::Block {
            brace_token: syn::token::Brace::default(),
            stmts: block
                .statements()
                .map(|stmt| self.compile_stmt(_sem, &stmt))
                .chain(block.tail_expr().map(|expr| syn::Stmt::Expr(self.compile_expr(_sem, &expr), None)))
                .collect(),
        }
    }

    fn compile_stmt<'a>(&self, sem: &'a Semantics<'a, RootDatabase>, stmt: &Stmt) -> syn::Stmt {
        match stmt {
            Stmt::ExprStmt(expr) => syn::Stmt::Expr(
                self.compile_expr(sem, &expr.expr().unwrap()),
                Some(<syn::Token![;]>::default())
            ),
            Stmt::Item(item) => syn::Stmt::Item(self.compile_item(sem, item)),
            Stmt::LetStmt(let_stmt) => syn::Stmt::Local(syn::Local {
                attrs: not_implemented!(vec![], "attrs for let stmt"),
                let_token: <syn::Token![let]>::default(),
                pat: self.compile_pat(let_stmt.pat().unwrap()),
                init: let_stmt.initializer().map(|init| syn::LocalInit {
                    eq_token: <syn::Token![=]>::default(),
                    expr: Box::new(self.compile_expr(sem, &init)),
                    diverge: let_stmt.let_else().map(|le| (<syn::Token![else]>::default(), Box::new(syn::Expr::Block(syn::ExprBlock {
                        attrs: not_implemented!(vec![], "attrs for else block of let-else stmt"),
                        label: None, // TODO??
                        block: self.compile_block(sem, &le.block_expr().unwrap()),
                    })))),
                }),
                semi_token: <syn::Token![;]>::default(),
            }),
        }
    }

    fn compile_expr<'a>(&self, sem: &'a Semantics<'a, RootDatabase>, expr: &Expr) -> syn::Expr {
        match expr {
            Expr::ArrayExpr(_array_expr) => todo!(),
            Expr::AsmExpr(_asm_expr) => todo!(),
            Expr::AwaitExpr(_await_expr) => todo!(),
            Expr::BecomeExpr(_become_expr) => todo!(),
            Expr::BinExpr(_bin_expr) => todo!(),
            Expr::BlockExpr(_block_expr) => todo!(),
            Expr::BreakExpr(_break_expr) => todo!(),
            Expr::CallExpr(_call_expr) => todo!(),
            Expr::CastExpr(_cast_expr) => todo!(),
            Expr::ClosureExpr(_closure_expr) => todo!(),
            Expr::ContinueExpr(_continue_expr) => todo!(),
            Expr::FieldExpr(_field_expr) => todo!(),
            Expr::ForExpr(_for_expr) => todo!(),
            Expr::FormatArgsExpr(_format_args_expr) => todo!(),
            Expr::IfExpr(_if_expr) => todo!(),
            Expr::IndexExpr(_index_expr) => todo!(),
            Expr::LetExpr(_let_expr) => todo!(),
            Expr::Literal(lit) => match lit.kind() {
                ra_ap_syntax::ast::LiteralKind::String(s) => syn::Expr::MethodCall(syn::ExprMethodCall {
                    attrs: vec![],
                    receiver: Box::new(syn::Expr::Lit(syn::ExprLit {
                        attrs: not_implemented!(vec![], "attrs lit string expr"),
                        lit: syn::Lit::CStr(syn::LitCStr::new(
                            CString::new(s.value().expect("bad escape character").bytes().collect::<Vec<_>>()).expect("null byte in string").as_c_str(),
                            proc_macro2::Span::call_site()
                        )),
                    })),
                    dot_token: <syn::Token![.]>::default(),
                    method: syn::Ident::new("as_ptr", proc_macro2::Span::call_site()),
                    turbofish: None,
                    paren_token: syn::token::Paren::default(),
                    args: syn::punctuated::Punctuated::new(),
                }),
                ra_ap_syntax::ast::LiteralKind::ByteString(bs) => syn::Expr::Lit(syn::ExprLit {
                    attrs: not_implemented!(vec![], "attrs lit byte string expr"),
                    lit: syn::Lit::ByteStr(syn::LitByteStr::new(bs.value().expect("bad escape character").as_ref(), proc_macro2::Span::call_site())),
                }),
                ra_ap_syntax::ast::LiteralKind::CString(cs) => syn::Expr::Lit(syn::ExprLit {
                    attrs: not_implemented!(vec![], "attrs lit cstring expr"),
                    lit: syn::Lit::CStr(syn::LitCStr::new(
                        CString::new(cs.value().expect("bad escape character")).expect("null byte in cstring").as_c_str(),
                        proc_macro2::Span::call_site(),
                    )),
                }),
                ra_ap_syntax::ast::LiteralKind::IntNumber(i) => syn::Expr::Lit(syn::ExprLit {
                    attrs: not_implemented!(vec![], "attrs lit int expr"),
                    lit: syn::Lit::Int(syn::LitInt::new(i.text(), proc_macro2::Span::call_site())),
                }),
                ra_ap_syntax::ast::LiteralKind::FloatNumber(f) => syn::Expr::Lit(syn::ExprLit {
                    attrs: not_implemented!(vec![], "attrs lit float expr"),
                    lit: syn::Lit::Float(syn::LitFloat::new(f.text(), proc_macro2::Span::call_site())),
                }),
                ra_ap_syntax::ast::LiteralKind::Char(c) => syn::Expr::Lit(syn::ExprLit {
                    attrs: not_implemented!(vec![], "attrs lit char expr"),
                    lit: syn::Lit::Char(syn::LitChar::new(c.value().expect("bad escape character"), proc_macro2::Span::call_site())),
                }),
                ra_ap_syntax::ast::LiteralKind::Byte(b) => syn::Expr::Lit(syn::ExprLit {
                    attrs: not_implemented!(vec![], "attrs lit byte expr"),
                    lit: syn::Lit::Byte(syn::LitByte::new(b.value().expect("bad escape character"), proc_macro2::Span::call_site())),
                }),
                ra_ap_syntax::ast::LiteralKind::Bool(b) => syn::Expr::Lit(syn::ExprLit {
                    attrs: not_implemented!(vec![], "attrs lit bool expr"),
                    lit: syn::Lit::Bool(syn::LitBool::new(b, proc_macro2::Span::call_site())),
                }),
            },
            Expr::LoopExpr(_loop_expr) => todo!(),
            Expr::MacroExpr(_macro_expr) => todo!(),
            Expr::MatchExpr(_match_expr) => todo!(),
            Expr::MethodCallExpr(_method_call_expr) => todo!(),
            Expr::OffsetOfExpr(_offset_of_expr) => todo!(),
            Expr::ParenExpr(_paren_expr) => todo!(),
            Expr::PathExpr(path) => syn::Expr::Path(syn::PatPath {
                attrs: not_implemented!(vec![], "attrs for path expr"),
                qself: None, // TODO
                path: self.compile_path(path.path().unwrap()),
            }),
            Expr::PrefixExpr(_prefix_expr) => todo!(),
            Expr::RangeExpr(_range_expr) => todo!(),
            Expr::RecordExpr(_record_expr) => todo!(),
            Expr::RefExpr(rif) => syn::Expr::Paren(syn::ExprParen {
                attrs: not_implemented!(vec![], "attrs for ref expr"),
                paren_token: syn::token::Paren::default(),
                expr: Box::new(syn::Expr::Cast(syn::ExprCast {
                    attrs: vec![],
                    expr: Box::new(syn::Expr::Paren(syn::ExprParen {
                        attrs: vec![],
                        paren_token: syn::token::Paren::default(),
                        expr: Box::new(syn::Expr::Reference(syn::ExprReference {
                            attrs: vec![],
                            and_token: <syn::Token![&]>::default(),
                            mutability: rif.mut_token().map(|_| <syn::Token![mut]>::default()),
                            expr: Box::new(self.compile_expr(sem, &rif.expr().unwrap())),
                        })),
                    })),
                    as_token: <syn::Token![as]>::default(),
                    ty: Box::new(syn::Type::Ptr(syn::TypePtr {
                        star_token: <syn::Token![*]>::default(),
                        const_token: rif.const_token().map(|_| <syn::Token![const]>::default()),
                        mutability: rif.mut_token().map(|_| <syn::Token![mut]>::default()),
                        elem: Box::new(syn::Type::Infer(syn::TypeInfer {
                            underscore_token: <syn::Token![_]>::default(),
                        })),
                    })),
                })),
            }),
            Expr::ReturnExpr(ret) => syn::Expr::Return(syn::ExprReturn {
                attrs: not_implemented!(vec![], "attrs for return expr"),
                return_token: <syn::Token![return]>::default(),
                expr: ret.expr().map(|expr| Box::new(self.compile_expr(sem, &expr))),
            }),
            Expr::TryExpr(_try_expr) => todo!(),
            Expr::TupleExpr(_tuple_expr) => todo!(),
            Expr::UnderscoreExpr(_) => syn::Expr::Infer(syn::ExprInfer {
                attrs: not_implemented!(vec![], "attrs for underscore expr"),
                underscore_token: <syn::Token![_]>::default(),
            }),
            Expr::WhileExpr(_while_expr) => todo!(),
            Expr::YeetExpr(_yeet_expr) => todo!(),
            Expr::YieldExpr(_yield_expr) => todo!(),
        }
    }

    fn compile_type<'a>(&self, sem: &'a Semantics<'a, RootDatabase>, ty: Type) -> syn::Type {
        match ty {
            Type::ArrayType(arr_ty) => syn::Type::Array(syn::TypeArray {
                bracket_token: syn::token::Bracket::default(),
                elem: Box::new(self.compile_type(sem, arr_ty.ty().unwrap())),
                semi_token: <syn::Token![;]>::default(),
                len: self.compile_expr(sem, &arr_ty.const_arg().unwrap().expr().unwrap()),
            }),
            Type::DynTraitType(_dyn_trait_type) => todo!(),
            Type::FnPtrType(_fn_ptr_type) => todo!(),
            Type::ForType(_for_type) => todo!(),
            Type::ImplTraitType(_impl_trait_type) => todo!(),
            Type::InferType(_infer_type) => syn::Type::Infer(syn::TypeInfer {
                underscore_token: <syn::Token![_]>::default(),
            }),
            Type::MacroType(_macro_type) => todo!(),
            Type::NeverType(..) => syn::Type::Never(syn::TypeNever {
                bang_token: <syn::Token![!]>::default(),
            }),
            Type::ParenType(_paren_type) => todo!(),
            Type::PathType(path_ty) => syn::Type::Path(syn::TypePath {
                qself: None, // TODO
                path: self.compile_path(path_ty.path().unwrap()),
            }),
            Type::PtrType(ptr_ty) => syn::Type::Ptr(syn::TypePtr {
                star_token: <syn::Token![*]>::default(),
                const_token: ptr_ty.const_token().map(|_| <syn::Token![const]>::default()),
                mutability: ptr_ty.mut_token().map(|_| <syn::Token![mut]>::default()),
                elem: Box::new(self.compile_type(sem, ptr_ty.ty().unwrap())),
            }),
            Type::RefType(ref_ty) => {
                self.report_reference_type(ref_ty.syntax().text_range());
                std::process::exit(0); // TODO: Something better
            }
            Type::SliceType(slice_ty) => syn::Type::Slice(syn::TypeSlice {
                bracket_token: syn::token::Bracket::default(),
                elem: Box::new(self.compile_type(sem, slice_ty.ty().unwrap())),
            }),
            Type::TupleType(tup_ty) => syn::Type::Tuple(syn::TypeTuple {
                paren_token: syn::token::Paren::default(),
                elems: tup_ty.fields().map(|ty| self.compile_type(sem, ty)).collect(),
            }),
        }
    }

    fn compile_path(&self, path: ra_ap_syntax::ast::Path) -> syn::Path {
        syn::Path {
            leading_colon: None, // TODO
            segments: path.segments().map(|seg| {
                match seg.kind().unwrap() {
                    ra_ap_syntax::ast::PathSegmentKind::Name(name_ref) => {
                        let ident = Box::leak(name_ref.text().to_string().into_boxed_str());
                        syn::PathSegment {
                            ident: syn::Ident::new(ident, proc_macro2::Span::call_site()),
                            arguments: syn::PathArguments::None,
                        }
                    }
                    ra_ap_syntax::ast::PathSegmentKind::Type { type_ref: _, trait_ref: _ } => todo!(),
                    ra_ap_syntax::ast::PathSegmentKind::SelfTypeKw => syn::PathSegment {
                        ident: syn::Ident::new("Self", proc_macro2::Span::call_site()), // NOTE: I think this is right
                        arguments: syn::PathArguments::None,
                    },
                    ra_ap_syntax::ast::PathSegmentKind::SelfKw => syn::PathSegment {
                        ident: syn::Ident::new("self", proc_macro2::Span::call_site()),
                        arguments: syn::PathArguments::None,
                    },
                    ra_ap_syntax::ast::PathSegmentKind::SuperKw => syn::PathSegment {
                        ident: syn::Ident::new("super", proc_macro2::Span::call_site()),
                        arguments: syn::PathArguments::None,
                    },
                    ra_ap_syntax::ast::PathSegmentKind::CrateKw => syn::PathSegment {
                        ident: syn::Ident::new("crate", proc_macro2::Span::call_site()),
                        arguments: syn::PathArguments::None,
                    },
                }
            }).collect(),
        }
    }

    fn compile_pat(&self, pat: Pat) -> syn::Pat {
        match pat {
            Pat::BoxPat(_box_pat) => todo!(),
            Pat::ConstBlockPat(_const_block_pat) => todo!(),
            Pat::IdentPat(id) => syn::Pat::Ident(syn::PatIdent {
                attrs: not_implemented!(vec![], "attrs for ident pat"),
                by_ref: id.ref_token().map(|_| <syn::Token![ref]>::default()),
                mutability: id.mut_token().map(|_| <syn::Token![mut]>::default()),
                ident: syn::Ident::new(&id.name().unwrap().text().to_string(), proc_macro2::Span::call_site()),
                subpat: id.at_token().map(|_| todo!()),
            }),
            Pat::LiteralPat(_literal_pat) => todo!(),
            Pat::MacroPat(_macro_pat) => todo!(),
            Pat::OrPat(_or_pat) => todo!(),
            Pat::ParenPat(_paren_pat) => todo!(),
            Pat::PathPat(_path_pat) => todo!(),
            Pat::RangePat(_range_pat) => todo!(),
            Pat::RecordPat(_record_pat) => todo!(),
            Pat::RefPat(_ref_pat) => todo!(),
            Pat::RestPat(_rest_pat) => todo!(),
            Pat::SlicePat(_slice_pat) => todo!(),
            Pat::TuplePat(_tuple_pat) => todo!(),
            Pat::TupleStructPat(_tuple_struct_pat) => todo!(),
            Pat::WildcardPat(_wildcard_pat) => syn::Pat::Wild(syn::PatWild {
                attrs: not_implemented!(vec![], "attrs for wild card pat"),
                underscore_token: <syn::Token![_]>::default(),
            }),
        }
    }
}

impl CrustCompiler {
    fn report_reference_type(&self, span: TextRange) {
        use annotate_snippets::{Level, Renderer, Snippet};

        let message = Level::Error.title("reference type used").snippet(
            Snippet::source(self.source.as_str())
                .origin(self.source_filename.as_str())
                .annotation(Level::Error
                    .span(span.start().into()..span.end().into())
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
        report_error_not_enough_args(args);
        return;
    };

    let mut compiler = CrustCompiler::new(file.clone()).unwrap_or_else(|_| {
        let args = Box::leak(env::args()
            .collect::<Vec<_>>()
            .into_boxed_slice());
        report_error_failed_to_open_source_file(args);
        std::process::exit(0);
    });

    let source_filename = String::from(std::path::absolute(&compiler.source_filename).unwrap().to_str().unwrap());
    let virutal_path = VfsPath::new_virtual_path(source_filename);

    let mut vfs = Vfs::default();
    vfs.set_file_contents(virutal_path.clone(), Some(compiler.source.clone().into()));
    let (file_id, _) = vfs.file_id(&virutal_path).unwrap();

    let mut fileset = FileSet::default();
    fileset.insert(file_id, virutal_path);
    let source_root = SourceRoot::new_local(fileset);

    let mut db = RootDatabase::default();
    db.set_file_text(file_id, compiler.source.as_str());
    db.set_file_source_root_with_durability(file_id, SourceRootId(0), Durability::default());
    db.set_source_root_with_durability(SourceRootId(0), triomphe::Arc::new(source_root), Durability::default());

    let krate = Crate::builder(
        BuiltCrateData {
            root_file_id: file_id,
            edition: ra_ap_syntax::Edition::Edition2021,
            dependencies: vec![],
            origin: CrateOrigin::Rustc { name: Symbol::empty() },
            is_proc_macro: false,
            proc_macro_cwd: triomphe::Arc::new(AbsPathBuf::try_from("/").unwrap()),
        },
        ExtraCrateData {
            version: None,
            display_name: None,
            potential_cfg_options: None,
        },
        triomphe::Arc::new(CrateWorkspaceData {
            data_layout: Ok("".into()),
            toolchain: None,
        }),
        CfgOptions::default(),
        Env::default(),
    ).new(&db);

    db.set_all_crates(triomphe::Arc::new(Box::new([krate])));

    let sem = Semantics::new(&db);
    let ast = sem.parse(EditionedFileId::new(&db, file_id, ra_ap_syntax::Edition::Edition2021));

    for item in ast.items() {
        compiler.outfile.items.push(compiler.compile_item(&sem, &item));
    }

    let file_tokens = compiler.outfile.into_token_stream();

    let generated_filepath = format!("{}.generated.rs", file);
    let mut out = File::create(&generated_filepath).unwrap();
    write!(out, "{file_tokens}").unwrap();

    Command::new("rustfmt")
        .args([OsString::from("--edition=2021"), OsString::from(generated_filepath)])
        .output()
        .expect("Error: Failed to format code.");
}

