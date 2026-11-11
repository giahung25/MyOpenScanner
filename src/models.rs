use nalgebra::Point2;
use serde::{Deserialize, Serialize};

/// CIE LAB color value
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Lab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

/// Thứ tự 4 góc: Top-Left, Top-Right, Bottom-Right, Bottom-Left
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DocumentCorners {
    pub tl: Point2<f32>,
    pub tr: Point2<f32>,
    pub br: Point2<f32>,
    pub bl: Point2<f32>,
}

impl DocumentCorners {
    pub fn new(tl: Point2<f32>, tr: Point2<f32>, br: Point2<f32>, bl: Point2<f32>) -> Self {
        Self { tl, tr, br, bl }
    }

    pub fn as_array(&self) -> [Point2<f32>; 4] {
        [self.tl, self.tr, self.br, self.bl]
    }

    pub fn to_json_array(&self) -> [[f32; 2]; 4] {
        [
            [self.tl.x, self.tl.y],
            [self.tr.x, self.tr.y],
            [self.br.x, self.br.y],
            [self.bl.x, self.bl.y],
        ]
    }

    /// Tính góc deskew trung bình của cạnh trên và cạnh dưới
    pub fn deskew_angle_deg(&self) -> f32 {
        let angle_top = (self.tr.y - self.tl.y).atan2(self.tr.x - self.tl.x).to_degrees();
        let angle_bottom = (self.br.y - self.bl.y).atan2(self.br.x - self.bl.x).to_degrees();
        ((angle_top + angle_bottom) / 2.0).round() as f32 / 100.0 * 100.0 // round to 2 decimal
    }

    /// Ước lượng kích thước tài liệu thực tế (width, height)
    pub fn estimate_size(&self) -> (f32, f32) {
        let width_top = (self.tr - self.tl).norm();
        let width_bottom = (self.br - self.bl).norm();
        let height_left = (self.bl - self.tl).norm();
        let height_right = (self.br - self.tr).norm();
        let max_w = width_top.max(width_bottom);
        let max_h = height_left.max(height_right);
        (max_w, max_h)
    }
}

/// Kết quả xử lý một ảnh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub file_name: String,
    pub status: ScanStatus,
    pub detected_corners: Option<DocumentCorners>,
    pub deskew_angle_deg: f32,
    pub confidence_score: f32,
    pub inferred_paper_size: PaperSize,
    pub target_dpi: u32,
    pub processing_time_ms: u64,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
}

/// Trạng thái xử lý
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Success,
    Fallback,
    Error,
}

/// Kích thước giấy chuẩn
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaperSize {
    A4,
    Letter,
    Custom,
}

impl PaperSize {
    pub fn dimensions_mm(&self) -> (f32, f32) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Custom => (0.0, 0.0),
        }
    }
}

/// Cấu hình pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub target_dim: u32,              // Resize target for detection (~800px)
    pub min_area_ratio: f32,          // Minimum contour area ratio
    pub snap_to_standard_aspect: bool, // Snap to A4/Letter aspect ratio
    pub target_luminance: u8,         // Target luminance for normalization
    pub clahe_clip_limit: f32,        // CLAHE clip limit
    pub unsharp_amount: f32,          // Unsharp mask amount
    pub unsharp_radius: f32,          // Unsharp mask radius (sigma)
    pub unsharp_threshold: u8,        // Unsharp threshold
    pub barcode_sharpen_strength: f32, // Barcode region sharpen strength
    pub output_dpi: u32,              // Output PDF DPI (200/300)
    pub output_format: OutputFormat,  // Output format
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            target_dim: 800,
            min_area_ratio: 0.12,
            snap_to_standard_aspect: true,
            target_luminance: 230,
            clahe_clip_limit: 1.5,
            unsharp_amount: 0.8,
            unsharp_radius: 1.2,
            unsharp_threshold: 8,
            barcode_sharpen_strength: 0.3,
            output_dpi: 300,
            output_format: OutputFormat::Pdf,
        }
    }
}

/// Định dạng đầu ra
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Pdf,
    Png,
    Jpg,
    Both,
}

/// Metadata mở rộng cho debug
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingMetadata {
    pub original_size: (u32, u32),
    pub rectified_size: (u32, u32),
    pub scale_factor: f32,
    pub homography_matrix: [[f32; 3]; 3],
    pub luminance_stats: LuminanceStats,
}

/// Thống kê độ sáng
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuminanceStats {
    pub mean_before: f32,
    pub mean_after: f32,
    pub std_dev_before: f32,
    pub std_dev_after: f32,
}