//! Generates the DirectStorage bindings from the `Microsoft.Direct3D.DirectStorage` headers.
//!
//! The pipeline mirrors windows-rs' own `tool_win32` and the `d3d12-agility` generator: headers
//! are parsed with the same pinned libclang, SAL shim and Windows SDK version that `windows-rs`
//! 0.100 uses, written out as reviewable RDL, compiled to a `.winmd`, and projected to Rust with
//! `windows-bindgen`.
//!
//! The scrape is additive on top of the Win32 metadata shipped in `windows-default`: everything
//! Win32 already defines (`HANDLE`, `ID3D12Device`, ...) is skipped, so only the DirectStorage
//! surface ends up in the RDL.
//!
//! Run from the repository root: `cargo run -p api_gen`.

use std::path::{Path, PathBuf};
use windows_clang::*;

/// Pinned `Microsoft.Direct3D.DirectStorage` NuGet package version.
const DIRECT_STORAGE_VERSION: &str = "1.3.0";

/// Pinned Windows SDK, matching the version windows-rs 0.100 was generated from (`SDK_VERSION` in
/// windows-rs' `crates/tools/win32/src/main.rs`).
const SDK_VERSION: &str = "10.0.28000.2270";

/// Scratch directory for intermediate artifacts.
const OUT_DIR: &str = "target/api_gen";

/// Committed per-header RDL snapshot, reviewable in pull requests.
const RDL_DIR: &str = "metadata";

/// The compiled metadata the bindings are generated from.
const WINMD: &str = "target/DirectStorage.winmd";

/// The Win32 metadata from `windows-default`, written out as the scrape exclusion reference.
const REFERENCE_WINMD: &str = "target/Windows.Win32.winmd";

/// Generated bindings.
const BINDINGS: &str = "src/bindings.rs";

/// SAL capture shim, copied from windows-rs' `crates/tools/win32/src/sal.h`.
const SAL_SHIM: &str = "api_gen/src/sal.h";

/// Same root namespace as windows-rs, so `windows-bindgen` maps `IUnknown`, `GUID`, `HRESULT`
/// and friends onto `windows-core`.
const ROOT: &str = "Windows.Win32";

/// Same prelude as `tool_win32`.
const PRELUDE: &str = "#define SECURITY_WIN32\n#define WIN32_NO_STATUS\n#include <winsock2.h>\n#include <windows.h>\n#undef WIN32_NO_STATUS\n#include <ntstatus.h>";

/// `windows-clang` 0.100 does not canonicalize the fixed underlying type of an enum, so the
/// typedef in `enum DSTORAGE_PRIORITY : INT8` is not recognized and falls back to `i32`, breaking
/// the layout of every struct containing such an enum. Spell the typedefs used as enum bases in
/// the DirectStorage headers as builtin types; the prelude has already declared the real typedefs.
const ENUM_BASE_SHIM: &str = "\n#define INT8 signed char\n#define UINT8 unsigned char\n#define INT32 int\n#define UINT32 unsigned int\n#define UINT64 unsigned long long";

/// Headers whose declarations are emitted.
const HEADERS: &[&str] = &["dstorage.h", "dstorageerr.h"];

/// Win32 types referenced by DirectStorage that are not generated, but imported from
/// `d3d12-agility` so that they are interchangeable with the rest of the D3D12 ecosystem.
const FROM_D3D12_AGILITY: &[&str] = &[
    "D3D12_BOX",
    "D3D12_TILED_RESOURCE_COORDINATE",
    "D3D12_TILE_REGION_SIZE",
    "HANDLE",
    "ID3D12Device",
    "ID3D12DeviceChild",
    "ID3D12Fence",
    "ID3D12Object",
    "ID3D12Pageable",
    "ID3D12Resource",
];

