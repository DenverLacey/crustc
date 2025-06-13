#![allow(unused)]

use quote::{ToTokens, quote};
use std::{
    collections::HashMap, default, env, ffi::{c_char, CStr, CString, OsString}, io::Write, fs::File, path::Path, process::Command, ptr, str::FromStr, sync::{Arc, Mutex}, time::Duration
};

use syn::{self, token::Default, Token};

use ra_ap_vfs::{VfsPath, Vfs};
use ra_ap_vfs_notify::NotifyHandle;
use ra_ap_base_db::{salsa::Durability, BuiltCrateData, Crate, CrateGraphBuilder, CrateOrigin, CrateWorkspaceData, Env, ExtraCrateData, FileId, FileSet, LangCrateOrigin, RootQueryDb, SourceDatabase, SourceRoot, SourceRootId};
use ra_ap_ide_db::{symbol_index::SymbolsDatabase, LineIndexDatabase, RootDatabase};
use ra_ap_hir::{db::HirDatabase, CfgOptions, EditionedFileId, ModuleDef, Semantics, Symbol};
use ra_ap_syntax::{ast::{Expr, HasAttrs, HasModuleItem, HasName, HasVisibility, Item, Stmt, Visibility, VisibilityKind}, AstNode, SourceFile, SyntaxNode};
use ra_ap_paths::{AbsPathBuf, Utf8PathBuf};
use std::path::PathBuf;

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
    fn compile_item<'a>(&mut self, sem: &'a Semantics<'a, RootDatabase>, item: Item) {
        match item {
            Item::Const(konst) => {
                let name = konst.name().unwrap().text().to_string();
                let Some(body) = konst.body() else {
                    todo!("bodiless const items");
                };

                let vis = self.compile_vis(konst.visibility());
                let expr = self.compile_expr(sem, &body);

                self.outfile.items.push(syn::Item::Const(syn::ItemConst {
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
                }));
            }
            Item::Enum(_) => todo!(),
            Item::ExternBlock(extern_block) => todo!(),
            Item::ExternCrate(extern_crate) => todo!(),
            Item::Fn(func) => {
                let name = func.name().unwrap().text().to_string();
                println!("func: {name}");

                let body = func.body().unwrap();
                for stmt in body.statements() {
                    match stmt {
                        Stmt::ExprStmt(expr) => {
                            let expr = expr.expr().unwrap();
                            let ty = sem.type_of_expr(&expr).unwrap();
                            println!("`{expr}` :: {ty:#?}");
                        }
                        Stmt::Item(item) => todo!(),
                        Stmt::LetStmt(let_stmt) => todo!(),
                    }
                }

                if let Some(expr) = body.tail_expr() {
                    let ty = sem.type_of_expr(&expr).unwrap();
                    println!("`{expr}` :: {ty:#?}");
                }
            }
            Item::Impl(_) => todo!(),
            Item::MacroCall(macro_call) => todo!(),
            Item::MacroDef(macro_def) => todo!(),
            Item::MacroRules(macro_rules) => todo!(),
            Item::Module(module) => todo!(),
            Item::Static(_) => todo!(),
            Item::Struct(strukt) => {
                let ident = syn::Ident::new(Box::leak(strukt.name().unwrap().text().to_string().into_boxed_str()), proc_macro2::Span::call_site());
                self.outfile.items.push(match strukt.kind() {
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
                                ty: self.compile_type(f.ty().unwrap()),
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
                                ty: self.compile_type(f.ty().unwrap()),
                            }).collect(),
                        }),
                        semi_token: None,
                    }),
                    ra_ap_syntax::ast::StructKind::Unit => syn::Item::Struct(syn::ItemStruct {
                        attrs: not_implemented!(vec![], "attrs for struct item"),
                        vis: self.compile_vis(strukt.visibility()),
                        struct_token: <syn::Token![struct]>::default(),
                        ident,
                        generics: todo!(),
                        fields: syn::Fields::Unit,
                        semi_token: None,
                    })
                });
            }
            Item::Trait(_) => todo!(),
            Item::TraitAlias(trait_alias) => todo!(),
            Item::TypeAlias(type_alias) => todo!(),
            Item::Union(union) => todo!(),
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

    fn compile_stmt<'a>(&self, sem: &'a Semantics<'a, RootDatabase>, stmt: &Stmt) -> syn::Stmt {
        not_implemented!(syn::Stmt::Expr(syn::Expr::Tuple(syn::ExprTuple {
            attrs: vec![],
            paren_token: syn::token::Paren::default(),
            elems: syn::punctuated::Punctuated::default(),
        }), Some(<syn::Token![;]>::default())), "compile_stmt not implemented")
    }

    fn compile_expr<'a>(&self, sem: &'a Semantics<'a, RootDatabase>, stmt: &Expr) -> syn::Expr {
        not_implemented!(syn::Expr::Tuple(syn::ExprTuple {
            attrs: vec![],
            paren_token: syn::token::Paren::default(),
            elems: syn::punctuated::Punctuated::default(),
        }), "compile_expr not implemented")
    }

    fn compile_type(&self, ty: ra_ap_syntax::ast::Type) -> syn::Type {
        not_implemented!(syn::Type::Never(syn::TypeNever {
            bang_token: <syn::Token![!]>::default(),
        }), "compile_type not implemented")
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
                    ra_ap_syntax::ast::PathSegmentKind::Type { type_ref, trait_ref } => todo!(),
                    ra_ap_syntax::ast::PathSegmentKind::SelfTypeKw => todo!(),
                    ra_ap_syntax::ast::PathSegmentKind::SelfKw => syn::PathSegment {
                        ident: syn::Ident::new("self", proc_macro2::Span::call_site()),
                        arguments: syn::PathArguments::None
                    },
                    ra_ap_syntax::ast::PathSegmentKind::SuperKw => syn::PathSegment {
                        ident: syn::Ident::new("super", proc_macro2::Span::call_site()),
                        arguments: syn::PathArguments::None
                    },
                    ra_ap_syntax::ast::PathSegmentKind::CrateKw => syn::PathSegment {
                        ident: syn::Ident::new("crate", proc_macro2::Span::call_site()),
                        arguments: syn::PathArguments::None
                    },
                }
            }).collect(),
        }
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
    let mut source_root = SourceRoot::new_local(fileset);

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
        compiler.compile_item(&sem, item);
    }

    let file_tokens = compiler.outfile.into_token_stream();

    let generated_filepath = format!("{}.generated.rs", file);
    let mut out = File::create(&generated_filepath).unwrap();
    write!(out, "{file_tokens}");

    Command::new("rustfmt")
        .args([OsString::from(generated_filepath)])
        .output()
        .expect("Error: Failed to format code.");
}

