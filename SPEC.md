# MyOpenScanner - Đặc tả Yêu cầu Kỹ thuật & Kiến trúc Pipeline (Rust Edition)

## 1. Tổng quan Dự án
MyOpenScanner là engine quét tài liệu hiệu năng cao được xây dựng hoàn toàn bằng **Rust**. Hệ thống chuyển đổi ảnh chụp tài liệu (hóa đơn, chứng từ, vận đơn, hợp đồng) từ camera điện thoại thành file PDF/Image chất lượng cao chuẩn in ấn và lưu trữ, tối ưu hóa cho downstream OCR/Barcode engines với độ trễ xử lý cực thấp ($< 150\text{ms}$ trên CPU thông thường).

---

## 2. Công nghệ Cốt lõi (Tech Stack)
* **Ngôn ngữ:** Rust (2021 Edition, rustc $\ge 1.80$).
* **Hệ sinh thái Crates:**
  * `image` (0.25+): Đọc/ghi ảnh (`DynamicImage`, `RgbImage`, `RgbaImage`), pixel buffers.
  * `imageproc`: Xử lý hình ảnh, Canny edge detection, Gaussian blur, morphological operations.
  * `nalgebra`: Đại số tuyến tính, tính ma trận phối cảnh Homography $3 \times 3$ và nắn phối cảnh (perspective warp).
  * `palette`: Không gian màu chuẩn quốc tế CIE LAB ($L^*, a^*, b^*$).
  * `pdf-writer` / `printpdf`: Tạo container PDF chuẩn DPI vector wrapper, nhúng ảnh JPEG/FlateDecode lossless.
  * `serde`, `serde_json`: Serialization cho JSON Metadata Contract.
  * `clap` (v4, derive): CLI parsing đa năng cho người dùng cuối.
  * `rayon`: Xử lý song song hóa (multithreading) đa luồng khi chạy batch thư mục.

---

## 3. Đặc tả Đầu vào & Đầu ra (Input / Output Specification)

### 3.1. Đầu vào (Input)
* **Định dạng hỗ trợ:** `.jpg`, `.jpeg`, `.png`, `.webp` (chế độ đơn lẻ hoặc thư mục batch).
* **Độ phân giải khuyến nghị:** Tối thiểu $1200 \times 1600$ px (camera 8MP – 12MP).
* **Điều kiện nhiễu thực tế cần xử lý:**
  * **Hình học:** Góc chụp nghiêng (perspective skew lên tới $35^\circ$), giấy cong nhẹ hoặc có nếp gấp (wrinkles), nền xung quanh lộn xộn (bàn gỗ, sàn nhà, ghế da).
  * **Ánh sáng:** Bóng râm cục bộ (bóng bàn tay, bóng điện thoại che khuất), ánh sáng vàng/trắng không đều.
  * **Đặc tính tài liệu:** Giấy trắng hoặc giấy màu (liên hồng, vàng, xanh), có khung bảng kẻ mảnh, mã vạch (1D/2D Barcode), chữ viết tay bút bi xanh/đen, dấu mộc đỏ.

### 3.2. Đầu ra (Output)
* **Tệp chính:** File `.pdf` (đóng gói lossless không nén vỡ hạt) và ảnh `.png`/`.jpg` chất lượng cao.
* **Thông số kỹ thuật:**
  * **Bố cục:** Cắt sát 4 cạnh tờ giấy, tỷ lệ khung hình nội suy chuẩn kích thước thực tế (A4/Letter), các đường kẻ bảng ngang/dọc vuông góc chuẩn $90^\circ$.
  * **Màu sắc & Ánh sáng:** Độ sáng nền đồng nhất trên toàn trang (không còn vệt bóng đen), **giữ nguyên màu nền giấy gốc** (giấy vàng ra vàng nhạt đồng đều, không tẩy trắng thành giấy trắng).
  * **Bảo tồn màu sắc pháp lý:** Chữ ký xanh ($b^* \ll 0$) và dấu mộc đỏ ($a^* \gg 0$) được giữ nguyên màu gốc.
  * **Độ sắc nét:** Chữ in nhỏ (6pt–8pt), nét bút ký và vạch Barcode tách bạch, triệt tiêu viền hào quang (halo artifacts).
* **Metadata đi kèm (JSON Contract):**
```json
{
  "file_name": "invoice_001.jpg",
  "status": "success",
  "detected_corners": [[120, 85], [1890, 110], [1920, 2650], [95, 2610]],
  "deskew_angle_deg": 12.4,
  "confidence_score": 0.96,
  "inferred_paper_size": "A4",
  "target_dpi": 300,
  "processing_time_ms": 115,
  "output_path": "output/invoice_001_scanned.pdf"
}
```

---

