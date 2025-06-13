#![no_std]
pub struct Foo {
    pub i: !,
    pub k: !,
}
pub(self) struct Bar {
    pub x: !,
    pub y: !,
}
pub(super) struct Baz {
    pub u: !,
    pub v: !,
}
pub(crate) struct Quox {
    pub a: !,
    pub b: !,
}
pub const FOO: ! = ();
