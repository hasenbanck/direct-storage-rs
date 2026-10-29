//
// Copyright (c) Microsoft. All rights reserved.
// This code is licensed under the MIT License (MIT).
// THIS CODE IS PROVIDED *AS IS* WITHOUT WARRANTY OF
// ANY KIND, EITHER EXPRESS OR IMPLIED, INCLUDING ANY
// IMPLIED WARRANTIES OF FITNESS FOR A PARTICULAR
// PURPOSE, MERCHANTABILITY, OR NON-INFRINGEMENT.
//

use std::{
    io::Write,
    mem::ManuallyDrop,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::exit,
    thread::sleep,
    time::Duration,
};

use direct_storage::{
    readonly_copy,
    runtime_loaded::{
        DStorageCreateCompressionCodec, DStorageGetFactory, DStorageSetConfiguration,
    },
    IDStorageCompressionCodec, IDStorageFactory, IDStorageFile, IDStorageQueue,
    DSTORAGE_COMMAND_TYPE_REQUEST, DSTORAGE_COMPRESSION_BEST_RATIO, DSTORAGE_COMPRESSION_FORMAT,
    DSTORAGE_COMPRESSION_FORMAT_GDEFLATE, DSTORAGE_COMPRESSION_FORMAT_NONE, DSTORAGE_CONFIGURATION,
    DSTORAGE_DEBUG_BREAK_ON_ERROR, DSTORAGE_DEBUG_SHOW_ERRORS, DSTORAGE_DESTINATION,
    DSTORAGE_DESTINATION_BUFFER, DSTORAGE_MAX_QUEUE_CAPACITY, DSTORAGE_PRIORITY_NORMAL,
    DSTORAGE_QUEUE_DESC, DSTORAGE_REQUEST, DSTORAGE_REQUEST_DESTINATION_BUFFER,
    DSTORAGE_REQUEST_OPTIONS, DSTORAGE_REQUEST_SOURCE_FILE, DSTORAGE_SOURCE, DSTORAGE_SOURCE_FILE,
};
use windows::{
    core::{PCSTR, PCWSTR},
    Win32::{
        Foundation::CloseHandle,
        Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_12_0,
            Direct3D12::{
                D3D12CreateDevice, ID3D12Device, ID3D12Fence, ID3D12Resource,
                D3D12_FEATURE_DATA_SHADER_MODEL, D3D12_FEATURE_SHADER_MODEL, D3D12_FENCE_FLAG_NONE,
                D3D12_HEAP_FLAG_NONE, D3D12_HEAP_PROPERTIES, D3D12_HEAP_TYPE_DEFAULT,
                D3D12_RESOURCE_DESC, D3D12_RESOURCE_DIMENSION_BUFFER, D3D12_RESOURCE_STATE_COMMON,
                D3D12_TEXTURE_LAYOUT_ROW_MAJOR, D3D_SHADER_MODEL_6_0,
            },
            Dxgi::Common::{DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC},
        },
        System::{
            Threading::{CreateEventA, GetCurrentProcess, WaitForSingleObject},
            WindowsProgramming::QueryProcessCycleTime,
        },
    },
};

#[derive(Copy, Clone, PartialEq, Eq)]
enum TestCase {
    Uncompressed,
    CpuGDeflate,
    GpuGDeflate,
}

#[derive(Copy, Clone)]
struct ChunkMetadata {
    compressed: bool,
    offset: u32,
    compressed_size: u32,
    uncompressed_size: u32,
}

#[derive(Clone)]
struct Metadata {
    uncompressed_size: u32,
    compressed_size: u32,
    chunks: Vec<ChunkMetadata>,
}

#[derive(Copy, Clone)]
struct Result {
    test_case: TestCase,
    staging_buffer_size_mib: u32,
    data: TestResult,
}

impl PartialEq<Self> for Result {
    fn eq(&self, other: &Self) -> bool {
        self.test_case == other.test_case
            && self.staging_buffer_size_mib == other.staging_buffer_size_mib
    }
}

impl Eq for Result {}

#[derive(Copy, Clone)]
struct TestResult {
    bandwidth: f64,
    process_cycles: u64,
}