fn main() {
    let time = std::time::Instant::now();

    ensure_libclang();
    assert_libclang_version();

    std::fs::create_dir_all(OUT_DIR).unwrap();
    std::fs::write(REFERENCE_WINMD, windows_default::WIN32).unwrap();

    let direct_storage =
        nuget_package("microsoft.direct3d.directstorage", DIRECT_STORAGE_VERSION).join("native");
    // D3D12 deliberately comes from the pinned Windows SDK rather than the Agility SDK: its
    // declarations then match the reference winmd exactly and are all skipped by the scrape. The
    // few D3D12 types DirectStorage refers to are taken from `d3d12-agility` at compile time.
    let mut include_dirs = vec![slashed(&direct_storage.join("include"))];
    include_dirs.extend(sdk_include_dirs());
    let include_args: Vec<String> = include_dirs
        .into_iter()
        .flat_map(|dir| ["-isystem".to_string(), dir])
        .collect();

    let mut source = String::from(PRELUDE);
    source.push_str(ENUM_BASE_SHIM);
    for header in HEADERS {
        source.push_str(&format!("\n#include <{header}>"));
    }

    let mut clang = clang();
    clang
        .args(["-x", "c++", "-ferror-limit=0"])
        .args(["-include", SAL_SHIM])
        .args(include_args)
        .library("dstorage.dll")
        // The reachability sweep is only enabled when at least one directory scope is set. A
        // scope that matches no SDK directory leaves `HEADERS` as the only roots.
        .scope("direct-storage-no-directory-scope")
        .scope_headers(HEADERS.iter().copied());
    clang.input_text(&source);
    let import_lib = slashed(&direct_storage.join("lib").join("x64").join("dstorage.lib"));
    clang
        .import_library(&import_lib)
        .unwrap_or_else(|e| panic!("failed to read import library `{import_lib}`: {e}"));

    let summary = clang.scrape(&ScrapePlan {
        root: ROOT.to_string(),
        rdl_dir: RDL_DIR.into(),
        out_dir: OUT_DIR.into(),
        winmd: WINMD.into(),
        archs: Arch::canonical_plus(&[], |name| Arch::known(name).unwrap()),
        reference_winmds: vec![REFERENCE_WINMD.into()],
        resolution_winmds: Vec::new(),
        seed: None,
        parallel: false,
    });
    print!("{summary}");

    // Select everything DirectStorage defines; the Win32 metadata only resolves what it refers to.
    let index = windows_metadata::reader::Index::read(WINMD).expect("failed to read winmd");
    let mut filters: Vec<String> = index
        .iter_items()
        .map(|(namespace, name, _)| format!("{namespace}.{name}"))
        .collect();
    filters.sort();
    filters.dedup();

    let mut args = vec![
        "--in",
        REFERENCE_WINMD,
        "--in",
        WINMD,
        "--out",
        BINDINGS,
        "--flat",
    ];
    args.push("--filter");
    args.extend(filters.iter().map(String::as_str));
    windows_bindgen::bindgen(args);

    strip_d3d12_agility_types(BINDINGS);

    println!("Finished in {:.2}s", time.elapsed().as_secs_f32());
}

/// `windows-bindgen` copies every referenced type into the bindings, and cannot refer to another
/// crate instead. Remove the items (including their vtables, impls and macro invocations) that
/// are imported from `d3d12-agility` instead, leaving the rest of the formatted output intact.
fn strip_d3d12_agility_types(path: &str) {
    use syn::spanned::Spanned;

    let source = std::fs::read_to_string(path).unwrap();
    let file = syn::parse_file(&source).unwrap();

    let is_stripped = |ident: &syn::Ident| {
        let ident = ident.to_string();
        let ident = ident.strip_suffix("_Vtbl").unwrap_or(&ident);
        FROM_D3D12_AGILITY.contains(&ident)
    };

    // 1-based, inclusive line ranges of the items to remove.
    let mut removed = Vec::new();
    for item in &file.items {
        let stripped = match item {
            syn::Item::Struct(item) => is_stripped(&item.ident),
            syn::Item::Union(item) => is_stripped(&item.ident),
            syn::Item::Type(item) => is_stripped(&item.ident),
            syn::Item::Impl(item) => match &*item.self_ty {
                syn::Type::Path(ty) => is_stripped(&ty.path.segments.last().unwrap().ident),
                _ => false,
            },
            // `define_interface!(ID3D12Device, ...)` and `interface_hierarchy!(ID3D12Device, ...)`
            syn::Item::Macro(item) => match item.mac.tokens.clone().into_iter().next() {
                Some(proc_macro2::TokenTree::Ident(ident)) => is_stripped(&ident),
                _ => false,
            },
            _ => false,
        };
        if stripped {
            removed.push(item.span().start().line..=item.span().end().line);
        }
    }

    let mut output = String::with_capacity(source.len());
    output.push_str(
        "// Bindings generated by `api_gen` with `windows-clang` and `windows-bindgen` 0.100\n\n",
    );
    output.push_str("#![allow(\n    non_snake_case,\n    non_upper_case_globals,\n    non_camel_case_types,\n    dead_code,\n    clippy::all\n)]\n\n");
    output.push_str("use d3d12_agility::*;\n\n");
    for (line, text) in (1..).zip(source.lines()) {
        if !removed.iter().any(|range| range.contains(&line)) {
            output.push_str(text);
            output.push('\n');
        }
    }
    std::fs::write(path, output).unwrap();
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// `10.0.28000.2270` -> `10.0.28000.0`, the directory name the SDK packages use.
fn marketing_dir(version: &str) -> String {
    let mut parts = version.split('.');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(major), Some(minor), Some(build)) => format!("{major}.{minor}.{build}.0"),
        _ => panic!("`{version}` is not a `major.minor.build[.revision]` version"),
    }
}

fn sdk_include_dirs() -> Vec<String> {
    let base: PathBuf = nuget_package("microsoft.windows.sdk.cpp", SDK_VERSION)
        .join("c")
        .join("Include")
        .join(marketing_dir(SDK_VERSION));
    ["ucrt", "um", "shared", "winrt", "cppwinrt"]
        .iter()
        .map(|seg| slashed(&base.join(seg)))
        .collect()
}
