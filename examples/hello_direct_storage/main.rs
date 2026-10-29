//
// Copyright (c) Microsoft. All rights reserved.
// This code is licensed under the MIT License (MIT).
// THIS CODE IS PROVIDED *AS IS* WITHOUT WARRANTY OF
// ANY KIND, EITHER EXPRESS OR IMPLIED, INCLUDING ANY
// IMPLIED WARRANTIES OF FITNESS FOR A PARTICULAR
// PURPOSE, MERCHANTABILITY, OR NON-INFRINGEMENT.
//
use std::{mem::ManuallyDrop, os::windows::ffi::OsStrExt, path::Path, process::exit};

use d3d12_agility::{
    D3D_FEATURE_LEVEL_12_0, D3D_SHADER_MODEL_6_0, D3D12_FEATURE_DATA_SHADER_MODEL,
    D3D12_FEATURE_SHADER_MODEL, D3D12_FENCE_FLAG_NONE, D3D12_HEAP_FLAG_NONE, D3D12_HEAP_PROPERTIES,
    D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DESC, D3D12_RESOURCE_DIMENSION_BUFFER,
    D3D12_RESOURCE_STATE_COMMON, D3D12_TEXTURE_LAYOUT_ROW_MAJOR, D3D12CreateDevice,
    DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC, HANDLE, ID3D12Device, ID3D12Fence, ID3D12Resource,
};
use direct_storage::{
    BY_HANDLE_FILE_INFORMATION, DSTORAGE_COMPRESSION_FORMAT_NONE, DSTORAGE_DESTINATION,
    DSTORAGE_DESTINATION_BUFFER, DSTORAGE_MAX_QUEUE_CAPACITY, DSTORAGE_PRIORITY_NORMAL,
    DSTORAGE_QUEUE_DESC, DSTORAGE_REQUEST, DSTORAGE_REQUEST_DESTINATION_BUFFER,
    DSTORAGE_REQUEST_OPTIONS, DSTORAGE_REQUEST_SOURCE_FILE, DSTORAGE_SOURCE, DSTORAGE_SOURCE_FILE,
    IDStorageFactory, IDStorageFile, IDStorageQueue, readonly_copy,
    runtime_loaded::DStorageGetFactory,
};
use windows_core::{BOOL, PCSTR, PCWSTR};

