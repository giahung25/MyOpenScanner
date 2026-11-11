use image::{GrayImage, RgbImage};
use nalgebra::Matrix3;

use crate::models::{DocumentCorners, PaperSize, PipelineConfig};
use crate::utils::homography::{compute_homography, create_dest_points, warp_perspective_bilinear};

/// Module 2: Geometric Rectifier
/// Cắt biên, Nắn phối cảnh (Homography Warp) từ góc chụp nghiêng lên đến 35 độ
/// về mặt phẳng thẳng đứng 90 độ, chuẩn hóa tỷ lệ khổ giấy và tinh chỉnh độ nghiêng vi mô (Fine Deskew).
#[derive(Debug, Clone)]
pub struct GeometricRectifier {
    pub snap_to_standard_aspect: bool,
    pub fine_deskew: bool,
}

impl GeometricRectifier {
    pub fn new(snap_to_standard_aspect: bool, fine_deskew: bool) -> Self {
        Self {
            snap_to_standard_aspect,
            fine_deskew,
        }
    }

    pub fn from_config(config: &PipelineConfig) -> Self {
        Self {
            snap_to_standard_aspect: config.snap_to_standard_aspect,
            fine_deskew: true,
        }
    }

    /// Nắn phối cảnh từ 4 góc tài liệu về ảnh phẳng chuẩn tỷ lệ.
    /// Trả về: (Ảnh đã nắn thẳng, Khổ giấy nhận diện được, Ma trận Homography 3x3)
    pub fn rectify(
        &self,
        image: &RgbImage,
        corners: &DocumentCorners,
    ) -> (RgbImage, PaperSize, Matrix3<f32>) {
        let tl = corners.tl;
        let tr = corners.tr;
        let br = corners.br;
        let bl = corners.bl;

        // 1. Tính toán kích thước chiều rộng và chiều cao tối đa theo khoảng cách Euclidean
        let width_top = (tr - tl).norm();
        let width_bottom = (br - bl).norm();
        let mut max_w = width_top.max(width_bottom);

        let height_left = (bl - tl).norm();
        let height_right = (br - tr).norm();
        let mut max_h = height_left.max(height_right);

        if max_w < 10.0 || max_h < 10.0 {
            return (image.clone(), PaperSize::Custom, Matrix3::identity());
        }

        // 2. Chuẩn hóa tỷ lệ khung hình theo tiêu chuẩn tài liệu thực tế (A4 hoặc Letter)
        let mut paper_size = PaperSize::Custom;
        if self.snap_to_standard_aspect {
            let ratio = max_h / max_w;
            if ratio >= 1.0 {
                // Khổ dọc (Portrait)
                // Chuẩn A4: 1:1.4142 (dung sai +/- 6%)
                if (1.33..=1.50).contains(&ratio) {
                    max_h = max_w * 1.4142135;
                    paper_size = PaperSize::A4;
                } else if (1.22..1.33).contains(&ratio) {
                    // Chuẩn US Letter: 1:1.2941 (dung sai +/- 5%)
                    max_h = max_w * 1.2941176;
                    paper_size = PaperSize::Letter;
                }
            } else {
                // Khổ ngang (Landscape)
                let inv_ratio = max_w / max_h;
                if (1.33..=1.50).contains(&inv_ratio) {
                    max_w = max_h * 1.4142135;
                    paper_size = PaperSize::A4;
                } else if (1.22..1.33).contains(&inv_ratio) {
                    max_w = max_h * 1.2941176;
                    paper_size = PaperSize::Letter;
                }
            }
        }

        let out_w = (max_w.round() as u32).max(10);
        let out_h = (max_h.round() as u32).max(10);

        // 3. Tọa độ phẳng đích (Destination points)
        let dst_pts = create_dest_points(out_w, out_h);
        let src_pts = [tl, tr, br, bl];

        // 4. Tính toán ma trận biến đổi phối cảnh (Homography 3x3)
        let homography = compute_homography(&src_pts, &dst_pts);

        // 5. Warp ảnh với nội suy song tuyến (Bilinear) chất lượng cao
        let mut warped = warp_perspective_bilinear(image, &homography, out_w, out_h);

        // 6. Tinh chỉnh độ nghiêng vi mô (Fine-tuning deskew) nếu được kích hoạt
        if self.fine_deskew {
            warped = self.apply_fine_deskew(&warped);
        }

        (warped, paper_size, homography)
    }

