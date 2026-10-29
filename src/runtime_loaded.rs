//! Functions in this module are loaded dynamically at runtime, instead of linked dynamically
//! against `dstorage.lib` at compile-time.

use std::{ffi::c_void, sync::OnceLock};

use libloading::Library;
use windows_core::{GUID, HRESULT, Interface, Result};

use crate::{DSTORAGE_COMPRESSION_FORMAT, DSTORAGE_CONFIGURATION};

static DIRECT_STORAGE_LIB: OnceLock<Libraries> = OnceLock::new();

struct Libraries {
    ds: Library,
    _core: Library,
}

/// Runtime-loaded variant of [`crate::DStorageCreateCompressionCodec()`] from `dstorage.dll`
///
/// # Safety
/// Loads a raw pointer from `dstorage.dll` and casts it to a function to call.
pub unsafe fn DStorageCreateCompressionCodec<T: Interface>(
    format: DSTORAGE_COMPRESSION_FORMAT,
    numThreads: u32,
) -> Result<T> {
    let f = DIRECT_STORAGE_LIB
        .get_or_init(|| unsafe { load_libraries() })
        .ds
        .get::<unsafe extern "system" fn(
            format: DSTORAGE_COMPRESSION_FORMAT,
            numThreads: u32,
            riid: *const GUID,
            ppv: *mut *mut c_void,
        ) -> HRESULT>(b"DStorageCreateCompressionCodec\0")
        .expect("Can't load function`DStorageCreateCompressionCodec`");

    let mut result__ = ::std::ptr::null_mut();
    f(format, numThreads, &T::IID, &mut result__).map(|| T::from_raw(result__))
}

/// Runtime-loaded variant of [`crate::DStorageSetConfiguration()`] from `dstorage.dll`
///
/// # Safety
/// Loads a raw pointer from `dstorage.dll` and casts it to a function to call.
pub unsafe fn DStorageSetConfiguration(configuration: &DSTORAGE_CONFIGURATION) -> Result<()> {
    let f = DIRECT_STORAGE_LIB
        .get_or_init(|| unsafe { load_libraries() })
        .ds
        .get::<unsafe extern "system" fn(configuration: *const DSTORAGE_CONFIGURATION) -> HRESULT>(
            b"DStorageSetConfiguration\0",
        )
        .expect("Can't load function`DStorageSetConfiguration`");

    f(configuration as *const DSTORAGE_CONFIGURATION).ok()
}

/// Runtime-loaded variant of [`crate::DStorageGetFactory()`] from `dstorage.dll`
///
/// # Safety
/// Loads a raw pointer from `dstorage.dll` and casts it to a function to call.
pub unsafe fn DStorageGetFactory<T: Interface>() -> Result<T> {
    let f = DIRECT_STORAGE_LIB
        .get_or_init(|| unsafe { load_libraries() })
        .ds
        .get::<unsafe extern "system" fn(riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT>(
            b"DStorageGetFactory\0",
        )
        .expect("Can't load function`DStorageGetFactory`");

    let mut result__ = ::std::ptr::null_mut();
    f(&T::IID, &mut result__).map(|| T::from_raw(result__))
}

unsafe fn load_libraries() -> Libraries {
    let ds = Library::new("dstorage.dll").expect("Can't load `dstorage.dll`");
    let core = Library::new("dstoragecore.dll").expect("Can't load `dstoragecore.dll`");
    Libraries { ds, _core: core }
}
