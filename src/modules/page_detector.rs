use image::{GrayImage, Luma, RgbImage};
use imageproc::contours::find_contours;
use imageproc::distance_transform::Norm;
use imageproc::edges::canny;
use imageproc::filter::gaussian_blur_f32;
use imageproc::morphology::{dilate, erode};
use nalgebra::Point2;

use crate::models::{DocumentCorners, PipelineConfig, ScanStatus};

/// Module 1: Page Detector
/// Nhận diện trang tài liệu, tách nền và bắt 4 góc [TL, TR, BR, BL] từ ảnh chụp.
/// Kết hợp đa kỹ thuật:
/// - Pass 1: Otsu Binary Thresholding + Morphological Closing (tách giấy/bàn tối ưu)
/// - Pass 2: Adaptive Thresholding (bù trừ bóng đổ mạnh)
/// - Pass 3: Canny Edge Detection (cho tài liệu viền kẻ mảnh/độ tương phản thấp)
/// - Fallback hierarchy: Convex Hull -> Min Area Rect -> Safe image bounds
#[derive(Debug, Clone)]
pub struct PageDetector {
    pub target_dim: u32,
    pub min_area_ratio: f32,
}

impl PageDetector {
    pub fn new(target_dim: u32, min_area_ratio: f32) -> Self {
        Self {
            target_dim,
            min_area_ratio,
        }
    }

    pub fn from_config(config: &PipelineConfig) -> Self {
        Self {
            target_dim: config.target_dim,
            min_area_ratio: config.min_area_ratio,
        }
    }