    /// Khử độ lệch nghiêng dòng chữ/bảng biểu còn sót lại sau khi nắn phối cảnh
    /// dựa trên phương pháp Projection Profile Variance trên ảnh thu nhỏ.
    pub fn apply_fine_deskew(&self, image: &RgbImage) -> RgbImage {
        let (w, h) = image.dimensions();
        if w < 50 || h < 50 {
            return image.clone();
        }

        // Tạo thumbnail xám để tính toán cực nhanh (~1-2ms)
        let thumb_dim = 400u32;
        let scale = thumb_dim as f32 / w.max(h) as f32;
        let tw = ((w as f32 * scale).round() as u32).max(20);
        let th = ((h as f32 * scale).round() as u32).max(20);

        let thumb = image::imageops::resize(
            image,
            tw,
            th,
            image::imageops::FilterType::Nearest,
        );
        let gray_thumb = image::imageops::grayscale(&thumb);

        // Tìm góc xoay trong dải [-3.0, +3.0] độ có phương sai profile dòng lớn nhất
        let mut best_angle = 0.0f32;
        let mut max_variance = calculate_projection_variance(&gray_thumb, 0.0);

        let mut angle = -3.0f32;
        while angle <= 3.0f32 {
            if angle.abs() > 0.1 {
                let var = calculate_projection_variance(&gray_thumb, angle);
                if var > max_variance {
                    max_variance = var;
                    best_angle = angle;
                }
            }
            angle += 0.25f32;
        }

        // Nếu góc tối ưu đủ lớn (|angle| >= 0.25 độ), xoay bù ảnh gốc
        if best_angle.abs() >= 0.25 {
            rotate_bilinear(image, -best_angle)
        } else {
            image.clone()
        }
    }
}

/// Tính phương sai của tổng cường độ điểm ảnh theo từng dòng (Projection Profile) khi xoay một góc angle
fn calculate_projection_variance(gray: &GrayImage, angle_deg: f32) -> f32 {
    let (w, h) = gray.dimensions();
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let rad = angle_deg.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let mut row_sums = vec![0.0f32; h as usize];

    for y in 0..h {
        let mut sum = 0.0f32;
        let dy = y as f32 - cy;
        for x in 0..w {
            let dx = x as f32 - cx;

            // Tọa độ gốc xoay ngược lại
            let src_x = (cx + dx * cos_a - dy * sin_a).round() as i32;
            let src_y = (cy + dx * sin_a + dy * cos_a).round() as i32;

            if src_x >= 0 && src_x < w as i32 && src_y >= 0 && src_y < h as i32 {
                let val = gray.get_pixel(src_x as u32, src_y as u32)[0] as f32;
                sum += val;
            }
        }
        row_sums[y as usize] = sum;
    }

    // Tính phương sai của row_sums
    let mean = row_sums.iter().sum::<f32>() / h as f32;
    let var = row_sums.iter().map(|&s| (s - mean).powi(2)).sum::<f32>() / h as f32;
    var
}

