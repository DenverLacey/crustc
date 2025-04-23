use libc::printf;

fn main(_argc: i32, _argv: *const *const c_char) -> i32 {
    printf!("hello world\n");
    0
}

