use my_open_scanner::{PipelineConfig, ScannerPipeline, ScanStatus};
use std::path::Path;

#[test]
fn test_real_sample_document_detection() {
    let path = Path::new("test_assets/sample_document.png");
    if !path.exists() {
        println!("test_assets/sample_document.png does not exist, skipping");
        return;
    }

    let pipeline = ScannerPipeline::new(PipelineConfig::default());
    let temp_dir = std::env::temp_dir().join("my_open_scanner_real_test");

    let result = pipeline
        .process_file(path, &temp_dir)
        .expect("Xử lý file ảnh thật");

    println!("🎯 Real Document Result:");
    println!("   Status: {:?}", result.status);
    println!("   Confidence: {:.2}", result.confidence_score);
    println!("   Deskew angle: {:.2}°", result.deskew_angle_deg);
    println!("   Paper size: {:?}", result.inferred_paper_size);
    println!("   Processing time: {} ms", result.processing_time_ms);
    if let Some(corners) = &result.detected_corners {
        println!("   Corners: TL={:?}, TR={:?}, BR={:?}, BL={:?}", corners.tl, corners.tr, corners.br, corners.bl);
    }

    // Phải nhận diện thành công (Success), không rơi vào Fallback
    assert_eq!(result.status, ScanStatus::Success, "Phải nhận diện 4 góc thành công trên ảnh thật");
    assert!(result.confidence_score >= 0.90);
    // Góc nghiêng tài liệu trong ảnh thật khoảng -3.6 độ
    assert!(result.deskew_angle_deg.abs() > 1.0, "Phải phát hiện góc nghiêng thực tế của tài liệu");

    // Dọn dẹp
    let _ = std::fs::remove_dir_all(&temp_dir);
}