/// Xoay ảnh quanh tâm một góc angle_deg sử dụng nội suy Bilinear
fn rotate_bilinear(src: &RgbImage, angle_deg: f32) -> RgbImage {
    use rayon::prelude::*;

    let (w, h) = src.dimensions();
    let mut dst = RgbImage::new(w, h);
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let rad = angle_deg.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let row_stride = (w * 3) as usize;
    let max_x = (w - 1) as f32;
    let max_y = (h - 1) as f32;

    dst.par_chunks_exact_mut(row_stride)
        .enumerate()
        .for_each(|(y, row)| {
            let dy = y as f32 + 0.5 - cy;
            for x in 0..w {
                let dx = x as f32 + 0.5 - cx;

                // Tọa độ nguồn
                let sx = cx + dx * cos_a + dy * sin_a;
                let sy = cy - dx * sin_a + dy * cos_a;

                let offset = (x * 3) as usize;
                if sx < 0.0 || sx > max_x || sy < 0.0 || sy > max_y {
                    // Ngoài viền: màu trắng (nền giấy)
                    row[offset] = 255;
                    row[offset + 1] = 255;
                    row[offset + 2] = 255;
                } else {
                    let x0 = sx.floor() as u32;
                    let y0 = sy.floor() as u32;
                    let x1 = (x0 + 1).min(w - 1);
                    let y1 = (y0 + 1).min(h - 1);

                    let fx = sx - x0 as f32;
                    let fy = sy - y0 as f32;

                    let p00 = src.get_pixel(x0, y0);
                    let p10 = src.get_pixel(x1, y0);
                    let p01 = src.get_pixel(x0, y1);
                    let p11 = src.get_pixel(x1, y1);

                    for c in 0..3 {
                        let v0 = p00[c] as f32 * (1.0 - fx) + p10[c] as f32 * fx;
                        let v1 = p01[c] as f32 * (1.0 - fx) + p11[c] as f32 * fx;
                        let v = v0 * (1.0 - fy) + v1 * fy;
                        row[offset + c] = v.round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        });

    dst
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;
    use nalgebra::Point2;

    #[test]
    fn test_geometric_rectifier_snap_a4() {
        // Tỷ lệ gần A4 (chiều cao ~ 1.40 chiều rộng)
        let corners = DocumentCorners::new(
            Point2::new(10.0, 10.0),
            Point2::new(210.0, 10.0),   // width = 200
            Point2::new(210.0, 290.0),  // height = 280 (ratio = 280/200 = 1.40)
            Point2::new(10.0, 290.0),
        );

        let img = RgbImage::from_pixel(300, 350, Rgb([200, 200, 200]));
        let rectifier = GeometricRectifier::new(true, false);

        let (rectified, paper_size, _h) = rectifier.rectify(&img, &corners);

        assert_eq!(paper_size, PaperSize::A4);
        assert_eq!(rectified.width(), 200);
        // max_h sau khi snap A4 phải là 200 * 1.4142135 = 283
        assert_eq!(rectified.height(), 283);
    }

    #[test]
    fn test_geometric_rectifier_snap_letter() {
        // Tỷ lệ gần Letter (chiều cao ~ 1.28 chiều rộng)
        let corners = DocumentCorners::new(
            Point2::new(0.0, 0.0),
            Point2::new(200.0, 0.0),   // width = 200
            Point2::new(200.0, 256.0),  // height = 256 (ratio = 256/200 = 1.28)
            Point2::new(0.0, 256.0),
        );

        let img = RgbImage::from_pixel(300, 350, Rgb([200, 200, 200]));
        let rectifier = GeometricRectifier::new(true, false);

        let (rectified, paper_size, _h) = rectifier.rectify(&img, &corners);

        assert_eq!(paper_size, PaperSize::Letter);
        assert_eq!(rectified.width(), 200);
        // max_h sau khi snap Letter phải là 200 * 1.2941176 = 259
        assert_eq!(rectified.height(), 259);
    }

    #[test]
    fn test_geometric_rectifier_custom() {
        // Tỷ lệ vuông (ratio = 1.0) không snap A4 hay Letter
        let corners = DocumentCorners::new(
            Point2::new(0.0, 0.0),
            Point2::new(200.0, 0.0),
            Point2::new(200.0, 200.0),
            Point2::new(0.0, 200.0),
        );

        let img = RgbImage::from_pixel(300, 300, Rgb([200, 200, 200]));
        let rectifier = GeometricRectifier::new(true, false);

        let (rectified, paper_size, _h) = rectifier.rectify(&img, &corners);

        assert_eq!(paper_size, PaperSize::Custom);
        assert_eq!(rectified.width(), 200);
        assert_eq!(rectified.height(), 200);
    }

    #[test]
    fn test_fine_deskew_no_crash() {
        let img = RgbImage::from_pixel(100, 100, Rgb([255, 255, 255]));
        let rectifier = GeometricRectifier::new(false, true);
        let deskewed = rectifier.apply_fine_deskew(&img);
        assert_eq!(deskewed.dimensions(), (100, 100));
    }
}
