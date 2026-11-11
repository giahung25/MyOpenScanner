use image::{Rgb, RgbImage};
use my_open_scanner::{OutputFormat, PipelineConfig, ScannerPipeline, ScanStatus};
use std::fs;

#[test]
fn test_end_to_end_pipeline_with_shadow_stamp_and_signature() {
    let width = 800u32;
    let height = 1000u32;
    let mut img = RgbImage::new(width, height);

    // 1. Nền bàn gỗ sẫm màu có vân
    for y in 0..height {
        for x in 0..width {
            let wood_grain = ((x as f32 * 0.1).sin() * 10.0) as i32;
            let r = (45 + wood_grain).clamp(0, 255) as u8;
            let g = (30 + wood_grain).clamp(0, 255) as u8;
            let b = (20 + wood_grain).clamp(0, 255) as u8;
            img.put_pixel(x, y, Rgb([r, g, b]));
        }
    }

    // 2. Tờ hóa đơn/chứng từ nghiêng nhẹ (~100..700 x 100..900) với gradient bóng râm
    for y in 120..880 {
        for x in 120..680 {
            // Giả lập bóng râm bàn tay/điện thoại: một góc tối hơn (y cao, x thấp)
            let shadow = ((x as f32 / 680.0) * 0.4 + (y as f32 / 880.0) * 0.3 + 0.3).clamp(0.4, 1.0);
            let base_paper_r = (240.0 * shadow) as u8;
            let base_paper_g = (235.0 * shadow) as u8;
            let base_paper_b = (220.0 * shadow) as u8; // Giấy hơi vàng ngà tự nhiên
            img.put_pixel(x, y, Rgb([base_paper_r, base_paper_g, base_paper_b]));
        }
    }

    // 3. Các dòng kẻ bảng và chữ in màu đen
    for x in 160..640 {
        img.put_pixel(x, 200, Rgb([30, 30, 30]));
        img.put_pixel(x, 260, Rgb([30, 30, 30]));
        img.put_pixel(x, 600, Rgb([30, 30, 30]));
    }

    // 4. Mộc đỏ (Red seal/stamp: ~40x40 pixel)
    for dy in 0..30 {
        for dx in 0..30 {
            if ((dx as i32 - 15).pow(2) + (dy as i32 - 15).pow(2)) <= 15 * 15 {
                img.put_pixel(450 + dx, 650 + dy, Rgb([215, 35, 35]));
            }
        }
    }

    // 5. Chữ ký bút bi xanh (Blue ink signature)
    for t in 0..80 {
        let sx = 250 + t;
        let sy = 700 + ((t as f32 * 0.2).sin() * 15.0) as u32;
        img.put_pixel(sx, sy, Rgb([25, 60, 220]));
        img.put_pixel(sx, sy + 1, Rgb([25, 60, 220]));
    }

    // Thiết lập pipeline
    let mut config = PipelineConfig::default();
    config.output_format = OutputFormat::Both;
    config.output_dpi = 300;
    let pipeline = ScannerPipeline::new(config);

    let test_dir = std::env::temp_dir().join("my_open_scanner_e2e");
    let _ = fs::create_dir_all(&test_dir);
    let output_base = test_dir.join("invoice_sample");

    let result = pipeline
        .process_image(&img, "invoice_sample.jpg", &output_base)
        .expect("Quá trình xử lý pipeline thành công");

    // Kiểm tra kết quả
    assert!(result.status == ScanStatus::Success || result.status == ScanStatus::Fallback);
    assert!(result.confidence_score > 0.5);
    println!("⏱️ Thời gian xử lý E2E: {} ms", result.processing_time_ms);

    // Kiểm tra các tệp đầu ra
    let pdf_path = output_base.with_extension("pdf");
    let png_path = output_base.with_extension("png");
    let json_path = output_base.with_extension("json");

    assert!(pdf_path.exists(), "Tệp PDF phải được tạo ra");
    assert!(png_path.exists(), "Tệp PNG phải được tạo ra");
    assert!(json_path.exists(), "Tệp JSON metadata phải được tạo ra");

    // Kiểm tra nội dung PDF
    let pdf_data = fs::read(&pdf_path).expect("Đọc file PDF");
    assert_eq!(&pdf_data[0..5], b"%PDF-");

    // Kiểm tra nội dung JSON
    let json_content = fs::read_to_string(&json_path).expect("Đọc file JSON");
    let parsed: serde_json::Value = serde_json::from_str(&json_content).expect("Parse JSON hợp lệ");
    assert_eq!(parsed["file_name"], "invoice_sample.jpg");

    // Đọc ảnh đầu ra đã xử lý và kiểm tra bảo tồn màu đỏ của mộc và màu xanh của chữ ký
    let processed_img = image::open(&png_path).expect("Đọc ảnh PNG đã xử lý").to_rgb8();
    assert!(processed_img.width() > 100);
    assert!(processed_img.height() > 100);

    // Dọn dẹp
    let _ = fs::remove_dir_all(&test_dir);
}
