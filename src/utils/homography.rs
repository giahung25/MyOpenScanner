use nalgebra::{Matrix3, Point2, Vector3};
use rayon::prelude::*;
use crate::models::DocumentCorners;

/// Tính ma trận Homography 3x3 từ 4 cặp điểm tương ứng
/// src_pts: 4 điểm nguồn (trên ảnh gốc)
/// dst_pts: 4 điểm đích (trên ảnh rectified)
pub fn compute_homography(
    src_pts: &[Point2<f32>; 4],
    dst_pts: &[Point2<f32>; 4],
) -> Matrix3<f32> {
    // Xây dựng hệ phương trình Ah = 0
    // Mỗi cặp điểm cho 2 hàng, hàng thứ 9 để 0 để có ma trận vuông 9x9 cho SVD đầy đủ
    let mut a = nalgebra::DMatrix::zeros(9, 9);

    for i in 0..4 {
        let x = src_pts[i].x;
        let y = src_pts[i].y;
        let xp = dst_pts[i].x;
        let yp = dst_pts[i].y;

        // Hàng 2i: cho x'
        a[(2 * i, 0)] = -x;
        a[(2 * i, 1)] = -y;
        a[(2 * i, 2)] = -1.0;
        a[(2 * i, 6)] = x * xp;
        a[(2 * i, 7)] = y * xp;
        a[(2 * i, 8)] = xp;

        // Hàng 2i+1: cho y'
        a[(2 * i + 1, 3)] = -x;
        a[(2 * i + 1, 4)] = -y;
        a[(2 * i + 1, 5)] = -1.0;
        a[(2 * i + 1, 6)] = x * yp;
        a[(2 * i + 1, 7)] = y * yp;
        a[(2 * i + 1, 8)] = yp;
    }

    // SVD để tìm vector h (cột cuối của V, tức hàng cuối của V^T)
    let svd = a.svd(true, true);
    let v_t = svd.v_t.unwrap();
    let h = v_t.row(8).transpose();

    let mut mat = Matrix3::new(
        h[0], h[1], h[2],
        h[3], h[4], h[5],
        h[6], h[7], h[8],
    );

    // Chuẩn hóa sao cho h[8] = 1.0 nếu khả dĩ
    if mat[(2, 2)].abs() > 1e-7 {
        mat /= mat[(2, 2)];
    }

    mat
}

/// Tính ma trận Homography ngược (từ ảnh rectified về ảnh gốc)
pub fn compute_inverse_homography(
    src_pts: &[Point2<f32>; 4],
    dst_pts: &[Point2<f32>; 4],
) -> Matrix3<f32> {
    // Đảo ngược src và dst
    compute_homography(dst_pts, src_pts)
}

/// Warp ảnh bằng Inverse Mapping với Bilinear Interpolation (đa luồng hóa qua Rayon)
/// Hiệu quả hơn forward mapping vì không có lỗ hổng (holes)
pub fn warp_perspective_bilinear(
    src: &image::RgbImage,
    homography: &Matrix3<f32>,
    out_width: u32,
    out_height: u32,
) -> image::RgbImage {
    let (src_w, src_h) = src.dimensions();
    let src_w = src_w as f32;
    let src_h = src_h as f32;

    let mut dst = image::RgbImage::new(out_width, out_height);

    // Inverse homography: dst -> src
    let h_inv = homography.try_inverse().unwrap_or_else(Matrix3::identity);
    let row_stride = (out_width * 3) as usize;

    dst.par_chunks_exact_mut(row_stride)
        .enumerate()
        .for_each(|(y, row)| {
            let y_f = y as f32 + 0.5;
            for x in 0..out_width {
                let x_f = x as f32 + 0.5;
                let pt_dst = Vector3::new(x_f, y_f, 1.0);
                let pt_src_h = h_inv * pt_dst;

                let offset = (x * 3) as usize;
                if pt_src_h.z.abs() < 1e-8 {
                    row[offset] = 0;
                    row[offset + 1] = 0;
                    row[offset + 2] = 0;
                    continue;
                }

                let sx = pt_src_h.x / pt_src_h.z;
                let sy = pt_src_h.y / pt_src_h.z;

                let pixel = bilinear_sample(src, sx, sy, src_w, src_h);
                row[offset] = pixel[0];
                row[offset + 1] = pixel[1];
                row[offset + 2] = pixel[2];
            }
        });

    dst
}

