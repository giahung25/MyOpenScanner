use image::RgbImage;
use rayon::prelude::*;

use crate::models::PipelineConfig;
use crate::utils::color::{fast_box_blur, lab_channels_to_rgb_image, rgb_image_to_lab_channels};

/// Module 4: Detail Enhancer
/// Làm nét chữ mảnh (6pt–8pt), nét bút ký tay và các vạch Barcode 1D/2D:
/// - Áp dụng Thresholded Unsharp Masking trên kênh L* (Luminance).
/// - Triệt tiêu viền hào quang (halo artifacts) bằng cách chỉ kích hoạt sharpening tại dải tương phản thực.
/// - Không khuếch đại hạt nhiễu (noise grain) trên các mảng nền phẳng.
pub struct DetailEnhancer;

impl DetailEnhancer {
    /// Tăng cường độ nét của tài liệu
    pub fn enhance(img: &RgbImage, config: &PipelineConfig) -> RgbImage {
        let (w, h) = img.dimensions();
        if w == 0 || h == 0 {
            return img.clone();
        }

        // 1. Tách các kênh CIE LAB
        let (l_channel, a_channel, b_channel) = rgb_image_to_lab_channels(img);

        // 2. Tính ảnh làm mờ L_blur bằng fast box blur (bán kính nhỏ ~1-2px tương ứng unsharp_radius)
        let radius = config.unsharp_radius.round().max(1.0) as u32;
        let l_blur = fast_box_blur(&l_channel, w, h, radius);

        // 3. Thresholded Unsharp Masking
        let threshold = config.unsharp_threshold as f32; // Ngưỡng bỏ qua nhiễu nền
        let amount = config.unsharp_amount;              // Cường độ tăng nét (0.6 - 1.2)
        let barcode_boost = config.barcode_sharpen_strength;

        let mut l_enhanced = vec![0.0f32; (w * h) as usize];

        l_enhanced
            .par_chunks_mut(w as usize)
            .enumerate()
            .for_each(|(y, row)| {
                let row_offset = y * w as usize;
                for x in 0..w as usize {
                    let idx = row_offset + x;
                    let original = l_channel[idx];
                    let blurred = l_blur[idx];
                    let diff = original - blurred;

                    if diff.abs() > threshold {
                        // Nét cạnh vượt ngưỡng: Tính hệ số làm nét
                        // Nếu là viền mực đen/nét mảnh (diff < 0), bổ sung barcode_boost để tách bạch nét
                        let effective_amount = if diff < 0.0 {
                            amount + barcode_boost
                        } else {
                            // Viền sáng xung quanh chữ: Giảm nhẹ cường độ để triệt tiêu halo ringing
                            amount * 0.75
                        };

                        let sharpened = original + effective_amount * diff;
                        row[x] = sharpened.clamp(0.0, 100.0);
                    } else {
                        // Vùng nền phẳng: Giữ nguyên để tránh sinh hạt nhiễu
                        row[x] = original;
                    }
                }
            });

        // 4. Tái hợp các kênh và chuyển về RgbImage
        lab_channels_to_rgb_image(w, h, &l_enhanced, &a_channel, &b_channel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn test_detail_enhancer_empty() {
        let img = RgbImage::new(0, 0);
        let config = PipelineConfig::default();
        let res = DetailEnhancer::enhance(&img, &config);
        assert_eq!(res.width(), 0);
    }

    #[test]
    fn test_detail_enhancer_sharpens_thin_stroke() {
        let mut img = RgbImage::new(30, 30);
        // Nền trắng
        for p in img.pixels_mut() {
            *p = Rgb([240, 240, 240]);
        }

        // Vẽ 1 vạch đen mảnh ở giữa (giả lập nét chữ hoặc vạch barcode)
        for y in 5..25 {
            img.put_pixel(15, y, Rgb([30, 30, 30]));
        }

        let config = PipelineConfig::default();
        let res = DetailEnhancer::enhance(&img, &config);

        // Vạch đen sau khi enhance phải giữ hoặc tăng độ tương phản (giá trị tối hơn hoặc bằng gốc)
        let original_val = img.get_pixel(15, 10)[0];
        let enhanced_val = res.get_pixel(15, 10)[0];
        assert!(
            enhanced_val <= original_val + 2,
            "Nét chữ phải được giữ độ tương phản sắc nét: {} vs {}",
            enhanced_val,
            original_val
        );
    }
}
