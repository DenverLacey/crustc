#![no_std]
pub unsafe fn fac(n: i32) {
    if n <= 1 {
        return n;
    }
    return fac(n - 1) + fac(n - 2);
}
pub unsafe fn bar(n: i32) -> i32 {
    let mut k = 0;
    for i in 1..=n {
        k = k + i;
    }
    k
}
