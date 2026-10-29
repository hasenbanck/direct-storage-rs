//! Rust bindings for [DirectStorage](https://github.com/microsoft/DirectStorage).
//!
//! We try to provide the same abstraction level and coding style as [windows-rs](https://github.com/microsoft/windows-rs).
//!
//! For more documentation, please have a look at the header files of the official
//! distribution. We can't simply copy those because of licensing issues.
//!
//! This crate will panic if it can't find the shared libraries of DirectStorage.
//! Please refer to the README.md on how to install them.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![warn(unused_qualifications)]

use std::mem::{transmute_copy, ManuallyDrop};

use windows_core::Interface;

mod bindings;
#[cfg(feature = "loaded")]
pub mod runtime_loaded;
pub use bindings::Microsoft::Direct3D::DirectStorage::*;

/// Create a temporary "owned" copy inside a [`ManuallyDrop`] without increasing the refcount or
/// moving away the source variable.
///
/// This is a common pattern when needing to pass interface pointers ("borrows") into Windows
/// structs.  Moving/cloning ownership is impossible/inconvenient because:
///
/// - The caller does _not_ assume ownership (and decrement the refcount at a later time);
/// - Unnecessarily increasing and decrementing the refcount;
/// - [`Drop`] destructors cannot run inside `union` structures (when the created structure is
///   implicitly dropped after a calll).
///
/// See also <https://github.com/microsoft/windows-rs/pull/2361#discussion_r1150799401> and
/// <https://github.com/microsoft/windows-rs/issues/2386>.
///
/// # Safety
/// Performs a [`transmute_copy()`] on a refcounted [`Interface`] type.  The returned [`ManuallyDrop`] should _not_ be
/// dropped.
pub unsafe fn readonly_copy<Src: Interface, Dst>(src: &Src) -> ManuallyDrop<Option<Dst>> {
    unsafe { transmute_copy(src) }
}

/// Since DirectStorage is compiled with MSVC, we have to use it's rules for C bitfields.
/// MSVC will only pack fields of the same type in the same backing field.
///
/// ```cpp
/// struct DSTORAGE_REQUEST_OPTIONS {
///      DSTORAGE_COMPRESSION_FORMAT CompressionFormat : 8;     // uint8_t  -> saved into A
///      DSTORAGE_REQUEST_SOURCE_TYPE SourceType : 1;           // uint64_t -> packed together into B
///      DSTORAGE_REQUEST_DESTINATION_TYPE DestinationType : 7; // uint64_t -> packed together into B
///      UINT64 Reserved : 48;                                  // uint64_t -> packed together into B
/// };
///
/// // Resulting layout:
/// struct Storage {
///      uint8_t A;
///      uint8_t PADDING[7];
///      uint64_t B;
/// }
/// ```
impl DSTORAGE_REQUEST_OPTIONS {
    pub fn CompressionFormat(&self) -> DSTORAGE_COMPRESSION_FORMAT {
        DSTORAGE_COMPRESSION_FORMAT(self._bitfield1)
    }

    pub fn set_CompressionFormat(&mut self, value: DSTORAGE_COMPRESSION_FORMAT) {
        self._bitfield1 = value.0;
    }

    pub fn GameAssetConditioningLibraryShuffleTransformType(
        &self,
    ) -> DSTORAGE_GACL_SHUFFLE_TRANSFORM_TYPE {
        DSTORAGE_GACL_SHUFFLE_TRANSFORM_TYPE(self._bitfield2)
    }

    pub fn set_GameAssetConditioningLibraryShuffleTransformType(
        &mut self,
        value: DSTORAGE_GACL_SHUFFLE_TRANSFORM_TYPE,
    ) {
        self._bitfield2 = value.0;
    }

    pub fn SourceType(&self) -> DSTORAGE_REQUEST_SOURCE_TYPE {
        let size = u64::BITS;
        DSTORAGE_REQUEST_SOURCE_TYPE(self._bitfield3 << (size - 1) >> (size - 1))
    }

