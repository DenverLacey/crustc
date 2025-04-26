RUSTC = rustc --edition=2021
LIBFLAGS = -O -C codegen-units=1 -C lto=true --crate-type=lib

all: libc.rlib

libc.rlib: libc.rs
	$(RUSTC) $(LIBFLAGS) -o libc.rlib libc.rs

