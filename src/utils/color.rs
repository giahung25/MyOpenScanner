use crate::models::Lab;
use nalgebra::{Matrix3, Vector3};

/// Chuyển đổi sRGB (0-255) -> CIE LAB
/// Dùng D65 illuminant, 2-degree observer
pub fn srgb_to_lab(r: u8, g: u8, b: u8) -> Lab {
    // 1. sRGB -> Linear RGB (0-1)
    let r_lin = srgb_to_linear(r as f32 / 255.0);
    let g_lin = srgb_to_linear(g as f32 / 255.0);
    let b_lin = srgb_to_linear(b as f32 / 255.0);

    // 2. Linear RGB -> XYZ (D65) scaled to 0-100
    // Matrix from sRGB to XYZ (D65)
    let xyz = Matrix3::new(
        0.4124564, 0.3575761, 0.1804375,
        0.2126729, 0.7151522, 0.0721750,
        0.0193339, 0.1191920, 0.9503041,
    ) * Vector3::new(r_lin, g_lin, b_lin) * 100.0;

    // 3. XYZ -> LAB
    xyz_to_lab(xyz.x, xyz.y, xyz.z)
}

/// Chuyển đổi CIE LAB -> sRGB (0-255)
pub fn lab_to_srgb(l: f32, a: f32, b_: f32) -> (u8, u8, u8) {
    // 1. LAB -> XYZ
    let xyz = lab_to_xyz(l, a, b_);

    // 2. XYZ -> Linear RGB (xyz is on 0-100 scale, normalize to 0-1)
    let rgb_lin = Matrix3::new(
        3.2404542, -1.5371385, -0.4985314,
        -0.9692660, 1.8760108, 0.0415560,
        0.0556434, -0.2040259, 1.0572252,
    ) * Vector3::new(xyz.0 / 100.0, xyz.1 / 100.0, xyz.2 / 100.0);

    // 3. Linear RGB -> sRGB
    let r = linear_to_srgb(rgb_lin.x);
    let g = linear_to_srgb(rgb_lin.y);
    let b = linear_to_srgb(rgb_lin.z);

    (
        (r * 255.0).round().clamp(0.0, 255.0) as u8,
        (g * 255.0).round().clamp(0.0, 255.0) as u8,
        (b * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// sRGB component -> Linear RGB
#[inline]
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear RGB component -> sRGB
#[inline]
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// XYZ -> LAB (D65 reference white: Xn=95.047, Yn=100.0, Zn=108.883)
fn xyz_to_lab(x: f32, y: f32, z: f32) -> Lab {
    const XN: f32 = 95.047;
    const YN: f32 = 100.000;
    const ZN: f32 = 108.883;

    let fx = f_xyz(x / XN);
    let fy = f_xyz(y / YN);
    let fz = f_xyz(z / ZN);

    let l = (116.0 * fy - 16.0).max(0.0);
    let a = 500.0 * (fx - fy);
    let b = 200.0 * (fy - fz);

    Lab { l, a, b }
}

/// LAB -> XYZ
fn lab_to_xyz(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    const XN: f32 = 95.047;
    const YN: f32 = 100.000;
    const ZN: f32 = 108.883;

    let fy = (l + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;

    let x = XN * f_xyz_inv(fx);
    let y = YN * f_xyz_inv(fy);
    let z = ZN * f_xyz_inv(fz);

    (x, y, z)
}

/// Helper function cho XYZ -> LAB
#[inline]
fn f_xyz(t: f32) -> f32 {
    const EPSILON: f32 = 216.0 / 24389.0; // 6/29^3
    const KAPPA: f32 = 24389.0 / 27.0;    // 29^3/3^3

    if t > EPSILON {
        t.cbrt()
    } else {
        (KAPPA * t + 16.0) / 116.0
    }
}

/// Helper function cho LAB -> XYZ
#[inline]
fn f_xyz_inv(t: f32) -> f32 {
    const EPSILON: f32 = 216.0 / 24389.0;
    const KAPPA: f32 = 24389.0 / 27.0;

    let t3 = t * t * t;
    if t3 > EPSILON {
        t3
    } else {
        (116.0 * t - 16.0) / KAPPA
    }
}

/// Chuyển đổi toàn bộ ảnh RgbImage sang LAB (trả về 3 kênh riêng biệt)
pub fn rgb_image_to_lab_channels(
    img: &image::RgbImage,
) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let (w, h) = img.dimensions();
    let mut l_channel = vec![0.0; (w * h) as usize];
    let mut a_channel = vec![0.0; (w * h) as usize];
    let mut b_channel = vec![0.0; (w * h) as usize];

    for (i, pixel) in img.pixels().enumerate() {
        let Lab { l, a, b } = srgb_to_lab(pixel[0], pixel[1], pixel[2]);
        l_channel[i] = l;
        a_channel[i] = a;
        b_channel[i] = b;
    }

    (l_channel, a_channel, b_channel)
}

/// Chuyển đổi 3 kênh LAB -> RgbImage
pub fn lab_channels_to_rgb_image(
    w: u32,
    h: u32,
    l_channel: &[f32],
    a_channel: &[f32],
    b_channel: &[f32],
) -> image::RgbImage {
    let mut img = image::RgbImage::new(w, h);
    for (i, pixel) in img.pixels_mut().enumerate() {
        let (r, g, b) = lab_to_srgb(l_channel[i], a_channel[i], b_channel[i]);
        pixel[0] = r;
        pixel[1] = g;
        pixel[2] = b;
    }
    img
}

/// Fast Box Blur (Integral Image) cho L channel
pub fn fast_box_blur(channel: &[f32], w: u32, h: u32, radius: u32) -> Vec<f32> {
    let w = w as usize;
    let h = h as usize;
    let r = radius as usize;
    let diam = 2 * r + 1;
    let area = (diam * diam) as f32;

    // Integral image
    let mut integral = vec![0.0f32; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row_sum = 0.0;
        for x in 0..w {
            row_sum += channel[y * w + x];
            integral[(y + 1) * (w + 1) + (x + 1)] = integral[y * (w + 1) + (x + 1)] + row_sum;
        }
    }

    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let y1 = y.saturating_sub(r);
        let y2 = (y + r + 1).min(h);
        for x in 0..w {
            let x1 = x.saturating_sub(r);
            let x2 = (x + r + 1).min(w);

            let sum = integral[y2 * (w + 1) + x2]
                - integral[y1 * (w + 1) + x2]
                - integral[y2 * (w + 1) + x1]
                + integral[y1 * (w + 1) + x1];

            out[y * w + x] = sum / area;
        }
    }
    out
}

/// Bilateral Filter cho L channel (giữ biên, làm mượt vùng phẳng)
pub fn bilateral_filter(
    channel: &[f32],
    w: u32,
    h: u32,
    spatial_sigma: f32,
    range_sigma: f32,
) -> Vec<f32> {
    let w = w as usize;
    let h = h as usize;
    let radius = (2.0 * spatial_sigma).ceil() as usize;

    // Pre-compute spatial Gaussian
    let mut spatial_weights = vec![0.0f32; (2 * radius + 1) * (2 * radius + 1)];
    for dy in 0..=2 * radius {
        for dx in 0..=2 * radius {
            let d2 = ((dx as isize - radius as isize).pow(2) + (dy as isize - radius as isize).pow(2)) as f32;
            spatial_weights[dy * (2 * radius + 1) + dx] = (-d2 / (2.0 * spatial_sigma * spatial_sigma)).exp();
        }
    }

    let mut out = vec![0.0f32; w * h];
    let range_sigma2 = 2.0 * range_sigma * range_sigma;

    for y in 0..h {
        for x in 0..w {
            let center_val = channel[y * w + x];
            let mut sum = 0.0f32;
            let mut weight_sum = 0.0f32;

            let y_min = y.saturating_sub(radius);
            let y_max = (y + radius + 1).min(h);
            let x_min = x.saturating_sub(radius);
            let x_max = (x + radius + 1).min(w);

            for ny in y_min..y_max {
                for nx in x_min..x_max {
                    let dy = ny - y + radius;
                    let dx = nx - x + radius;
                    let spatial_w = spatial_weights[dy * (2 * radius + 1) + dx];

                    let diff = channel[ny * w + nx] - center_val;
                    let range_w = (-diff * diff / range_sigma2).exp();

                    let w_total = spatial_w * range_w;
                    sum += w_total * channel[ny * w + nx];
                    weight_sum += w_total;
                }
            }
            out[y * w + x] = sum / weight_sum.max(1e-6);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srgb_lab_roundtrip() {
        // Test white
        let (r, g, b) = lab_to_srgb(100.0, 0.0, 0.0);
        assert!((r as i32 - 255).abs() <= 2);
        assert!((g as i32 - 255).abs() <= 2);
        assert!((b as i32 - 255).abs() <= 2);

        // Test red
        let lab = srgb_to_lab(255, 0, 0);
        assert!(lab.l > 50.0);
        assert!(lab.a > 50.0);

        // Test blue
        let lab = srgb_to_lab(0, 0, 255);
        assert!(lab.l > 20.0);
        assert!(lab.b < -50.0);
    }
}