pub fn main() {
    let test_cases = [
        TestCase::Uncompressed,
        TestCase::CpuGDeflate,
        TestCase::GpuGDeflate,
    ];

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        show_help_text();
        exit(-1);
    }

    let original_file_path = PathBuf::from(&args[1]);
    let gdeflate_file_path = Path::new(&original_file_path).with_extension("gdeflate");

    let mut chunk_size_mib = 16;
    if args.len() > 2 {
        chunk_size_mib = args[2]
            .parse::<u32>()
            .expect("Second argument not a valid 32 bit unsigned integer");
        if chunk_size_mib == 0 {
            show_help_text();
            println!("\nInvalid chunk size: {}", args[2]);
            exit(-1);
        }
    }
    let chunk_size_bytes = chunk_size_mib * 1024 * 1024;

    let uncompressed_metadata = uncompressed(&original_file_path, chunk_size_bytes);

    let compress_metadata = compressed(
        DSTORAGE_COMPRESSION_FORMAT_GDEFLATE,
        &original_file_path,
        &gdeflate_file_path,
        chunk_size_bytes,
    );

    let staging_sizes_mib: Vec<u32> = (0..=8).map(|i| 2u32.pow(i)).collect();

    let mut results: Vec<Result> = Vec::new();

    for test_case in test_cases {
        let compression_format: DSTORAGE_COMPRESSION_FORMAT;
        let num_runs: u32;
        let metadata: &Metadata;
        let file_path: &PathBuf;
        let mut configuration = DSTORAGE_CONFIGURATION::default();

        match test_case {
            TestCase::Uncompressed => {
                compression_format = DSTORAGE_COMPRESSION_FORMAT_NONE;
                num_runs = 10;
                metadata = &uncompressed_metadata;
                file_path = &original_file_path;
                println!("\nUNCOMPRESSED:");
            }
            TestCase::CpuGDeflate => {
                compression_format = DSTORAGE_COMPRESSION_FORMAT_GDEFLATE;
                num_runs = 10;

                configuration.NumBuiltInCpuDecompressionThreads = 0; // Best guess by the system.
                configuration.DisableGpuDecompression = true.into();

                metadata = &compress_metadata;
                file_path = &gdeflate_file_path;
                println!("\nCPU GDEFLATE:");
            }
            TestCase::GpuGDeflate => {
                compression_format = DSTORAGE_COMPRESSION_FORMAT_GDEFLATE;
                num_runs = 10;

                metadata = &compress_metadata;
                file_path = &gdeflate_file_path;
                println!("\nGPU GDEFLATE:");
            }
        }

        let mut factory = unsafe {
            DStorageSetConfiguration(&configuration)
                .expect("Can't set DirectStorage configuration");

            let factory: IDStorageFactory =
                DStorageGetFactory().expect("Can't get DirectStorage factory");

            factory.SetDebugFlags(DSTORAGE_DEBUG_SHOW_ERRORS | DSTORAGE_DEBUG_BREAK_ON_ERROR);

            factory
        };

        for staging_buffer_size_mib in staging_sizes_mib.iter().copied() {
            if staging_buffer_size_mib < chunk_size_mib {
                continue;
            }

            let data = run_test(
                &mut factory,
                staging_buffer_size_mib,
                file_path,
                compression_format,
                metadata,
                num_runs,
            );

            results.push(Result {
                test_case,
                staging_buffer_size_mib,
                data,
            });
        }
    }

    println!();

    let header =
        "\"Staging Buffer Size MiB\"\t\"Uncompressed\"\t\"CPU GDEFLATE\"\t\"GPU GDEFLATE\"";

    let mut bandwith = header.to_owned();
    let mut cycles = header.to_owned();

    for staging_buffer_size_mib in staging_sizes_mib {
        let mut bandwith_row = format!("\n{staging_buffer_size_mib}\t");
        let mut cycles_row = format!("\n{staging_buffer_size_mib}\t");

        let mut found_one = false;

        for test_case in test_cases.iter().copied() {
            let it = results.iter().find(|r| {
                r.test_case == test_case && r.staging_buffer_size_mib == staging_buffer_size_mib
            });

            if let Some(it) = it {
                bandwith_row.push_str(&format!("{:.2}\t", it.data.bandwidth));
                cycles_row.push_str(&format!("{:.2}\t", it.data.process_cycles));
                found_one = true;
            }
        }

        if found_one {
            bandwith_row.push('\t');
            cycles_row.push('\t');

            bandwith.push_str(bandwith_row.as_str());
            cycles.push_str(cycles_row.as_str());

            bandwith_row.push('\n');
            cycles_row.push('\n');
        }
    }

    let mut combined = format!(
        "Bandwith in GB/s\n{bandwith}\n\nCycles\n{cycles}\n\nCompression\nCase\tSize\tRatio\n",
    );

    ratio_line(&mut combined, "Uncompressed", &uncompressed_metadata);
    ratio_line(&mut combined, "Compressed", &compress_metadata);
    combined.push('\n');

    println!("{}", combined.as_str());
}

