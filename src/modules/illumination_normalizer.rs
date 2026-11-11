use image::RgbImage;
use rayon::prelude::*;

use crate::models::PipelineConfig;
use crate::utils::color::{lab_channels_to_rgb_image, rgb_image_to_lab_channels};

/// Module 3: Illumination Normalizer
/// Cân bằng sáng toàn diện trên không gian màu CIE LAB:
/// - Ước lượng bản đồ nền (Illumination Map) trên kênh L* thông qua multi-scale background estimation.
/// - Triệt tiêu bóng râm cục bộ và làm đều sáng toàn trang.
/// - BẢO TỒN NGUYÊN VẸN hai kênh sắc độ a* và b*:
///   + Mộc đỏ (a* >> 0) và chữ ký xanh (b* << 0) không bị bay màu.
///   + Giấy liên màu (vàng, hồng, xanh) giữ nguyên sắc độ tự nhiên, không bị tẩy trắng.
pub struct IlluminationNormalizer;

impl IlluminationNormalizer {
    /// Cân bằng sáng ảnh RGB theo cấu hình
    pub fn normalize(img: &RgbImage, config: &PipelineConfig) -> RgbImage {
        let (w, h) = img.dimensions();
        if w == 0 || h == 0 {
            return img.clone();
        }

        // 1. Chuyển sang không gian màu CIE LAB (L: 0-100, a: -128..127, b: -128..127)
        let (l_channel, a_channel, b_channel) = rgb_image_to_lab_channels(img);

        // 2. Ước lượng bản đồ nền (Background Map) trên kênh L* bằng multi-scale low-pass filter
        let bg_map = Self::estimate_background_luma(&l_channel, w, h);

        // 3. Chuẩn hóa kênh L* theo tỷ lệ nền: L_norm = (L / L_bg) * L_target
        let target_l = (config.target_luminance as f32 / 255.0) * 100.0;
        let mut l_norm = vec![0.0f32; (w * h) as usize];

        l_norm
            .par_chunks_mut(w as usize)
            .enumerate()
            .for_each(|(y, row)| {
                let row_offset = y * w as usize;
                for x in 0..w as usize {
                    let idx = row_offset + x;
                    let l_val = l_channel[idx];
                    let bg_val = bg_map[idx].max(10.0); // Tránh chia cho 0 hoặc vùng quá tối

                    // Bù trừ độ sáng: Nâng vùng tối lên tương ứng với độ suy hao của nền
                    let ratio = (l_val / bg_val).min(1.2);
                    let mut compensated = ratio * target_l;

                    // Nếu pixel gốc thuộc về text/stroke đậm màu (l_val thấp), giữ độ tương phản cao
                    if l_val < bg_val * 0.65 {
                        compensated = l_val * (target_l / bg_val.max(50.0));
                    }

                    row[x] = compensated.clamp(0.0, 100.0);
                }
            });

        // 4. Giữ nguyên hoàn toàn hai kênh a* và b* để bảo tồn chữ ký xanh, mộc đỏ và màu giấy liên
        lab_channels_to_rgb_image(w, h, &l_norm, &a_channel, &b_channel)
    }