    pub fn set_SourceType(&mut self, value: DSTORAGE_REQUEST_SOURCE_TYPE) {
        let mask = ((1 << 1) - 1) << 0;
        self._bitfield3 &= !mask;
        self._bitfield3 |= value.0 & mask;
    }

    pub fn DestinationType(&self) -> DSTORAGE_REQUEST_DESTINATION_TYPE {
        let size = u64::BITS;
        DSTORAGE_REQUEST_DESTINATION_TYPE(self._bitfield3 << (size - 8) >> (size - 8 + 1))
    }

    pub fn set_DestinationType(&mut self, value: DSTORAGE_REQUEST_DESTINATION_TYPE) {
        let mask = ((1 << (8 - 1)) - 1) << 1;
        self._bitfield3 &= !mask;
        self._bitfield3 |= (value.0 << 1) & mask;
    }
}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, size_of};

    use super::*;

    #[cfg(target_pointer_width = "32")]
    #[test]
    fn test_msvc_compat_32bit() {
        assert_eq!(size_of::<DSTORAGE_PRIORITY>(), 1);
        assert_eq!(align_of::<DSTORAGE_PRIORITY>(), 1);

        assert_eq!(size_of::<DSTORAGE_REQUEST_SOURCE_TYPE>(), 8);
        assert_eq!(align_of::<DSTORAGE_REQUEST_SOURCE_TYPE>(), 8);

        assert_eq!(size_of::<DSTORAGE_REQUEST_DESTINATION_TYPE>(), 8);
        assert_eq!(align_of::<DSTORAGE_REQUEST_DESTINATION_TYPE>(), 8);

        assert_eq!(size_of::<DSTORAGE_QUEUE_DESC>(), 24);
        assert_eq!(align_of::<DSTORAGE_QUEUE_DESC>(), 8);

        assert_eq!(size_of::<DSTORAGE_QUEUE_INFO>(), 32);
        assert_eq!(align_of::<DSTORAGE_QUEUE_INFO>(), 8);

        assert_eq!(size_of::<DSTORAGE_COMPRESSION_FORMAT>(), 1);
        assert_eq!(align_of::<DSTORAGE_COMPRESSION_FORMAT>(), 1);

        assert_eq!(size_of::<DSTORAGE_REQUEST_OPTIONS>(), 16);
        assert_eq!(align_of::<DSTORAGE_REQUEST_OPTIONS>(), 8);

        assert_eq!(size_of::<DSTORAGE_DEBUG>(), 4);
        assert_eq!(align_of::<DSTORAGE_DEBUG>(), 4);

        assert_eq!(size_of::<DSTORAGE_SOURCE_FILE>(), 24);
        assert_eq!(align_of::<DSTORAGE_SOURCE_FILE>(), 8);

        assert_eq!(size_of::<DSTORAGE_SOURCE_MEMORY>(), 8);
        assert_eq!(align_of::<DSTORAGE_SOURCE_MEMORY>(), 4);

        assert_eq!(size_of::<DSTORAGE_DESTINATION_MEMORY>(), 8);
        assert_eq!(align_of::<DSTORAGE_DESTINATION_MEMORY>(), 4);

        assert_eq!(size_of::<DSTORAGE_DESTINATION_BUFFER>(), 24);
        assert_eq!(align_of::<DSTORAGE_DESTINATION_BUFFER>(), 8);

        assert_eq!(size_of::<DSTORAGE_DESTINATION_TEXTURE_REGION>(), 32);
        assert_eq!(align_of::<DSTORAGE_DESTINATION_TEXTURE_REGION>(), 4);

        assert_eq!(size_of::<DSTORAGE_DESTINATION_MULTIPLE_SUBRESOURCES>(), 8);
        assert_eq!(align_of::<DSTORAGE_DESTINATION_MULTIPLE_SUBRESOURCES>(), 4);

        assert_eq!(size_of::<DSTORAGE_DESTINATION_TILES>(), 36);
        assert_eq!(align_of::<DSTORAGE_DESTINATION_TILES>(), 4);

        assert_eq!(size_of::<DSTORAGE_SOURCE>(), 24);
        assert_eq!(align_of::<DSTORAGE_SOURCE>(), 8);

        assert_eq!(size_of::<DSTORAGE_DESTINATION>(), 40);
        assert_eq!(align_of::<DSTORAGE_DESTINATION>(), 8);

        assert_eq!(size_of::<DSTORAGE_REQUEST>(), 104);
        assert_eq!(align_of::<DSTORAGE_REQUEST>(), 8);

        assert_eq!(size_of::<DSTORAGE_COMMAND_TYPE>(), 4);
        assert_eq!(align_of::<DSTORAGE_COMMAND_TYPE>(), 4);

        assert_eq!(size_of::<DSTORAGE_ERROR_PARAMETERS_REQUEST>(), 688);
        assert_eq!(align_of::<DSTORAGE_ERROR_PARAMETERS_REQUEST>(), 8);

        assert_eq!(size_of::<DSTORAGE_ERROR_PARAMETERS_STATUS>(), 8);
        assert_eq!(align_of::<DSTORAGE_ERROR_PARAMETERS_STATUS>(), 4);

        assert_eq!(size_of::<DSTORAGE_ERROR_PARAMETERS_SIGNAL>(), 16);
        assert_eq!(align_of::<DSTORAGE_ERROR_PARAMETERS_SIGNAL>(), 8);

        assert_eq!(size_of::<DSTORAGE_ERROR_PARAMETERS_EVENT>(), 4);
        assert_eq!(align_of::<DSTORAGE_ERROR_PARAMETERS_EVENT>(), 4);

        assert_eq!(size_of::<DSTORAGE_ERROR_FIRST_FAILURE>(), 696);
        assert_eq!(align_of::<DSTORAGE_ERROR_FIRST_FAILURE>(), 8);

        assert_eq!(size_of::<DSTORAGE_ERROR_RECORD>(), 704);
        assert_eq!(align_of::<DSTORAGE_ERROR_RECORD>(), 8);

        assert_eq!(size_of::<DSTORAGE_STAGING_BUFFER_SIZE>(), 4);
        assert_eq!(align_of::<DSTORAGE_STAGING_BUFFER_SIZE>(), 4);

        assert_eq!(size_of::<DSTORAGE_GET_REQUEST_FLAGS>(), 4);
        assert_eq!(align_of::<DSTORAGE_GET_REQUEST_FLAGS>(), 4);

        assert_eq!(size_of::<DSTORAGE_CUSTOM_DECOMPRESSION_FLAGS>(), 4);
        assert_eq!(align_of::<DSTORAGE_CUSTOM_DECOMPRESSION_FLAGS>(), 4);

        assert_eq!(size_of::<DSTORAGE_CUSTOM_DECOMPRESSION_REQUEST>(), 48);
        assert_eq!(align_of::<DSTORAGE_CUSTOM_DECOMPRESSION_REQUEST>(), 8);

        assert_eq!(size_of::<DSTORAGE_CUSTOM_DECOMPRESSION_RESULT>(), 16);
        assert_eq!(align_of::<DSTORAGE_CUSTOM_DECOMPRESSION_RESULT>(), 8);

        assert_eq!(size_of::<DSTORAGE_CONFIGURATION>(), 28);
        assert_eq!(align_of::<DSTORAGE_CONFIGURATION>(), 4);

        assert_eq!(size_of::<DSTORAGE_COMPRESSION>(), 4);
        assert_eq!(align_of::<DSTORAGE_COMPRESSION>(), 4);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn test_msvc_compat_64bit() {
        assert_eq!(size_of::<DSTORAGE_PRIORITY>(), 1);
        assert_eq!(align_of::<DSTORAGE_PRIORITY>(), 1);

        assert_eq!(size_of::<DSTORAGE_REQUEST_SOURCE_TYPE>(), 8);
        assert_eq!(align_of::<DSTORAGE_REQUEST_SOURCE_TYPE>(), 8);

        assert_eq!(size_of::<DSTORAGE_REQUEST_DESTINATION_TYPE>(), 8);
        assert_eq!(align_of::<DSTORAGE_REQUEST_DESTINATION_TYPE>(), 8);

        assert_eq!(size_of::<DSTORAGE_QUEUE_DESC>(), 32);
        assert_eq!(align_of::<DSTORAGE_QUEUE_DESC>(), 8);

        assert_eq!(size_of::<DSTORAGE_QUEUE_INFO>(), 40);
        assert_eq!(align_of::<DSTORAGE_QUEUE_INFO>(), 8);

        assert_eq!(size_of::<DSTORAGE_COMPRESSION_FORMAT>(), 1);
        assert_eq!(align_of::<DSTORAGE_COMPRESSION_FORMAT>(), 1);

        assert_eq!(size_of::<DSTORAGE_REQUEST_OPTIONS>(), 16);
        assert_eq!(align_of::<DSTORAGE_REQUEST_OPTIONS>(), 8);

        assert_eq!(size_of::<DSTORAGE_DEBUG>(), 4);
        assert_eq!(align_of::<DSTORAGE_DEBUG>(), 4);

        assert_eq!(size_of::<DSTORAGE_SOURCE_FILE>(), 24);
        assert_eq!(align_of::<DSTORAGE_SOURCE_FILE>(), 8);

        assert_eq!(size_of::<DSTORAGE_SOURCE_MEMORY>(), 16);
        assert_eq!(align_of::<DSTORAGE_SOURCE_MEMORY>(), 8);

        assert_eq!(size_of::<DSTORAGE_DESTINATION_MEMORY>(), 16);
        assert_eq!(align_of::<DSTORAGE_DESTINATION_MEMORY>(), 8);

        assert_eq!(size_of::<DSTORAGE_DESTINATION_BUFFER>(), 24);
        assert_eq!(align_of::<DSTORAGE_DESTINATION_BUFFER>(), 8);

        assert_eq!(size_of::<DSTORAGE_DESTINATION_TEXTURE_REGION>(), 40);
        assert_eq!(align_of::<DSTORAGE_DESTINATION_TEXTURE_REGION>(), 8);

        assert_eq!(size_of::<DSTORAGE_DESTINATION_MULTIPLE_SUBRESOURCES>(), 16);
        assert_eq!(align_of::<DSTORAGE_DESTINATION_MULTIPLE_SUBRESOURCES>(), 8);

        assert_eq!(size_of::<DSTORAGE_DESTINATION_TILES>(), 40);
        assert_eq!(align_of::<DSTORAGE_DESTINATION_TILES>(), 8);

        assert_eq!(size_of::<DSTORAGE_SOURCE>(), 24);
        assert_eq!(align_of::<DSTORAGE_SOURCE>(), 8);

        assert_eq!(size_of::<DSTORAGE_DESTINATION>(), 40);
        assert_eq!(align_of::<DSTORAGE_DESTINATION>(), 8);

        assert_eq!(size_of::<DSTORAGE_REQUEST>(), 104);
        assert_eq!(align_of::<DSTORAGE_REQUEST>(), 8);

        assert_eq!(size_of::<DSTORAGE_COMMAND_TYPE>(), 4);
        assert_eq!(align_of::<DSTORAGE_COMMAND_TYPE>(), 4);

        assert_eq!(size_of::<DSTORAGE_ERROR_PARAMETERS_REQUEST>(), 688);
        assert_eq!(align_of::<DSTORAGE_ERROR_PARAMETERS_REQUEST>(), 8);

        assert_eq!(size_of::<DSTORAGE_ERROR_PARAMETERS_STATUS>(), 16);
        assert_eq!(align_of::<DSTORAGE_ERROR_PARAMETERS_STATUS>(), 8);

        assert_eq!(size_of::<DSTORAGE_ERROR_PARAMETERS_SIGNAL>(), 16);
        assert_eq!(align_of::<DSTORAGE_ERROR_PARAMETERS_SIGNAL>(), 8);

        assert_eq!(size_of::<DSTORAGE_ERROR_PARAMETERS_EVENT>(), 8);
        assert_eq!(align_of::<DSTORAGE_ERROR_PARAMETERS_EVENT>(), 8);

        assert_eq!(size_of::<DSTORAGE_ERROR_FIRST_FAILURE>(), 696);
        assert_eq!(align_of::<DSTORAGE_ERROR_FIRST_FAILURE>(), 8);

        assert_eq!(size_of::<DSTORAGE_ERROR_RECORD>(), 704);
        assert_eq!(align_of::<DSTORAGE_ERROR_RECORD>(), 8);

        assert_eq!(size_of::<DSTORAGE_STAGING_BUFFER_SIZE>(), 4);
        assert_eq!(align_of::<DSTORAGE_STAGING_BUFFER_SIZE>(), 4);

        assert_eq!(size_of::<DSTORAGE_GET_REQUEST_FLAGS>(), 4);
        assert_eq!(align_of::<DSTORAGE_GET_REQUEST_FLAGS>(), 4);

        assert_eq!(size_of::<DSTORAGE_CUSTOM_DECOMPRESSION_FLAGS>(), 4);
        assert_eq!(align_of::<DSTORAGE_CUSTOM_DECOMPRESSION_FLAGS>(), 4);

        assert_eq!(size_of::<DSTORAGE_CUSTOM_DECOMPRESSION_REQUEST>(), 48);
        assert_eq!(align_of::<DSTORAGE_CUSTOM_DECOMPRESSION_REQUEST>(), 8);

        assert_eq!(size_of::<DSTORAGE_CUSTOM_DECOMPRESSION_RESULT>(), 16);
        assert_eq!(align_of::<DSTORAGE_CUSTOM_DECOMPRESSION_RESULT>(), 8);

        assert_eq!(size_of::<DSTORAGE_CONFIGURATION>(), 28);
        assert_eq!(align_of::<DSTORAGE_CONFIGURATION>(), 4);

        assert_eq!(size_of::<DSTORAGE_COMPRESSION>(), 4);
        assert_eq!(align_of::<DSTORAGE_COMPRESSION>(), 4);
    }

    #[test]
    fn test_bitfield() {
        let mut options = DSTORAGE_REQUEST_OPTIONS::default();
        options.set_CompressionFormat(DSTORAGE_COMPRESSION_FORMAT_GDEFLATE);
        assert_eq!(
            options.CompressionFormat(),
            DSTORAGE_COMPRESSION_FORMAT_GDEFLATE
        );

        options.set_SourceType(DSTORAGE_REQUEST_SOURCE_MEMORY);
        assert_eq!(options.SourceType(), DSTORAGE_REQUEST_SOURCE_MEMORY);

        options.set_DestinationType(DSTORAGE_REQUEST_DESTINATION_TEXTURE_REGION);
        assert_eq!(
            options.DestinationType(),
            DSTORAGE_REQUEST_DESTINATION_TEXTURE_REGION
        );

        options.set_CompressionFormat(DSTORAGE_COMPRESSION_FORMAT_NONE);
        options.set_SourceType(DSTORAGE_REQUEST_SOURCE_FILE);
        options.set_DestinationType(DSTORAGE_REQUEST_DESTINATION_MEMORY);

        assert_eq!(options, DSTORAGE_REQUEST_OPTIONS::default());
    }
}
