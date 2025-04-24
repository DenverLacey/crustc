RUSTC = rustc --edition=2021
RUSTFLAGS = -O -C codegen-units=1 -C lto=true
CRUSTFLAGS = -g -C opt-level=z -C link-args=-lc -C panic="abort"

DEPS = --extern=libc=libs/crust/libc.rlib
DEPS += --extern=ariadne=libs/ariadne/target/release/libariadne.rlib -L libs/ariadne/target/release/deps
DEPS += --extern=syn=libs/syn-2.0.100/target/release/libsyn.rlib -L libs/syn-2.0.100/target/release/deps

crustc: src/main.rs
	$(RUSTC) $(CRUSTFLAGS) $(DEPS) -o bin/debug/crustc src/main.rs

