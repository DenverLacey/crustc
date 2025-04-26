use std::ffi::{c_char, CStr, CString};
use std::{env, ptr};

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

unsafe fn start(args: *const [*const CStr]) {
    if args.len() <= 1 {
        report_error_not_enough_args(args);
        return;
    }

    let source_file = libc::fopen((*args)[1], c"r");
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
    let source_tree = syn::parse_file(source.to_str().unwrap()).unwrap();
    println!("{:#?}", source_tree);

    libc::printf!(c"=== Analysis =======================================\n");
    libc::printf!(c"Not yet implemented\n");
}

fn main() {
    let args = Box::leak(env::args()
        .map(|a| Box::leak(CString::new(a).unwrap().into_boxed_c_str()) as *const CStr)
        .collect::<Vec<_>>()
        .into_boxed_slice()) as *const [*const CStr];

    unsafe { start(args) }
}