    /// Phát hiện 4 góc tài liệu từ ảnh RGB gốc
    /// Trả về: (DocumentCorners, confidence_score, ScanStatus)
    pub fn detect(&self, image: &RgbImage) -> (DocumentCorners, f32, ScanStatus) {
        let (orig_w, orig_h) = image.dimensions();
        if orig_w < 10 || orig_h < 10 {
            let default_corners = self.default_bounds(orig_w, orig_h);
            return (default_corners, 0.1, ScanStatus::Fallback);
        }

        // 1. Multi-scale resize về thumbnail để tăng tốc độ (< 5ms) và khử nhiễu vi mô
        let max_dim = orig_w.max(orig_h);
        let scale = if max_dim > self.target_dim {
            self.target_dim as f32 / max_dim as f32
        } else {
            1.0
        };

        let proc_w = ((orig_w as f32 * scale).round() as u32).max(10);
        let proc_h = ((orig_h as f32 * scale).round() as u32).max(10);

        let proc_img = if scale < 1.0 {
            image::imageops::resize(
                image,
                proc_w,
                proc_h,
                image::imageops::FilterType::Triangle,
            )
        } else {
            image.clone()
        };

        // 2. Grayscale & Gaussian blur
        let gray = image::imageops::grayscale(&proc_img);
        let blurred = gaussian_blur_f32(&gray, 1.2);
        let total_pixels = (proc_w * proc_h) as f32;
        let min_area = total_pixels * self.min_area_ratio;

        // Tính ngưỡng Otsu tự động
        let otsu_thresh = calculate_otsu_threshold(&blurred);

        // --- PASS 1: OTSU BINARY THRESHOLDING (Tách giấy sáng trên mặt bàn tối) ---
        {
            let mut binary = GrayImage::new(proc_w, proc_h);
            for (x, y, pixel) in blurred.enumerate_pixels() {
                let v = if pixel[0] >= otsu_thresh { 255 } else { 0 };
                binary.put_pixel(x, y, Luma([v]));
            }

            // Morphological closing để liên kết các nét chữ/vết gãy thành khối giấy liền
            let dilated = dilate(&binary, Norm::L1, 4);
            let closed = erode(&dilated, Norm::L1, 4);

            if let Some(corners) = self.find_quad_from_binary(&closed, proc_w, proc_h, min_area) {
                let scaled = scale_corners_to_original(&corners, scale, orig_w, orig_h);
                return (scaled, 0.98, ScanStatus::Success);
            }
        }

        // --- PASS 2: ADAPTIVE THRESHOLDING (Bù trừ bóng râm gradient mạnh) ---
        {
            let adaptive_mask = calculate_adaptive_mask(&blurred, 15, 6);
            let dilated = dilate(&adaptive_mask, Norm::L1, 3);
            let closed = erode(&dilated, Norm::L1, 3);

            if let Some(corners) = self.find_quad_from_binary(&closed, proc_w, proc_h, min_area) {
                let scaled = scale_corners_to_original(&corners, scale, orig_w, orig_h);
                return (scaled, 0.95, ScanStatus::Success);
            }
        }

        // --- PASS 3: CANNY EDGE DETECTION (Cho nền tương phản thấp / viền kẻ) ---
        let high_thresh = (otsu_thresh as f32).clamp(50.0, 180.0);
        let low_thresh = (0.4 * high_thresh).max(20.0);
        let edges = canny(&blurred, low_thresh, high_thresh);

        let dilated_edges = dilate(&edges, Norm::L1, 2);
        let closed_edges = erode(&dilated_edges, Norm::L1, 2);

        let contours = find_contours::<u32>(&closed_edges);
        let mut contour_polys: Vec<Vec<Point2<f32>>> = contours
            .into_iter()
            .map(|c| {
                c.points
                    .into_iter()
                    .map(|p| Point2::new(p.x as f32, p.y as f32))
                    .collect()
            })
            .filter(|pts: &Vec<Point2<f32>>| pts.len() >= 4)
            .collect();

        contour_polys.sort_by(|a, b| {
            polygon_area(b)
                .partial_cmp(&polygon_area(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for poly in &contour_polys {
            let area = polygon_area(poly);
            if area < min_area {
                break;
            }

            let peri = polygon_perimeter(poly);
            for &eps_factor in &[0.015, 0.02, 0.03, 0.04, 0.05] {
                let mut approx = ramer_douglas_peucker(poly, eps_factor * peri);
                if approx.len() > 1 && (approx[0] - *approx.last().unwrap()).norm() < 15.0 {
                    approx.pop();
                }
                if approx.len() == 4 && is_valid_quadrilateral(&approx, proc_w, proc_h) {
                    let corners = order_corners(&approx);
                    let scaled = scale_corners_to_original(&corners, scale, orig_w, orig_h);
                    return (scaled, 0.90, ScanStatus::Success);
                }
            }
        }

        // --- FALLBACK CẤP 1: Convex Hull trên contour lớn nhất ---
        if let Some(largest) = contour_polys.first() {
            if polygon_area(largest) >= min_area {
                let hull = convex_hull(largest);
                let peri = polygon_perimeter(&hull);
                for &eps_factor in &[0.02, 0.03, 0.04, 0.06] {
                    let mut approx = ramer_douglas_peucker(&hull, eps_factor * peri);
                    if approx.len() > 1 && (approx[0] - *approx.last().unwrap()).norm() < 15.0 {
                        approx.pop();
                    }
                    if approx.len() == 4 && is_valid_quadrilateral(&approx, proc_w, proc_h) {
                        let corners = order_corners(&approx);
                        let scaled = scale_corners_to_original(&corners, scale, orig_w, orig_h);
                        return (scaled, 0.80, ScanStatus::Fallback);
                    }
                }
            }
        }

        // --- FALLBACK CẤP 2: Rotated Min Bounding Box trên contour lớn nhất ---
        if let Some(largest) = contour_polys.first() {
            if polygon_area(largest) >= min_area {
                if let Some(box_pts) = min_area_rect(largest) {
                    let corners = order_corners(&box_pts);
                    let scaled = scale_corners_to_original(&corners, scale, orig_w, orig_h);
                    return (scaled, 0.60, ScanStatus::Fallback);
                }
            }
        }

        // --- FALLBACK CẤP 3: Khung viền nội tiếp ảnh mặc định (lề an toàn 2%) ---
        let default_corners = self.default_bounds(orig_w, orig_h);
        (default_corners, 0.35, ScanStatus::Fallback)
    }

    /// Trích xuất tứ giác lồi từ ảnh nhị phân đã lọc hình thái học
    fn find_quad_from_binary(
        &self,
        binary_mask: &GrayImage,
        proc_w: u32,
        proc_h: u32,
        min_area: f32,
    ) -> Option<[Point2<f32>; 4]> {
        let contours = find_contours::<u32>(binary_mask);
        let mut polys: Vec<Vec<Point2<f32>>> = contours
            .into_iter()
            .map(|c| {
                c.points
                    .into_iter()
                    .map(|p| Point2::new(p.x as f32, p.y as f32))
                    .collect()
            })
            .filter(|pts: &Vec<Point2<f32>>| pts.len() >= 4)
            .collect();

        polys.sort_by(|a, b| {
            polygon_area(b)
                .partial_cmp(&polygon_area(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for poly in &polys {
            let area = polygon_area(poly);
            if area < min_area {
                break;
            }

            // 1. Thử nghiệm RDP trực tiếp trên contour (có dedup điểm đầu-cuối của vòng khép kín)
            let peri = polygon_perimeter(poly);
            for &eps_factor in &[0.015, 0.02, 0.025, 0.03, 0.04, 0.05] {
                let mut approx = ramer_douglas_peucker(poly, eps_factor * peri);
                if approx.len() > 1 && (approx[0] - *approx.last().unwrap()).norm() < 15.0 {
                    approx.pop();
                }
                if approx.len() == 4 && is_valid_quadrilateral(&approx, proc_w, proc_h) {
                    return Some(order_corners(&approx));
                }
            }

            // 2. Thử nghiệm RDP trên Convex Hull của contour
            let hull = convex_hull(poly);
            let hull_peri = polygon_perimeter(&hull);
            for &eps_factor in &[0.02, 0.025, 0.03, 0.04, 0.05, 0.06] {
                let mut approx = ramer_douglas_peucker(&hull, eps_factor * hull_peri);
                if approx.len() > 1 && (approx[0] - *approx.last().unwrap()).norm() < 15.0 {
                    approx.pop();
                }
                if approx.len() == 4 && is_valid_quadrilateral(&approx, proc_w, proc_h) {
                    return Some(order_corners(&approx));
                }
            }
        }

        None
    }

    fn default_bounds(&self, w: u32, h: u32) -> DocumentCorners {
        let margin_x = (w as f32 * 0.02).max(1.0);
        let margin_y = (h as f32 * 0.02).max(1.0);
        let w_f = w as f32;
        let h_f = h as f32;

        DocumentCorners::new(
            Point2::new(margin_x, margin_y),
            Point2::new(w_f - margin_x, margin_y),
            Point2::new(w_f - margin_x, h_f - margin_y),
            Point2::new(margin_x, h_f - margin_y),
        )
    }
}

/// Tính ngưỡng Otsu tự động cho ảnh xám
pub fn calculate_otsu_threshold(gray: &GrayImage) -> u8 {
    let mut hist = [0u32; 256];
    let total_pixels = (gray.width() * gray.height()) as f32;

    for pixel in gray.pixels() {
        hist[pixel[0] as usize] += 1;
    }

    let mut sum_all = 0.0f32;
    for (t, &count) in hist.iter().enumerate() {
        sum_all += (t as f32) * (count as f32);
    }

    let mut weight_bg = 0.0f32;
    let mut sum_bg = 0.0f32;
    let mut max_variance = 0.0f32;
    let mut best_thresh = 128u8;

    for (t, &count) in hist.iter().enumerate() {
        weight_bg += count as f32;
        if weight_bg == 0.0 {
            continue;
        }
        let weight_fg = total_pixels - weight_bg;
        if weight_fg == 0.0 {
            break;
        }

        sum_bg += (t as f32) * (count as f32);
        let mean_bg = sum_bg / weight_bg;
        let mean_fg = (sum_all - sum_bg) / weight_fg;

        let variance = weight_bg * weight_fg * (mean_bg - mean_fg).powi(2);
        if variance > max_variance {
            max_variance = variance;
            best_thresh = t as u8;
        }
    }

    best_thresh
}

/// Tạo mặt nạ Adaptive Thresholding dựa trên Box Blur cục bộ
fn calculate_adaptive_mask(gray: &GrayImage, radius: u32, c_offset: i32) -> GrayImage {
    let (w, h) = gray.dimensions();
    let channel_f32: Vec<f32> = gray.pixels().map(|p| p[0] as f32).collect();
    let local_mean = crate::utils::color::fast_box_blur(&channel_f32, w, h, radius);

    let mut mask = GrayImage::new(w, h);
    for y in 0..h {
        let row_offset = (y * w) as usize;
        for x in 0..w {
            let idx = row_offset + x as usize;
            let orig = gray.get_pixel(x, y)[0] as i32;
            let mean = local_mean[idx] as i32;
            let val = if orig >= mean - c_offset { 255 } else { 0 };
            mask.put_pixel(x, y, Luma([val]));
        }
    }
    mask
}

/// Tính diện tích đa giác theo công thức Shoelace
pub fn polygon_area(pts: &[Point2<f32>]) -> f32 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0f32;
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i].x * pts[j].y - pts[j].x * pts[i].y;
    }
    (area / 2.0).abs()
}

/// Tính chu vi đa giác
pub fn polygon_perimeter(pts: &[Point2<f32>]) -> f32 {
    let n = pts.len();
    if n < 2 {
        return 0.0;
    }
    let mut peri = 0.0f32;
    for i in 0..n {
        let j = (i + 1) % n;
        let dx = pts[j].x - pts[i].x;
        let dy = pts[j].y - pts[i].y;
        peri += (dx * dx + dy * dy).sqrt();
    }
    peri
}

/// Thuật toán Ramer-Douglas-Peucker đơn giản hóa đường gấp khúc/đa giác
pub fn ramer_douglas_peucker(pts: &[Point2<f32>], epsilon: f32) -> Vec<Point2<f32>> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }

    let mut max_dist = 0.0f32;
    let mut index = 0;
    let start = pts[0];
    let end = *pts.last().unwrap();

    for i in 1..pts.len() - 1 {
        let dist = point_to_segment_distance(pts[i], start, end);
        if dist > max_dist {
            max_dist = dist;
            index = i;
        }
    }

    if max_dist > epsilon {
        let mut rec1 = ramer_douglas_peucker(&pts[..=index], epsilon);
        let rec2 = ramer_douglas_peucker(&pts[index..], epsilon);

        rec1.pop(); // Tránh trùng điểm nối giữa
        rec1.extend(rec2);
        rec1
    } else {
        vec![start, end]
    }
}

/// Khoảng cách từ một điểm tới một đoạn thẳng
fn point_to_segment_distance(p: Point2<f32>, a: Point2<f32>, b: Point2<f32>) -> f32 {
    let ab = b - a;
    let ab_len_sq = ab.x * ab.x + ab.y * ab.y;
    if ab_len_sq < 1e-6 {
        return (p - a).norm();
    }

    let ap = p - a;
    let t = ((ap.x * ab.x + ap.y * ab.y) / ab_len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    (p - proj).norm()
}

/// Tìm Convex Hull bằng thuật toán Andrew's Monotone Chain
pub fn convex_hull(points: &[Point2<f32>]) -> Vec<Point2<f32>> {
    let mut pts = points.to_vec();
    if pts.len() <= 3 {
        return pts;
    }

    pts.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });

    pts.dedup_by(|a, b| (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4);
    if pts.len() <= 3 {
        return pts;
    }

    let cross = |o: Point2<f32>, a: Point2<f32>, b: Point2<f32>| -> f32 {
        (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
    };

    // Lower hull
    let mut lower = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }

    // Upper hull
    let mut upper = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }

    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Kiểm tra tính hợp lệ của hình tứ giác phát hiện được
pub fn is_valid_quadrilateral(pts: &[Point2<f32>], max_w: u32, max_h: u32) -> bool {
    if pts.len() != 4 {
        return false;
    }

    // 1. Kiểm tra tính lồi (Convexity) bằng tích có hướng
    let mut sign = 0.0f32;
    for i in 0..4 {
        let p1 = pts[i];
        let p2 = pts[(i + 1) % 4];
        let p3 = pts[(i + 2) % 4];

        let v1 = p2 - p1;
        let v2 = p3 - p2;
        let cp = v1.x * v2.y - v1.y * v2.x;

        if cp.abs() < 1e-3 {
            return false; // Điểm thẳng hàng
        }

        if sign == 0.0 {
            sign = cp.signum();
        } else if cp.signum() != sign {
            return false; // Không lồi hoặc tự cắt
        }
    }

    // 2. Kiểm tra góc giữa các cạnh kề nhau (35 độ đến 145 độ)
    for i in 0..4 {
        let p1 = pts[i];
        let p2 = pts[(i + 1) % 4];
        let p3 = pts[(i + 2) % 4];

        let v1 = p1 - p2;
        let v2 = p3 - p2;

        let dot = v1.x * v2.x + v1.y * v2.y;
        let n1 = (v1.x * v1.x + v1.y * v1.y).sqrt();
        let n2 = (v2.x * v2.x + v2.y * v2.y).sqrt();

        if n1 < 5.0 || n2 < 5.0 {
            return false;
        }

        let cos_angle = (dot / (n1 * n2)).clamp(-1.0, 1.0);
        let angle_deg = cos_angle.acos().to_degrees();

        if angle_deg < 35.0 || angle_deg > 145.0 {
            return false;
        }
    }

    // 3. Tỷ lệ diện tích tứ giác so với bounding box
    let area = polygon_area(pts);
    let total_area = (max_w * max_h) as f32;
    if area < total_area * 0.05 || area > total_area * 0.99 {
        return false;
    }

    true
}

/// Sắp xếp 4 đỉnh theo thứ tự chuẩn: [Top-Left, Top-Right, Bottom-Right, Bottom-Left]
pub fn order_corners(pts: &[Point2<f32>]) -> [Point2<f32>; 4] {
    assert!(pts.len() >= 4);
    let four = &pts[..4];

    // TL có tổng x + y nhỏ nhất
    // BR có tổng x + y lớn nhất
    let mut tl = four[0];
    let mut br = four[0];
    let mut min_sum = four[0].x + four[0].y;
    let mut max_sum = min_sum;

    for &p in four.iter().skip(1) {
        let sum = p.x + p.y;
        if sum < min_sum {
            min_sum = sum;
            tl = p;
        }
        if sum > max_sum {
            max_sum = sum;
            br = p;
        }
    }

    // TR có hiệu y - x nhỏ nhất (x - y lớn nhất)
    // BL có hiệu y - x lớn nhất (x - y nhỏ nhất)
    let mut tr = four[0];
    let mut bl = four[0];
    let mut min_diff = four[0].y - four[0].x;
    let mut max_diff = min_diff;

    for &p in four.iter().skip(1) {
        let diff = p.y - p.x;
        if diff < min_diff {
            min_diff = diff;
            tr = p;
        }
        if diff > max_diff {
            max_diff = diff;
            bl = p;
        }
    }

    [tl, tr, br, bl]
}

/// Scale tọa độ từ thumbnail về kích thước ảnh gốc và clamp trong khung hình
fn scale_corners_to_original(
    corners: &[Point2<f32>; 4],
    scale: f32,
    orig_w: u32,
    orig_h: u32,
) -> DocumentCorners {
    let inv_scale = 1.0 / scale;
    let max_x = (orig_w - 1) as f32;
    let max_y = (orig_h - 1) as f32;

    let scale_point = |p: Point2<f32>| -> Point2<f32> {
        Point2::new(
            (p.x * inv_scale).clamp(0.0, max_x),
            (p.y * inv_scale).clamp(0.0, max_y),
        )
    };

    DocumentCorners::new(
        scale_point(corners[0]),
        scale_point(corners[1]),
        scale_point(corners[2]),
        scale_point(corners[3]),
    )
}

/// Tính Min Area Rect (Rotated Bounding Box) cho tập điểm
pub fn min_area_rect(pts: &[Point2<f32>]) -> Option<[Point2<f32>; 4]> {
    let n = pts.len();
    if n < 3 {
        return None;
    }

    let mut min_area = f32::MAX;
    let mut best_box = [Point2::new(0.0, 0.0); 4];

    for i in 0..n {
        let p1 = pts[i];
        let p2 = pts[(i + 1) % n];
        let edge = p2 - p1;
        let len = (edge.x * edge.x + edge.y * edge.y).sqrt();
        if len < 1e-4 {
            continue;
        }

        let u_x = edge.x / len;
        let u_y = edge.y / len;
        let v_x = -u_y;
        let v_y = u_x;

        let mut min_u = f32::MAX;
        let mut max_u = f32::MIN;
        let mut min_v = f32::MAX;
        let mut max_v = f32::MIN;

        for p in pts {
            let du = p.x * u_x + p.y * u_y;
            let dv = p.x * v_x + p.y * v_y;
            min_u = min_u.min(du);
            max_u = max_u.max(du);
            min_v = min_v.min(dv);
            max_v = max_v.max(dv);
        }

        let area = (max_u - min_u) * (max_v - min_v);
        if area < min_area {
            min_area = area;
            best_box = [
                Point2::new(u_x * min_u + v_x * min_v, u_y * min_u + v_y * min_v),
                Point2::new(u_x * max_u + v_x * min_v, u_y * max_u + v_y * min_v),
                Point2::new(u_x * max_u + v_x * max_v, u_y * max_u + v_y * max_v),
                Point2::new(u_x * min_u + v_x * max_v, u_y * min_u + v_y * max_v),
            ];
        }
    }

    Some(best_box)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn test_order_corners() {
        let pts = [
            Point2::new(100.0, 10.0),  // TR
            Point2::new(10.0, 10.0),   // TL
            Point2::new(100.0, 150.0), // BR
            Point2::new(10.0, 150.0),  // BL
        ];
        let ordered = order_corners(&pts);
        assert_eq!(ordered[0], Point2::new(10.0, 10.0)); // TL
        assert_eq!(ordered[1], Point2::new(100.0, 10.0)); // TR
        assert_eq!(ordered[2], Point2::new(100.0, 150.0)); // BR
        assert_eq!(ordered[3], Point2::new(10.0, 150.0)); // BL
    }

    #[test]
    fn test_page_detector_synthetic() {
        let mut img = RgbImage::from_pixel(400, 500, Rgb([30, 30, 30]));

        // Vẽ trang giấy trắng ở giữa (từ x: 50..350, y: 50..450)
        for y in 50..450 {
            for x in 50..350 {
                img.put_pixel(x, y, Rgb([240, 240, 240]));
            }
        }

        let detector = PageDetector::new(800, 0.1);
        let (corners, conf, status) = detector.detect(&img);

        assert_eq!(status, ScanStatus::Success);
        assert!(conf >= 0.90);

        // Góc TL phải xấp xỉ (50, 50)
        assert!((corners.tl.x - 50.0).abs() < 10.0);
        assert!((corners.tl.y - 50.0).abs() < 10.0);
        // Góc BR phải xấp xỉ (350, 450)
        assert!((corners.br.x - 350.0).abs() < 10.0);
        assert!((corners.br.y - 450.0).abs() < 10.0);
    }
}
