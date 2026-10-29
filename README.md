# DirectStorage for Rust

[![Documentation](https://docs.rs/direct-storage/badge.svg)](https://docs.rs/direct-storage/)
[![Crates.io](https://img.shields.io/crates/v/direct-storage.svg)](https://crates.io/crates/direct-storage)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE.APACHE)

This crates implements [windows-rs](https://github.com/microsoft/windows-rs)
compatible bindings for the DirectStorage API, built on `windows-core` and
sharing its D3D12 types with [`d3d12-agility`](https://github.com/Traverse-Research/d3d12-agility-rs).

We try to provide the same abstraction level and coding style as windows-rs.

## Requirements

Because of licensing issues we can't provide the shared libraries that are
used by DirectStorage. You need to download them yourself and put them
in the working directory of your project, or in places windows is
searching for the library [as documented](https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-loadlibraryw#remarks):

 1. Download the correct version of DirectStorage from [nuget.org](https://devblogs.microsoft.com/directx/directstorage-api-downloads/).
 2. Extract the package file (`.nupkg` files are just `.zip` files).
 3. Copy the ".dll" files from the folder for you architecture
    (under `\native\bin`).
 4. Copy the ".lib" file from the folder for your architecture
    (under `\native\lib`)
 5. Place the `dstorage.dll`, `dstoragecore.dll` and `dstorage.lib` files
    into the working directory of your project.

## Version

This crate currently targets DirectStorage version 1.3. How long we will
support older versions is not clear yet, but we may support older version
with feature toggles if the need arises.

## Examples

We ported some examples from the [DirectStorage Repository](https://github.com/microsoft/DirectStorage)
to Rust.

They can be found in the examples folder and are licensed under [MIT](examples/LICENSE.MIT).

## License

This crate is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE.APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE.MIT) or http://opensource.org/licenses/MIT)

at your option.

Please be aware that DirectStorage itself is licensed under a proprietary
license by Microsoft, which you can find alongside the binary distribution
of the shared library.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

## Disclaimer

This is not an official Microsoft product (experimental or otherwise).
This crate is not endorsed or supported by Microsoft in any way.
