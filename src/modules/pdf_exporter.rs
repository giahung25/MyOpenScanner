use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::RgbImage;
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};

use crate::models::{OutputFormat, PaperSize, PipelineConfig, ScanResult};

/// Module 5: PDF Exporter
/// Đóng gói ảnh đã quét thành file PDF chuẩn DPI in ấn (200 / 300 DPI)
/// và/hoặc ảnh raster (.jpg, .png) kèm metadata JSON log.
pub struct PdfExporter;

impl PdfExporter {
    /// Xuất tài liệu ra PDF và/hoặc ảnh chất lượng cao kèm ghi file metadata JSON
    pub fn export(
        img: &RgbImage,
        output_base_path: &Path,
        paper_size: PaperSize,
        scan_result: &mut ScanResult,
        config: &PipelineConfig,
    ) -> Result<String, String> {
        let (w, h) = img.dimensions();
        if w == 0 || h == 0 {
            return Err("Ảnh rỗng, không thể xuất PDF".to_string());
        }

        // Tạo thư mục đầu ra nếu chưa tồn tại
        if let Some(parent) = output_base_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Không thể tạo thư mục đầu ra: {}", e))?;
            }
        }

        let mut primary_output_path = String::new();

        // 1. Xuất file PDF nếu cấu hình yêu cầu
        if matches!(config.output_format, OutputFormat::Pdf | OutputFormat::Both) {
            let pdf_path = output_base_path.with_extension("pdf");
            let pdf_bytes = Self::create_pdf(img, paper_size, config.output_dpi)?;

            std::fs::write(&pdf_path, pdf_bytes)
                .map_err(|e| format!("Không thể lưu file PDF: {}", e))?;

            primary_output_path = pdf_path.to_string_lossy().to_string();
            scan_result.output_path = Some(primary_output_path.clone());
        }

        // 2. Xuất file ảnh raster nếu cấu hình yêu cầu
        if matches!(config.output_format, OutputFormat::Png | OutputFormat::Jpg | OutputFormat::Both) {
            let ext = match config.output_format {
                OutputFormat::Jpg => "jpg",
                _ => "png",
            };
            let img_path = output_base_path.with_extension(ext);
            img.save(&img_path)
                .map_err(|e| format!("Không thể lưu file ảnh: {}", e))?;

            if primary_output_path.is_empty() {
                primary_output_path = img_path.to_string_lossy().to_string();
                scan_result.output_path = Some(primary_output_path.clone());
            }
        }

        // 3. Ghi file JSON metadata đi kèm
        let json_path = output_base_path.with_extension("json");
        if let Ok(json_str) = serde_json::to_string_pretty(&scan_result) {
            let _ = std::fs::write(&json_path, json_str);
        }

        Ok(primary_output_path)
    }

    /// Đóng gói ảnh RGB vào vector PDF với DPI chính xác
    pub fn create_pdf(img: &RgbImage, paper_size: PaperSize, dpi: u32) -> Result<Vec<u8>, String> {
        let (w, h) = img.dimensions();
        let target_dpi = dpi.max(72) as f32;

        // Tính kích thước trang PDF theo điểm (1 pt = 1/72 inch)
        let (page_w_pt, page_h_pt) = match paper_size {
            PaperSize::A4 => (595.28f32, 841.89f32),
            PaperSize::Letter => (612.0f32, 792.0f32),
            PaperSize::Custom => ((w as f32 / target_dpi) * 72.0, (h as f32 / target_dpi) * 72.0),
        };

        // Nén ảnh sang JPEG chất lượng cao (quality 95)
        let mut jpeg_bytes = Vec::new();
        {
            let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, 95);
            encoder
                .encode(img.as_raw(), w, h, image::ExtendedColorType::Rgb8)
                .map_err(|e| format!("Lỗi mã hóa JPEG cho PDF: {}", e))?;
        }

        // Xây dựng tài liệu PDF qua pdf-writer
        let mut pdf = Pdf::new();

        let catalog_id = Ref::new(1);
        let pages_id = Ref::new(2);
        let page_id = Ref::new(3);
        let image_id = Ref::new(4);
        let content_id = Ref::new(5);
        let image_name = Name(b"Im0");

        // 1. Document Catalog
        pdf.catalog(catalog_id).pages(pages_id);

        // 2. Pages Tree
        pdf.pages(pages_id).kids([page_id]).count(1);

        // 3. Page Object
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, page_w_pt, page_h_pt));
        page.parent(pages_id);
        page.contents(content_id);
        page.resources().x_objects().pair(image_name, image_id);
        page.finish();

        // 4. Content stream: Vẽ ảnh khớp toàn trang
        let mut content = Content::new();
        content.save_state();
        content.transform([page_w_pt, 0.0, 0.0, page_h_pt, 0.0, 0.0]);
        content.x_object(image_name);
        content.restore_state();
        pdf.stream(content_id, &content.finish());

        // 5. Image XObject nhúng luồng JPEG với /DCTDecode
        let mut image_xobject = pdf.image_xobject(image_id, &jpeg_bytes);
        image_xobject.width(w as i32);
        image_xobject.height(h as i32);
        image_xobject.color_space().device_rgb();
        image_xobject.bits_per_component(8);
        image_xobject.filter(pdf_writer::Filter::DctDecode);
        image_xobject.finish();

        Ok(pdf.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_export_creation() {
        let img = RgbImage::new(100, 100);
        let pdf_data = PdfExporter::create_pdf(&img, PaperSize::A4, 300).expect("Tạo PDF thành công");

        assert!(pdf_data.len() > 100);
        // Header PDF luôn bắt đầu bằng %PDF-
        assert_eq!(&pdf_data[0..5], b"%PDF-");
    }
}
