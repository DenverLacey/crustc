# Crust

A transpiler from 'Crust' to Rust source code written in the Crust style as specified by [Tsoding](https://www.twitch.tv/tsoding).

* [Tsoding video about crust](https://youtu.be/5MIsMbFjvkw?si=ziAF2PT7Aj4IC1an)
* [Original crust repo](https://github.com/tsoding/Crust/tree/main)
* [BartMessey's nocargo repo](https://github.com/BartMassey/nocargo)

# Rules of Crust

1. Every function is unsafe.
1. No references, only pointers.
1. No cargo, build with rustc directly.
1. No std, libc is allowed.
1. Only Edition 2021.
2. All user structs \#\[derive(Clone, Copy)].
3. Everything is pub by default.

> "*These rules may change. The goal is to make programming in Rust fun.*" - Tsoding

# 'Crust' Syntax

Part of this project is coming up with a syntax for 'Crust' which is really just a style of writting Rust code. 
'Crust' tries to be as close to Rust as possible, with main differences being between safety, visibility, and ergonomics of pointers.

For example, the following 'Crust code.

```rust
use libc::printf;

struct Foo {
    i: i32,
    f: f32,
}

fn print_foo(foo: *const Foo) {
    printf!("(%d, %f)\n", foo.i, foo.f);
}
```

Would be transpiled to the following Rust code.

```rust
use libc::printf;

#[derive(Clone, Copy)]
pub struct Foo {
    pub i: i32,
    pub f: f32,
}

pub unsafe fn print_foo(foo: *const Foo) {
    printf!(c"(%d, %f)\n".as_ptr(), (*foo).i, (*foo).f);
}
```

