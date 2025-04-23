RUSTC = rustc --edition=2021
RUSTFLAGS = -O -C codegen-units=1 -C lto=true
CRUSTFLAGS = -g -C opt-level=z -C link-args=-lc -C panic="abort"

crustc: src/main.rs
	$(RUSTC) $(CRUSTFLAGS) -o bin/debug/crustc src/main.rs