/// Bilinear sampling từ ảnh nguồn
fn bilinear_sample(
    src: &image::RgbImage,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> image::Rgb<u8> {
    // Clamp to valid range với border replicate
    let x = x.clamp(0.0, w - 1.0);
    let y = y.clamp(0.0, h - 1.0);

    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(w as u32 - 1);
    let y1 = (y0 + 1).min(h as u32 - 1);

    let dx = x - x0 as f32;
    let dy = y - y0 as f32;

    let p00 = src.get_pixel(x0, y0);
    let p10 = src.get_pixel(x1, y0);
    let p01 = src.get_pixel(x0, y1);
    let p11 = src.get_pixel(x1, y1);

    let mut out = [0u8; 3];
    for c in 0..3 {
        let v00 = p00[c] as f32;
        let v10 = p10[c] as f32;
        let v01 = p01[c] as f32;
        let v11 = p11[c] as f32;

        let v0 = v00 * (1.0 - dx) + v10 * dx;
        let v1 = v01 * (1.0 - dx) + v11 * dx;
        let v = v0 * (1.0 - dy) + v1 * dy;

        out[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    image::Rgb(out)
}

/// Warp nhanh dùng imageproc (cho reference/testing)
#[cfg(feature = "imageproc-warp")]
pub fn warp_perspective_imageproc(
    src: &image::RgbImage,
    homography: &Matrix3<f32>,
    out_width: u32,
    out_height: u32,
) -> image::RgbImage {
    use imageproc::geometric_transformations::{warp, Interpolation, Projection};
    use image::ImageBuffer;

    let proj = Projection::from_matrix([
        homography[(0, 0)] as f64, homography[(0, 1)] as f64, homography[(0, 2)] as f64,
        homography[(1, 0)] as f64, homography[(1, 1)] as f64, homography[(1, 2)] as f64,
        homography[(2, 0)] as f64, homography[(2, 1)] as f64, homography[(2, 2)] as f64,
    ]);

    warp(src, &proj, Interpolation::Bilinear, image::Rgb([0, 0, 0]), out_width, out_height)
}

/// Tạo destination points cho rectified image
pub fn create_dest_points(width: u32, height: u32) -> [Point2<f32>; 4] {
    let w = width as f32;
    let h = height as f32;
    [
        Point2::new(0.0, 0.0),
        Point2::new(w - 1.0, 0.0),
        Point2::new(w - 1.0, h - 1.0),
        Point2::new(0.0, h - 1.0),
    ]
}

/// Chuyển DocumentCorners sang array of Point2
pub fn corners_to_points(corners: &DocumentCorners) -> [Point2<f32>; 4] {
    [corners.tl, corners.tr, corners.br, corners.bl]
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Point2;

    #[test]
    fn test_homography_identity() {
        let src = [
            Point2::new(0.0, 0.0),
            Point2::new(100.0, 0.0),
            Point2::new(100.0, 100.0),
            Point2::new(0.0, 100.0),
        ];
        let dst = src;

        let h = compute_homography(&src, &dst);
        // Should be close to identity
        assert!((h[(0, 0)] - 1.0).abs() < 0.01);
        assert!((h[(1, 1)] - 1.0).abs() < 0.01);
        assert!((h[(2, 2)] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_warp_identity() {
        let src = image::RgbImage::new(100, 100);
        let h = Matrix3::identity();
        let dst = warp_perspective_bilinear(&src, &h, 100, 100);
        assert_eq!(dst.dimensions(), (100, 100));
    }
}