    /// Ước lượng bản đồ chiếu sáng nền L* sử dụng thumbnail downscale + large-kernel blur + bilinear upsample
    /// Kỹ thuật này tối ưu hiệu năng (< 5ms) và tự động lọc bỏ chữ in/nét bút mảnh.
    fn estimate_background_luma(l_channel: &[f32], w: u32, h: u32) -> Vec<f32> {
        let max_dim = 200u32;
        let scale = if w.max(h) > max_dim {
            max_dim as f32 / w.max(h) as f32
        } else {
            1.0
        };

        let thumb_w = ((w as f32 * scale).round() as u32).max(1);
        let thumb_h = ((h as f32 * scale).round() as u32).max(1);

        // Downsample L* về thumbnail bằng area averaging
        let mut thumb = vec![0.0f32; (thumb_w * thumb_h) as usize];
        let x_ratio = w as f32 / thumb_w as f32;
        let y_ratio = h as f32 / thumb_h as f32;

        for ty in 0..thumb_h {
            let sy_start = (ty as f32 * y_ratio) as usize;
            let sy_end = (((ty + 1) as f32 * y_ratio) as usize).min(h as usize);
            for tx in 0..thumb_w {
                let sx_start = (tx as f32 * x_ratio) as usize;
                let sx_end = (((tx + 1) as f32 * x_ratio) as usize).min(w as usize);

                let mut sum = 0.0f32;
                let mut count = 0usize;
                for y in sy_start..sy_end {
                    for x in sx_start..sx_end {
                        sum += l_channel[y * w as usize + x];
                        count += 1;
                    }
                }
                thumb[(ty * thumb_w + tx) as usize] = if count > 0 { sum / count as f32 } else { 50.0 };
            }
        }

        // Lọc làm mượt mạnh trên thumbnail để loại bỏ biến thiên tần số cao
        let blur_radius = ((thumb_w.min(thumb_h) as f32 * 0.15).round() as u32).max(3);
        let blurred_thumb = crate::utils::color::fast_box_blur(&thumb, thumb_w, thumb_h, blur_radius);

        // Upsample bilinear trở lại kích thước gốc
        let mut bg_map = vec![0.0f32; (w * h) as usize];
        let inv_x_ratio = (thumb_w - 1) as f32 / (w - 1).max(1) as f32;
        let inv_y_ratio = (thumb_h - 1) as f32 / (h - 1).max(1) as f32;

        bg_map
            .par_chunks_mut(w as usize)
            .enumerate()
            .for_each(|(y, row)| {
                let gy = y as f32 * inv_y_ratio;
                let y0 = gy.floor() as usize;
                let y1 = (y0 + 1).min(thumb_h as usize - 1);
                let dy = gy - y0 as f32;

                for x in 0..w as usize {
                    let gx = x as f32 * inv_x_ratio;
                    let x0 = gx.floor() as usize;
                    let x1 = (x0 + 1).min(thumb_w as usize - 1);
                    let dx = gx - x0 as f32;

                    let c00 = blurred_thumb[y0 * thumb_w as usize + x0];
                    let c10 = blurred_thumb[y0 * thumb_w as usize + x1];
                    let c01 = blurred_thumb[y1 * thumb_w as usize + x0];
                    let c11 = blurred_thumb[y1 * thumb_w as usize + x1];

                    let top = c00 * (1.0 - dx) + c10 * dx;
                    let bottom = c01 * (1.0 - dx) + c11 * dx;
                    row[x] = top * (1.0 - dy) + bottom * dy;
                }
            });

        bg_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn test_illumination_normalizer_empty() {
        let img = RgbImage::new(0, 0);
        let config = PipelineConfig::default();
        let res = IlluminationNormalizer::normalize(&img, &config);
        assert_eq!(res.width(), 0);
    }

    #[test]
    fn test_preserves_red_stamp_and_blue_ink() {
        let mut img = RgbImage::new(50, 50);

        // Điểm 1: Mộc đỏ (Red stamp: R=220, G=30, B=30)
        img.put_pixel(10, 10, Rgb([220, 30, 30]));

        // Điểm 2: Chữ ký xanh (Blue ink: R=20, G=50, B=210)
        img.put_pixel(25, 25, Rgb([20, 50, 210]));

        // Nền giấy trắng có bóng râm (R=140, G=140, B=140)
        for y in 0..50 {
            for x in 0..50 {
                if (x, y) != (10, 10) && (x, y) != (25, 25) {
                    img.put_pixel(x, y, Rgb([140, 140, 140]));
                }
            }
        }

        let config = PipelineConfig::default();
        let res = IlluminationNormalizer::normalize(&img, &config);

        // Kiểm tra điểm mộc đỏ: Kênh R vẫn phải vượt trội G và B
        let red_pixel = res.get_pixel(10, 10);
        assert!(
            red_pixel[0] > red_pixel[1] + 80,
            "Mộc đỏ phải giữ nguyên màu đỏ tươi: {:?}",
            red_pixel
        );

        // Kiểm tra điểm chữ ký xanh: Kênh B vẫn phải vượt trội R và G
        let blue_pixel = res.get_pixel(25, 25);
        assert!(
            blue_pixel[2] > blue_pixel[0] + 80,
            "Chữ ký xanh phải giữ nguyên màu xanh tươi: {:?}",
            blue_pixel
        );
    }
}
