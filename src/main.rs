use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use rayon::prelude::*;

use my_open_scanner::{OutputFormat, PipelineConfig, ScannerPipeline, ScanResult};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliOutputFormat {
    Pdf,
    Png,
    Jpg,
    Both,
}

impl From<CliOutputFormat> for OutputFormat {
    fn from(f: CliOutputFormat) -> Self {
        match f {
            CliOutputFormat::Pdf => OutputFormat::Pdf,
            CliOutputFormat::Png => OutputFormat::Png,
            CliOutputFormat::Jpg => OutputFormat::Jpg,
            CliOutputFormat::Both => OutputFormat::Both,
        }
    }
}

/// MyOpenScanner: High-Performance Document Scanner Engine in Rust
#[derive(Parser, Debug)]
#[command(name = "my_open_scanner")]
#[command(author = "MyOpenScanner Contributors")]
#[command(version = "0.1.0")]
#[command(about = "Quét tài liệu từ camera điện thoại sang PDF/Image chất lượng cao", long_about = None)]
struct Args {
    /// Đường dẫn đến file ảnh đầu vào (đơn lẻ)
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Đường dẫn đến thư mục chứa ảnh để quét hàng loạt (batch mode)
    #[arg(short, long)]
    batch_dir: Option<PathBuf>,

    /// Thư mục xuất kết quả (PDF, Image, JSON)
    #[arg(short, long, default_value = "./output")]
    output_dir: PathBuf,

    /// DPI mục tiêu cho file PDF (200 hoặc 300)
    #[arg(long, default_value_t = 300)]
    dpi: u32,

    /// Định dạng xuất: pdf, image, hoặc both
    #[arg(long, value_enum, default_value_t = CliOutputFormat::Both)]
    format: CliOutputFormat,

    /// Độ sáng mục tiêu cho nền tài liệu (0 - 255)
    #[arg(long, default_value_t = 230)]
    target_luminance: u8,
}

fn main() {
    let args = Args::parse();

    let mut config = PipelineConfig::default();
    config.output_dpi = args.dpi;
    config.output_format = args.format.into();
    config.target_luminance = args.target_luminance;

    let pipeline = ScannerPipeline::new(config);

    if let Some(input_path) = args.input {
        // Chế độ quét 1 file
        println!("🚀 Đang xử lý tài liệu: {}", input_path.display());
        match pipeline.process_file(&input_path, &args.output_dir) {
            Ok(res) => {
                println!("✅ Hoàn tất thành công!");
                println!("   - File: {}", res.file_name);
                println!("   - Trạng thái: {:?}", res.status);
                println!("   - Góc deskew: {:.2}°", res.deskew_angle_deg);
                println!("   - Khổ giấy nhận diện: {:?}", res.inferred_paper_size);
                println!("   - Thời gian xử lý: {} ms", res.processing_time_ms);
                if let Some(path) = &res.output_path {
                    println!("   - Đường dẫn output: {}", path);
                }
            }
            Err(e) => {
                eprintln!("❌ Lỗi xử lý: {}", e);
                std::process::exit(1);
            }
        }
    } else if let Some(batch_dir) = args.batch_dir {
        // Chế độ quét batch thư mục song song bằng Rayon
        if !batch_dir.exists() || !batch_dir.is_dir() {
            eprintln!("❌ Thư mục không tồn tại: {}", batch_dir.display());
            std::process::exit(1);
        }

        println!("📂 Đang quét hàng loạt trong thư mục: {}", batch_dir.display());
        let entries: Vec<PathBuf> = std::fs::read_dir(&batch_dir)
            .expect("Không thể đọc thư mục")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    matches!(ext_lower.as_str(), "jpg" | "jpeg" | "png" | "webp")
                } else {
                    false
                }
            })
            .collect();

        if entries.is_empty() {
            println!("⚠️ Không tìm thấy ảnh hợp lệ (.jpg, .jpeg, .png, .webp).");
            return;
        }

        println!("⚙️ Tìm thấy {} tệp ảnh. Bắt đầu xử lý song song...", entries.len());
        let start_batch = Instant::now();

        let results: Vec<Result<ScanResult, String>> = entries
            .par_iter()
            .map(|path| pipeline.process_file(path, &args.output_dir))
            .collect();

        let elapsed = start_batch.elapsed();
        let total_count = results.len();
        let success_count = results.iter().filter(|r| r.is_ok()).count();

        println!("\n📊 TỔNG KẾT BATCH PROCESSING:");
        println!("   - Tổng số ảnh: {}", total_count);
        println!("   - Thành công: {} / {}", success_count, total_count);
        println!("   - Tổng thời gian: {:.2} s", elapsed.as_secs_f64());
        if total_count > 0 {
            println!(
                "   - Tốc độ trung bình: {:.1} ms / ảnh",
                elapsed.as_millis() as f64 / total_count as f64
            );
        }

        // Xuất summary.json
        let summary_results: Vec<ScanResult> = results.into_iter().filter_map(|r| r.ok()).collect();
        let summary_path = args.output_dir.join("summary.json");
        if let Ok(json_str) = serde_json::to_string_pretty(&summary_results) {
            let _ = std::fs::write(summary_path, json_str);
        }
    } else {
        eprintln!("⚠️ Vui lòng cung cấp --input <file> hoặc --batch-dir <thư_mục>.");
        eprintln!("   Chạy `my_open_scanner --help` để xem hướng dẫn.");
    }
}