fn ratio_line(s: &mut String, name: &str, metadata: &Metadata) {
    s.push_str(&format!(
        "{name}\t{}\t\t{:.2}\n",
        metadata.compressed_size,
        metadata.compressed_size as f64 / metadata.uncompressed_size as f64
    ));
}

fn uncompressed(original_file_path: &PathBuf, chunk_size_bytes: u32) -> Metadata {
    let file = std::fs::File::open(original_file_path).expect("Can't open file");
    let size = file.metadata().expect("No metadata available").len();
    let size = u32::try_from(size).expect("File is bigger than u32::MAX");

    let mut chunks_metadata = Vec::new();

    let mut offset = 0;
    while offset < size {
        let chunk_size = u32::min(size - offset, chunk_size_bytes);
        chunks_metadata.push(ChunkMetadata {
            compressed: false,
            offset,
            compressed_size: chunk_size,
            uncompressed_size: chunk_size,
        });

        offset += chunk_size;
    }

    Metadata {
        uncompressed_size: size,
        compressed_size: size,
        chunks: chunks_metadata,
    }
}

fn compressed(
    compression: DSTORAGE_COMPRESSION_FORMAT,
    original_file_path: &PathBuf,
    compressed_file_path: &PathBuf,
    chunk_size_bytes: u32,
) -> Metadata {
    let uncompressed_data = std::fs::read(original_file_path).expect("Can't read file");
    let uncompressed_size =
        u32::try_from(uncompressed_data.len()).expect("File content is bigger than u32::MAX");

    let mut compressed_file =
        std::fs::File::create(compressed_file_path).expect("Can't create compressed file");

    let num_chunks = uncompressed_size.div_ceil(chunk_size_bytes);

    println!(
        "Compressing {original_file_path:?} to {compressed_file_path:?} in {num_chunks} x {} MiB chunks",
        chunk_size_bytes / 1024 / 1024
    );

    let codec: IDStorageCompressionCodec =
        unsafe { DStorageCreateCompressionCodec(compression, 0) }.expect("Can' create codec");

    let mut total_compressed_size = 0;
    let mut chunks: Vec<Vec<u8>> = Vec::with_capacity(num_chunks as usize);
    let mut chunks_metadata: Vec<ChunkMetadata> = Vec::with_capacity(num_chunks as usize);

    for chunk_offset in (0..num_chunks).map(|i| i * chunk_size_bytes) {
        let chunk_size = u32::min(uncompressed_size - chunk_offset, chunk_size_bytes);

        let bound = unsafe { codec.CompressBufferBound(chunk_size as usize) };
        let mut chunk = Vec::with_capacity(bound);

        let mut compressed_size: usize = 0;
        unsafe {
            codec
                .CompressBuffer(
                    &uncompressed_data[chunk_offset as usize] as *const u8 as *const _,
                    chunk_size as usize,
                    DSTORAGE_COMPRESSION_BEST_RATIO,
                    chunk.as_mut_ptr() as *mut _,
                    bound,
                    &mut compressed_size,
                )
                .expect("Can't compress buffer");
            chunk.set_len(compressed_size);
        }

        if compressed_size < chunk_size as usize {
            let offset = total_compressed_size;
            total_compressed_size += compressed_size;

            chunks.push(chunk);

            chunks_metadata.push(ChunkMetadata {
                compressed: true,
                offset: offset as u32,
                compressed_size: compressed_size as u32,
                uncompressed_size: chunk_size,
            })
        } else {
            // It's more efficient to save the uncompressed chunk.
            let offset = total_compressed_size;
            total_compressed_size += chunk_size as usize;

            unsafe { chunk.set_len(chunk_size as usize) };
            chunk.copy_from_slice(
                &uncompressed_data[chunk_offset as usize..(chunk_offset + chunk_size) as usize],
            );
            chunks.push(chunk);

            chunks_metadata.push(ChunkMetadata {
                compressed: false,
                offset: offset as u32,
                compressed_size: chunk_size,
                uncompressed_size: chunk_size,
            });
        }
    }

    println!(
        "Compressed from {uncompressed_size} to {total_compressed_size} bytes ({:.2}%)",
        (total_compressed_size as f64 / uncompressed_size as f64) * 100.0
    );

    for chunk in chunks {
        compressed_file
            .write_all(&chunk)
            .expect("Can't write compressed data in file");
    }
    compressed_file
        .flush()
        .expect("Can't flush compressed file");

    Metadata {
        uncompressed_size,
        compressed_size: total_compressed_size as u32,
        chunks: chunks_metadata,
    }
}