## 4. Kiến trúc 5 Bước (Functional Pipeline)

```
[Raw Image]
   │
   ▼
1. Page Detector (Tách nền & Bắt 4 góc)
   │
   ▼
2. Geometric Rectifier (Cắt biên, Nắn phối cảnh & Làm phẳng)
   │
   ▼
3. Illumination Normalizer (Khử bóng & Cân bằng sáng trên hệ màu LAB)
   │
   ▼
4. Detail Enhancer (Làm nét chữ mảnh, chữ ký & Barcode)
   │
   ▼
5. PDF Exporter (Đóng gói PDF chuẩn DPI)
```

### Module 1: Page Detector (`src/modules/page_detector.rs`)
* Multi-scale processing: Resize ảnh về thumbnail $\sim 800\text{px}$ để giảm thiểu chi phí tính toán.
* Phát hiện biên (Canny edge detection) kết hợp Morphological closing để liên kết các đoạn biên đứt quãng.
* Tìm contour tứ giác lồi diện tích lớn nhất (Convex Quadrilateral).
* Sắp xếp 4 đỉnh theo thứ tự chuẩn: Top-Left, Top-Right, Bottom-Right, Bottom-Left (`[TL, TR, BR, BL]`).
* Scale tọa độ 4 góc về kích thước ảnh gốc $1.0\times$.

### Module 2: Geometric Rectifier (`src/modules/geometric_rectifier.rs`)
* Tính kích thước mục tiêu $W \times H$ dựa trên khoảng cách Euclidean lớn nhất của các cặp cạnh đối diện.
* Kiểm tra và snap về tỷ lệ tiêu chuẩn A4 ($1:\sqrt{2}$) nếu sai số $< 5\%$.
* Giải hệ phương trình tìm ma trận Homography $\mathbf{H}_{3 \times 3}$ qua `nalgebra`.
* Thực hiện biến đổi xạ ảnh ngược (Inverse Perspective Mapping) với Bilinear Interpolation trên ảnh gốc.

### Module 3: Illumination Normalizer (`src/modules/illumination_normalizer.rs`)
* Chuyển đổi RGB sang CIE LAB.
* Trích xuất kênh $L^*$ (Luminance). Ước lượng bản đồ nền (Background Illumination Map) bằng Fast Box Blur / Bilateral Filter với bán kính kernel lớn.
* Chuẩn hóa:
  $$L^*_{\text{norm}}(x, y) = \text{clamp}\left( \frac{L^*(x,y)}{L^*_{\text{bg}}(x,y)} \times 255, 0, 255 \right)$$
* Giữ nguyên hai kênh sắc độ $a^*$ và $b^*$ để bảo tồn màu sắc của mộc đỏ, chữ ký và màu nền giấy liên.
* Chuyển ngược lại RGB.

### Module 4: Detail Enhancer (`src/modules/detail_enhancer.rs`)
* Thực hiện Thresholded Unsharp Masking trên kênh $L^*$:
  $$\Delta = L^* - L^*_{\text{blur}}$$
  Chỉ áp dụng tăng nét khi $|\Delta| > \text{threshold}$ nhằm tránh khuếch đại hạt nhiễu trên nền phẳng và loại bỏ viền hào quang (halo artifacts).

### Module 5: PDF Exporter (`src/modules/pdf_exporter.rs`)
* Nhúng ảnh kết quả vào trang PDF chuẩn khổ (A4: 210 x 297 mm, Letter: 8.5 x 11 inch) theo DPI thực tế (200/300 DPI).
* Xuất file `.pdf`, tùy chọn xuất file `.png`/`.jpg` chất lượng cao.
* Tạo cấu trúc JSON metadata log.

---

## 5. Cấu trúc Dự án (Rust Workspace Layout)

```
MyOpenScanner/
├── Cargo.toml
├── SPEC.md
├── src/
│   ├── lib.rs                        # Public API: ScannerPipeline, Config
│   ├── main.rs                       # CLI tool: arguments parsing & batch runner
│   ├── models.rs                     # ScanResult, DocumentCorners, OutputFormat
│   ├── pipeline.rs                   # Điều phối 5 bước xử lý
│   ├── modules/
│   │   ├── mod.rs
│   │   ├── page_detector.rs          # Bước 1
│   │   ├── geometric_rectifier.rs    # Bước 2
│   │   ├── illumination_normalizer.rs# Bước 3
│   │   ├── detail_enhancer.rs        # Bước 4
│   │   └── pdf_exporter.rs           # Bước 5
│   └── utils/
│       ├── mod.rs
│       ├── color.rs                  # Chuyển đổi màu sRGB <-> CIE LAB
│       └── homography.rs             # Tính Homography 3x3 & Warp Bilinear
```
