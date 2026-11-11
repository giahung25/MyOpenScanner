use std::path::Path;
use std::time::Instant;

use image::RgbImage;

use crate::models::{PaperSize, PipelineConfig, ScanResult, ScanStatus};
use crate::modules::detail_enhancer::DetailEnhancer;
use crate::modules::geometric_rectifier::GeometricRectifier;
use crate::modules::illumination_normalizer::IlluminationNormalizer;
use crate::modules::page_detector::PageDetector;
use crate::modules::pdf_exporter::PdfExporter;

/// Pipeline điều phối 5 bước xử lý của MyOpenScanner
pub struct ScannerPipeline {
    pub config: PipelineConfig,
}

impl ScannerPipeline {
    pub fn new(config: PipelineConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(PipelineConfig::default())
    }

    /// Xử lý một ảnh từ đường dẫn file
    pub fn process_file(
        &self,
        input_path: &Path,
        output_dir: &Path,
    ) -> Result<ScanResult, String> {
        let file_name = input_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unknown")
            .to_string();

        let stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document");

        let output_base_path = output_dir.join(format!("{}_scanned", stem));

        // Đọc ảnh đầu vào
        let img = image::open(input_path)
            .map_err(|e| format!("Không thể mở ảnh {}: {}", input_path.display(), e))?
            .to_rgb8();

        self.process_image(&img, &file_name, &output_base_path)
    }

    /// Xử lý buffer ảnh RGB đã nạp trong bộ nhớ
    pub fn process_image(
        &self,
        img: &RgbImage,
        file_name: &str,
        output_base_path: &Path,
    ) -> Result<ScanResult, String> {
        let start_time = Instant::now();

        // Khởi tạo ScanResult mặc định
        let mut result = ScanResult {
            file_name: file_name.to_string(),
            status: ScanStatus::Success,
            detected_corners: None,
            deskew_angle_deg: 0.0,
            confidence_score: 1.0,
            inferred_paper_size: PaperSize::Custom,
            target_dpi: self.config.output_dpi,
            processing_time_ms: 0,
            output_path: None,
            searchable_pdf_path: None,
            error_message: None,
        };

        // --- BƯỚC 1: PAGE DETECTOR (Bắt 4 góc tài liệu) ---
        let detector = PageDetector::from_config(&self.config);
        let (corners, confidence, status) = detector.detect(img);
        result.detected_corners = Some(corners);
        result.deskew_angle_deg = corners.deskew_angle_deg();
        result.confidence_score = confidence;
        result.status = status;

        // --- BƯỚC 2: GEOMETRIC RECTIFIER (Cắt biên, Nắn phối cảnh & Làm phẳng) ---
        let rectifier = GeometricRectifier::from_config(&self.config);
        let (rectified_img, paper_size, _h_matrix) = rectifier.rectify(img, &corners);
        result.inferred_paper_size = paper_size;

        // --- BƯỚC 3: ILLUMINATION NORMALIZER (Khử bóng & Cân bằng sáng LAB) ---
        let normalized_img = IlluminationNormalizer::normalize(&rectified_img, &self.config);

        // --- BƯỚC 4: DETAIL ENHANCER (Làm nét chữ mảnh, chữ ký & Barcode) ---
        let enhanced_img = DetailEnhancer::enhance(&normalized_img, &self.config);

        // --- BƯỚC 5: PDF EXPORTER (Đóng gói PDF chuẩn DPI & Xuất JSON) ---
        let export_res = PdfExporter::export(
            &enhanced_img,
            output_base_path,
            paper_size,
            &mut result,
            &self.config,
        );

        if let Err(e) = export_res {
            result.status = ScanStatus::Error;
            result.error_message = Some(e);
        }

        // --- BƯỚC 6 (TÙY CHỌN): TẠO SEARCHABLE PDF (Lớp text ẩn bôi đen được) ---
        if self.config.searchable_pdf {
            let img_path = output_base_path.with_extension("png");
            let temp_img_created = if !img_path.exists() {
                let _ = enhanced_img.save(&img_path);
                true
            } else {
                false
            };

            let file_stem = output_base_path.file_name().unwrap_or_default().to_string_lossy();
            let parent_dir = output_base_path.parent().unwrap_or(Path::new("."));
            let searchable_pdf = parent_dir.join(format!("{}_searchable.pdf", file_stem));
            let text_file = parent_dir.join(format!("{}_text.txt", file_stem));
            let ocr_json = parent_dir.join(format!("{}_ocr.json", file_stem));

            let ocr_script = Path::new("scripts/ocr_processor.py");
            if ocr_script.exists() {
                let cmd_res = std::process::Command::new("python")
                    .arg(ocr_script)
                    .arg("--image")
                    .arg(&img_path)
                    .arg("--output-pdf")
                    .arg(&searchable_pdf)
                    .arg("--output-text")
                    .arg(&text_file)
                    .arg("--output-json")
                    .arg(&ocr_json)
                    .arg("--dpi")
                    .arg(self.config.output_dpi.to_string())
                    .status();

                if let Ok(exit_status) = cmd_res {
                    if exit_status.success() {
                        result.searchable_pdf_path = Some(searchable_pdf.to_string_lossy().to_string());
                    }
                }
            }

            if temp_img_created && !matches!(self.config.output_format, crate::models::OutputFormat::Png | crate::models::OutputFormat::Both) {
                let _ = std::fs::remove_file(&img_path);
            }
        }

        result.processing_time_ms = start_time.elapsed().as_millis() as u64;

        // Cập nhật lại file JSON metadata với thời gian xử lý chính xác
        let json_path = output_base_path.with_extension("json");
        if let Ok(json_str) = serde_json::to_string_pretty(&result) {
            let _ = std::fs::write(&json_path, json_str);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_synthetic_document() {
        let mut img = RgbImage::new(400, 500);
        // Nền mặt bàn màu sẫm
        for p in img.pixels_mut() {
            *p = image::Rgb([60, 40, 30]);
        }

        // Vẽ trang giấy trắng ở giữa
        for y in 50..450 {
            for x in 50..350 {
                img.put_pixel(x, y, image::Rgb([235, 230, 220]));
            }
        }

        let pipeline = ScannerPipeline::with_default_config();
        let temp_dir = std::env::temp_dir().join("my_open_scanner_test");
        let output_base = temp_dir.join("test_doc");

        let res = pipeline
            .process_image(&img, "test_doc.jpg", &output_base)
            .expect("Pipeline thực thi thành công");

        assert!(res.status == ScanStatus::Success || res.status == ScanStatus::Fallback);
        assert!(res.detected_corners.is_some());
        assert!(res.output_path.is_some());

        // Dọn dẹp
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
