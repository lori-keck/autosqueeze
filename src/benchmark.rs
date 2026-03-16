/// benchmark.rs — FIXED. DO NOT MODIFY.
///
/// This is the scoring function. It measures:
/// 1. Compression ratio (compressed_size / original_size) — lower is better
/// 2. Compression speed (MB/s)
/// 3. Decompression speed (MB/s)
/// 4. Correctness — decompress(compress(data)) must equal original data
///
/// The agent runs this after every modification to compress.rs.
/// If correctness fails, the experiment is a failure regardless of ratio.
///
/// Usage: cargo run --release --bin benchmark

// Import compress/decompress from the compress module
// We compile compress.rs as a library for benchmarking
#[allow(dead_code, unused_imports)]
mod compress_lib {
    include!("compress.rs");
}

use std::fs;
use std::path::Path;
use std::time::Instant;

fn main() {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║              crunch benchmark                   ║");
    println!("╚══════════════════════════════════════════════════╝\n");

    let corpus_dir = Path::new("corpus");
    if !corpus_dir.exists() {
        eprintln!("ERROR: corpus/ directory not found. Run prepare.sh first.");
        std::process::exit(1);
    }

    let mut total_original: usize = 0;
    let mut total_compressed: usize = 0;
    let mut total_compress_time_ns: u128 = 0;
    let mut total_decompress_time_ns: u128 = 0;
    let mut all_correct = true;
    let mut file_count = 0;

    // Collect and sort files for deterministic order
    let mut entries: Vec<_> = fs::read_dir(corpus_dir)
        .expect("Failed to read corpus/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in &entries {
        let path = entry.path();
        let filename = path.file_name().unwrap().to_string_lossy();
        let data = fs::read(&path).expect("Failed to read file");

        if data.is_empty() {
            continue;
        }

        // Compress
        let compress_start = Instant::now();
        let compressed = compress_lib::compress(&data);
        let compress_time = compress_start.elapsed();

        // Decompress
        let decompress_start = Instant::now();
        let decompressed = compress_lib::decompress(&compressed);
        let decompress_time = decompress_start.elapsed();

        // Correctness check
        let correct = decompressed == data;
        if !correct {
            all_correct = false;
        }

        let ratio = compressed.len() as f64 / data.len() as f64;
        let compress_speed = data.len() as f64 / compress_time.as_secs_f64() / 1_000_000.0;
        let decompress_speed = data.len() as f64 / decompress_time.as_secs_f64() / 1_000_000.0;

        println!(
            "  {:<30} {:>8} → {:>8}  ratio: {:.4}  c: {:>7.1} MB/s  d: {:>7.1} MB/s  {}",
            filename,
            format_size(data.len()),
            format_size(compressed.len()),
            ratio,
            compress_speed,
            decompress_speed,
            if correct { "✅" } else { "❌ CORRUPT" }
        );

        total_original += data.len();
        total_compressed += compressed.len();
        total_compress_time_ns += compress_time.as_nanos();
        total_decompress_time_ns += decompress_time.as_nanos();
        file_count += 1;
    }

    if file_count == 0 {
        eprintln!("ERROR: No files found in corpus/");
        std::process::exit(1);
    }

    // Summary
    let overall_ratio = total_compressed as f64 / total_original as f64;
    let overall_compress_speed =
        total_original as f64 / (total_compress_time_ns as f64 / 1_000_000_000.0) / 1_000_000.0;
    let overall_decompress_speed =
        total_original as f64 / (total_decompress_time_ns as f64 / 1_000_000_000.0) / 1_000_000.0;

    println!("\n──────────────────────────────────────────────────────────────────────");
    println!("  TOTAL: {} files", file_count);
    println!(
        "  Original: {}  Compressed: {}",
        format_size(total_original),
        format_size(total_compressed)
    );
    println!();

    // The scores — these are what the agent optimizes
    println!("  ┌─────────────────────────────────────────┐");
    println!("  │ COMPRESSION RATIO:  {:<20.6} │", overall_ratio);
    println!("  │ COMPRESS SPEED:     {:<17.1} MB/s │", overall_compress_speed);
    println!("  │ DECOMPRESS SPEED:   {:<17.1} MB/s │", overall_decompress_speed);
    println!(
        "  │ CORRECTNESS:        {:<20} │",
        if all_correct { "PASS ✅" } else { "FAIL ❌" }
    );
    println!("  └─────────────────────────────────────────┘");

    // Machine-readable output for the agent
    println!("\n[SCORE]");
    println!("ratio={:.6}", overall_ratio);
    println!("compress_speed_mbps={:.1}", overall_compress_speed);
    println!("decompress_speed_mbps={:.1}", overall_decompress_speed);
    println!("correct={}", all_correct);

    if !all_correct {
        println!("\n⚠️  EXPERIMENT FAILED: Decompression produced incorrect output.");
        println!("    This change must be reverted.");
        std::process::exit(1);
    }
}

fn format_size(bytes: usize) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}