fn run_test(
    factory: &mut IDStorageFactory,
    staging_buffer_size_mib: u32,
    source_filename: &Path,
    compression_format: DSTORAGE_COMPRESSION_FORMAT,
    metadata: &Metadata,
    num_runs: u32,
) -> TestResult {
    let wide_file_name: Vec<u16> = source_filename
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();

    let file: IDStorageFile =
        unsafe { factory.OpenFile(PCWSTR::from_raw(wide_file_name.as_ptr())) }
            .expect("Can't create DirectStorage file");

    // The staging buffer size must be set before any queues are created.
    print!("Staging buffer: {staging_buffer_size_mib} MiB");
    unsafe { factory.SetStagingBufferSize(staging_buffer_size_mib * 1024 * 1024) }
        .expect("Can't set staging buffer size");

    let mut device = None::<ID3D12Device>;
    unsafe {
        D3D12CreateDevice(None, D3D_FEATURE_LEVEL_12_0, &mut device).expect("Can't get DX12 device")
    };
    let device = device.expect("Device is None");

    let mut info = D3D12_FEATURE_DATA_SHADER_MODEL {
        HighestShaderModel: D3D_SHADER_MODEL_6_0,
    };
    unsafe {
        device
            .CheckFeatureSupport(
                D3D12_FEATURE_SHADER_MODEL,
                <*mut _>::cast(&mut info),
                std::mem::size_of_val(&info) as u32,
            )
            .expect("Can't query shader model")
    };
    if info.HighestShaderModel.0 < D3D_SHADER_MODEL_6_0.0 {
        println!("\nAt least shader model 6.0 is needed to support DirectStorage.");
        exit(-1);
    }

    // Create a DirectStorage queue which will be used to load data into a buffer on the GPU.
    let queue_desc = DSTORAGE_QUEUE_DESC {
        SourceType: DSTORAGE_REQUEST_SOURCE_FILE,
        Capacity: DSTORAGE_MAX_QUEUE_CAPACITY as u16,
        Priority: DSTORAGE_PRIORITY_NORMAL,
        Name: PCSTR::null(),
        Device: unsafe { readonly_copy(&device) },
    };

    let queue: IDStorageQueue =
        unsafe { factory.CreateQueue(&queue_desc) }.expect("Can't create DirectStorage queue");

    // Create the ID3D12Resource buffer which will be populated with the file's contents.
    let heap_props = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_DEFAULT,
        ..Default::default()
    };
    let buffer_desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: metadata.uncompressed_size as u64,
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

    let fence: ID3D12Fence = unsafe {
        device
            .CreateFence(0, D3D12_FENCE_FLAG_NONE)
            .expect("Can't create a fence")
    };

    let fence_event =
        unsafe { CreateEventA(None, false, false, None).expect("Can't create event") };

    let mut mean_bandwidth: f64 = 0.0;
    let mut mean_cycle_time: u64 = 0;

    for fence_value in 1..=(num_runs as u64) {
        let mut dst_offset = 0;

        unsafe {
            fence
                .SetEventOnCompletion(fence_value, fence_event)
                .expect("Can't set completion event")
        };

        for chunk in &metadata.chunks {
            let compression_format = if chunk.compressed {
                compression_format
            } else {
                DSTORAGE_COMPRESSION_FORMAT_NONE
            };

            let mut options = DSTORAGE_REQUEST_OPTIONS::default();
            options.set_CompressionFormat(compression_format);
            options.set_SourceType(DSTORAGE_REQUEST_SOURCE_FILE);
            options.set_DestinationType(DSTORAGE_REQUEST_DESTINATION_BUFFER);

            let file = file.clone();
            let buffer_resource = buffer_resource.clone();

            let request = DSTORAGE_REQUEST {
                Options: options,
                Source: DSTORAGE_SOURCE {
                    File: ManuallyDrop::new(DSTORAGE_SOURCE_FILE {
                        Source: unsafe { readonly_copy(&file) },
                        Offset: chunk.offset as u64,
                        Size: chunk.compressed_size,
                    }),
                },
                Destination: DSTORAGE_DESTINATION {
                    Buffer: ManuallyDrop::new(DSTORAGE_DESTINATION_BUFFER {
                        Resource: unsafe { readonly_copy(&buffer_resource) },
                        Offset: dst_offset,
                        Size: chunk.uncompressed_size,
                    }),
                },
                UncompressedSize: chunk.uncompressed_size,
                CancellationTag: 0,
                Name: PCSTR::null(),
            };

            unsafe { queue.EnqueueRequest(&request) };

            dst_offset += request.UncompressedSize as u64;
        }

        unsafe { queue.EnqueueSignal(&fence, fence_value) }

        let start_time = std::time::Instant::now();
        let start_cycle_time = get_process_cycle_time();

        unsafe {
            queue.Submit();
            WaitForSingleObject(fence_event, 5 * 1000);
        }

        let end_cycle_time = get_process_cycle_time();
        let end_time = std::time::Instant::now();

        let completed_value = unsafe { fence.GetCompletedValue() };

        if completed_value == u64::MAX {
            // Device removed!  Give DirectStorage a chance to detect the error.
            sleep(Duration::from_secs(5));
        }

        let error_record = unsafe { queue.RetrieveErrorRecord() };

        if error_record.FirstFailure.HResult.is_err() {
            println!(
                "\n\nThe DirectStorage request failed. HRESULT: {}",
                error_record.FirstFailure.HResult
            );
            if error_record.FirstFailure.CommandType == DSTORAGE_COMMAND_TYPE_REQUEST {
                let request = unsafe { error_record.FirstFailure.Anonymous.Request };
                let file = unsafe { &request.Request.Source.File };
                let offset = file.Offset;
                let size = file.Size;
                println!("Offset: {offset} Size: {size}");
            }
            exit(-1);
        }

        let duration_in_seconds = end_time.duration_since(start_time).as_secs_f64();
        let bandwidth =
            (metadata.uncompressed_size as f64 / duration_in_seconds) / 1000.0 / 1000.0 / 1000.0;

        mean_bandwidth += bandwidth;
        mean_cycle_time += end_cycle_time - start_cycle_time;
    }

    unsafe {
        CloseHandle(fence_event).expect("Can't close fence event");
    }

    mean_bandwidth /= num_runs as f64;
    mean_cycle_time /= num_runs as u64;

    println!("\t...... {mean_bandwidth:.2} GB/s, mean cycle time: {mean_cycle_time}");

    TestResult {
        bandwidth: mean_bandwidth,
        process_cycles: mean_cycle_time,
    }
}

fn show_help_text() {
    println!(
        "Compresses a file, saves it to disk, and then loads & decompresses using DirectStorage.\n"
    );
    println!("Arguments: <path> [chunk size in MiB]\n");
    println!("Default chunk size is 16 MiB.")
}

#[inline(always)]
fn get_process_cycle_time() -> u64 {
    let mut cycles = 0;
    unsafe { QueryProcessCycleTime(GetCurrentProcess(), &mut cycles) }
        .expect("Failed to query process cycle time");
    cycles
}
