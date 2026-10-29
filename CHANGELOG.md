# Change Log

## Unreleased

- Targets [windows-rs `0.100`](https://github.com/microsoft/windows-rs/releases/tag/74), depending only on `windows-core` instead of `windows`
- D3D12 types (`ID3D12Device`, `ID3D12Resource`, `ID3D12Fence`, `D3D12_BOX`, ...) and `HANDLE` now come from [`d3d12-agility`](https://github.com/Traverse-Research/d3d12-agility-rs) instead of `windows::Win32`
- Bindings are generated straight from the DirectStorage headers with `windows-clang`, replacing the `win32metadata`-based `.winmd` pipeline.  This changes the API:
  - All items live at the crate root instead of `Microsoft::Direct3D::DirectStorage`
  - Enums and flags are plain integer aliases with constants instead of newtypes
  - Methods returning a bare `HRESULT` (`GetFileInformation()`, `SetStagingBufferSize()`, `CompressBuffer()`, ...) no longer return `Result`; call `.ok()`
  - `RetrieveErrorRecord()` and `Query()` take an out-pointer instead of returning the value
  - `Name` fields are `*const i8` instead of `PCSTR`
- Increased MSRV of Rust to 1.95

## v0.7.1 (2025-09-09)

- Targets [windows-rs `0.61` - `0.62`](https://github.com/microsoft/windows-rs/releases/tag/69)

## v0.7.0 (2025-07-02)

- Targets [windows-rs `0.61`](https://github.com/microsoft/windows-rs/releases/tag/63)
- Targets [DirectStorage v1.3.0](https://devblogs.microsoft.com/directx/directstorage-1-3-is-now-available)

## v0.6.0 (2025-07-01)

- Targets [windows-rs `0.60`](https://github.com/microsoft/windows-rs/releases/tag/0.62.0)
- Increased MSV of Rust to 1.74

## v0.5.1 (2025-01-15)

- Allow using windows-targets 0.52 next to 0.53

## v0.5.0 (2025-01-14)

- Targets windows-rs v0.57-0.59
- Increased MSV of Rust to 1.70
- Removed once_cell dependency

## v0.4.1 (2024-07-17)

- Targets windows-rs v0.56-0.58

## v0.4.0 (2024-06-25)

- Targets windows-rs v0.56-0.57

## v0.3.0 (2024-02-19)

- Targets DirectStorage v1.2.1
- Targets windows-rs v0.52

## v0.2.0 (2023-01-19)

- Targets windows-rs v0.44
- Use libloading

## v0.1.0 (2023-01-17)

- Targets DirectStorage v1.1
- Targets windows-rs v0.43
