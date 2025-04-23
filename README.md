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
1. All user structs #\[derive(Clone, Copy)].
1. Everything is pub by default.

> "*These rules may change. The goal is to make programming in Rust fun.*" - Tsoding


