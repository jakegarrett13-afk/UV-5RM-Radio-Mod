#![no_std]
#![no_main]

use bf5rh_abi::{Api, AppEvent, AppResult};

mod logic;

static mut API_PTR: *const Api = core::ptr::null();

#[no_mangle]
pub extern "C" fn app_entry(api: &Api, ev: AppEvent) -> AppResult {
    unsafe {
        API_PTR = api as *const Api;
    }
    logic::entry(api, ev)
}

// Byte-at-a-time memory routines. The library's fast ones take about 1.3 KB, a sixth
// of the app's 8 KB arena, and nothing here copies more than a few hundred bytes.
// Volatile accesses stop the compiler turning these loops back into calls to themselves.
mod mem {
    // Every entry point calls only `copy` or `fill`. Calling `memset` or `memcpy` by name
    // here would be read as the built-in again and compiled into a call back to
    // `__aeabi_memclr` and friends, which then call themselves forever.
    use core::ffi::c_void;
    use core::ptr::{read_volatile, write_volatile};

    #[inline(never)]
    unsafe fn copy(d: *mut u8, s: *const u8, n: usize) {
        let mut i = 0;
        while i < n {
            write_volatile(d.add(i), read_volatile(s.add(i)));
            i += 1;
        }
    }

    #[inline(never)]
    unsafe fn fill(d: *mut u8, c: u8, n: usize) {
        let mut i = 0;
        while i < n {
            write_volatile(d.add(i), c);
            i += 1;
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
        copy(dst as *mut u8, src as *const u8, n);
        dst
    }

    #[no_mangle]
    pub unsafe extern "C" fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void {
        fill(dst as *mut u8, c as u8, n);
        dst
    }

    #[no_mangle]
    pub unsafe extern "C" fn __aeabi_memcpy(dst: *mut c_void, src: *const c_void, n: usize) {
        copy(dst as *mut u8, src as *const u8, n);
    }

    #[no_mangle]
    pub unsafe extern "C" fn __aeabi_memcpy4(dst: *mut c_void, src: *const c_void, n: usize) {
        copy(dst as *mut u8, src as *const u8, n);
    }

    #[no_mangle]
    pub unsafe extern "C" fn __aeabi_memset(dst: *mut c_void, n: usize, c: i32) {
        fill(dst as *mut u8, c as u8, n);
    }

    #[no_mangle]
    pub unsafe extern "C" fn __aeabi_memset4(dst: *mut c_void, n: usize, c: i32) {
        fill(dst as *mut u8, c as u8, n);
    }

    #[no_mangle]
    pub unsafe extern "C" fn __aeabi_memclr(dst: *mut c_void, n: usize) {
        fill(dst as *mut u8, 0, n);
    }

    #[no_mangle]
    pub unsafe extern "C" fn __aeabi_memclr4(dst: *mut c_void, n: usize) {
        fill(dst as *mut u8, 0, n);
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        let ptr = API_PTR;
        if !ptr.is_null() {
            ((*ptr).app_fault)(0);
        }
    }
    loop {}
}
