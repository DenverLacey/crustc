# Must Do:

* Use rustc-private stuff (We need the actual compiler it seems)

* Ban references
* Compile generated code (Edition 2021, ¿link with libc by default?)
* Convert string literal to c-string literals

# Stretch Goals:

* Implicit conversion from `*const/mut CStr` to `*const/mut c_char`
* Auto dereferencing of pointers