windows_core::link!("kernel32.dll" "system" fn CreateEventA(lpeventattributes : *const core::ffi::c_void, bmanualreset : BOOL, binitialstate : BOOL, lpname : PCSTR) -> HANDLE);
windows_core::link!("kernel32.dll" "system" fn WaitForSingleObject(hhandle : HANDLE, dwmilliseconds : u32) -> u32);
windows_core::link!("kernel32.dll" "system" fn CloseHandle(hobject : HANDLE) -> BOOL);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("No data file give as first argument.");
        exit(1);
    }

    let file_path: Vec<u16> = Path::new(&args[1])
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();

    let mut device = None::<ID3D12Device>;
    unsafe {
        D3D12CreateDevice(
            None::<&windows_core::IUnknown>,
            D3D_FEATURE_LEVEL_12_0,
            &mut device,
        )
        .expect("Can't get DX12 device")
    };
    let device = device.expect("Device is None");

    let mut info = D3D12_FEATURE_DATA_SHADER_MODEL {
        HighestShaderModel: D3D_SHADER_MODEL_6_0,
    };
    unsafe {
        device
            .CheckFeatureSupport(
                D3D12_FEATURE_SHADER_MODEL,
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<D3D12_FEATURE_DATA_SHADER_MODEL>() as u32,
            )
            .ok()
            .expect("Can't query shader model")
    };
    if info.HighestShaderModel < D3D_SHADER_MODEL_6_0 {
        println!("At least shader model 6.0 is needed to support DirectStorage.");
        exit(-1);
    }

    let factory: IDStorageFactory =
        unsafe { DStorageGetFactory().expect("Can't create DirectStorage factory") };

    let file: IDStorageFile = unsafe {
        factory
            .OpenFile(PCWSTR::from_raw(file_path.as_ptr()))
            .expect("Can't open file")
    };

    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        file.GetFileInformation(&mut info)
            .ok()
            .expect("Can't get file information")
    };
    let file_size = info.nFileSizeLow;

    // Create a DirectStorage queue which will be used to load data into a buffer on the GPU.
    let queue_desc = DSTORAGE_QUEUE_DESC {
        SourceType: DSTORAGE_REQUEST_SOURCE_FILE,
        Capacity: DSTORAGE_MAX_QUEUE_CAPACITY as u16,
        Priority: DSTORAGE_PRIORITY_NORMAL,
        Name: std::ptr::null(),
        Device: unsafe { readonly_copy(&device) },
    };

    let queue: IDStorageQueue = unsafe {
        factory
            .CreateQueue(&queue_desc)
            .expect("Can't create DirectStorage queue")
    };

    // Create the ID3D12Resource buffer which will be populated with the file's contents.
    let heap_props = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_DEFAULT,
        ..Default::default()
    };
    let buffer_desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: file_size as u64,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: DXGI_FORMAT_UNKNOWN,
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        ..Default::default()
    };

    let mut buffer_resource: Option<ID3D12Resource> = None;
    unsafe {
        device
            .CreateCommittedResource(
                &heap_props,
                D3D12_HEAP_FLAG_NONE,
                &buffer_desc,
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut buffer_resource,
            )
            .expect("Can't create committed resource")
    };
    let buffer_resource = buffer_resource.expect("Buffer Resource is None");

    // Enqueue a request to read the file contents into a destination D3D12 buffer resource.
    // Note: The example request below is performing a single read of the entire file contents.
    let mut options = DSTORAGE_REQUEST_OPTIONS::default();
    options.set_CompressionFormat(DSTORAGE_COMPRESSION_FORMAT_NONE);
    options.set_SourceType(DSTORAGE_REQUEST_SOURCE_FILE);
    options.set_DestinationType(DSTORAGE_REQUEST_DESTINATION_BUFFER);

    let request = DSTORAGE_REQUEST {
        Options: options,
        Source: DSTORAGE_SOURCE {
            File: ManuallyDrop::new(DSTORAGE_SOURCE_FILE {
                Source: unsafe { readonly_copy(&file) },
                Offset: 0,
                Size: file_size,
            }),
        },
        Destination: DSTORAGE_DESTINATION {
            Buffer: ManuallyDrop::new(DSTORAGE_DESTINATION_BUFFER {
                Resource: unsafe { readonly_copy(&buffer_resource) },
                Offset: 0,
                Size: file_size,
            }),
        },
        UncompressedSize: file_size,
        CancellationTag: 0,
        Name: std::ptr::null(),
    };

    println!("Enqueue Request to Queue.");

    unsafe { queue.EnqueueRequest(&request) }

    // Configure a fence to be signaled when the request is completed
    let fence: ID3D12Fence = unsafe {
        device
            .CreateFence(0, D3D12_FENCE_FLAG_NONE)
            .expect("Can't create a fence")
    };

    let fence_event =
        unsafe { CreateEventA(std::ptr::null(), false.into(), false.into(), PCSTR::null()) };
    assert!(!fence_event.0.is_null(), "Can't create event");

    const FENCE_VALUE: u64 = 1;

    unsafe {
        fence
            .SetEventOnCompletion(FENCE_VALUE, fence_event)
            .ok()
            .expect("Can't set on completion event");

        queue.EnqueueSignal(&fence, FENCE_VALUE);
        queue.Submit();
    }

    println!("Waiting for the DirectStorage request to complete.");

    unsafe {
        let _success = WaitForSingleObject(fence_event, 5 * 1000);

        CloseHandle(fence_event).expect("Failed to close Event object");
    };

    let mut error_record = Default::default();
    unsafe { queue.RetrieveErrorRecord(&mut error_record) };

    if error_record.FailureCount > 0 {
        println!(
            "The DirectStorage request failed. HRESULT: {}",
            error_record.FirstFailure.HResult
        );
    } else {
        println!("The DirectStorage request completed successfully.");
    }
}
