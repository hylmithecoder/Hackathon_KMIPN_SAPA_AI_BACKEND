//! Compatibility shim for glibc fortify symbols when statically linking musl on NixOS/Linux host.

#[cfg(all(target_os = "linux", target_env = "musl"))]
mod musl_stubs {
    use std::ffi::{c_int, c_void};

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn __memcpy_chk(
        dest: *mut c_void,
        src: *const c_void,
        len: usize,
        _destlen: usize,
    ) -> *mut c_void {
        unsafe {
            std::ptr::copy_nonoverlapping(src as *const u8, dest as *mut u8, len);
        }
        dest
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn __memmove_chk(
        dest: *mut c_void,
        src: *const c_void,
        len: usize,
        _destlen: usize,
    ) -> *mut c_void {
        unsafe {
            std::ptr::copy(src as *const u8, dest as *mut u8, len);
        }
        dest
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn __memset_chk(
        dest: *mut c_void,
        c: c_int,
        len: usize,
        _destlen: usize,
    ) -> *mut c_void {
        unsafe {
            std::ptr::write_bytes(dest as *mut u8, c as u8, len);
        }
        dest
    }
}

