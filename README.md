# MyOpenScanner 📄⚡

> **High-Performance Document Scanning & Searchable PDF Engine in Rust**  
> Engine quét và chuẩn hóa tài liệu chụp từ điện thoại (hóa đơn, chứng từ, hợp đồng) với tốc độ cao, khử bóng thông minh, bảo tồn con dấu đỏ & chữ ký xanh, và xuất PDF có thể bôi đen chữ (Searchable PDF) giữ nguyên $100\%$ bố cục gốc.

---

## 🌟 Tính năng Nổi bật (Key Features)

* 🚀 **Hiệu năng Cực cao (Blazing Fast):** Viết hoàn toàn bằng **Rust**, đa luồng hóa qua **Rayon**, không có GIL, độ trễ xử lý chỉ từ **$100\text{ms} - 350\text{ms}$** trên CPU thông thường.
* 📐 **Bắt 4 Góc & Nắn Phối cảnh (Auto Corner Detection & Rectification):**
  * Tự động nhận diện góc nghiêng (perspective skew lên tới $35^\circ$).
  * Multi-pass contour detection kết hợp **Otsu Binary Masking** và **Canny Edges**, loại bỏ sạch mặt bàn gỗ, bóng râm và vật thể gây nhiễu.
  * Tự động snap chuẩn tỷ lệ in ấn thực tế (**A4**, **Letter**).
  * Giải ma trận Homography $3 \times 3$ và nắn phẳng bằng phép ánh xạ xạ ảnh ngược (Inverse Bilinear Warp).
* 🎨 **Cân bằng Sáng CIE LAB (Illumination Normalization):**
  * Ước lượng bản đồ nền độ sáng trên kênh $L^*$ bằng thuật toán Integral Fast Box Blur có bù trừ biên điểm ảnh chính xác, triệt tiêu hoàn toàn bóng bàn tay và bóng điện thoại.
  * **Bảo tồn $100\%$ màu sắc pháp lý:** Giữ nguyên độ tươi của **mộc đỏ** ($a^* \gg 0$), **chữ ký bút bi xanh** ($b^* \ll 0$) và các loại giấy liên màu (hồng, vàng, xanh). Không bị tẩy trắng nhân tạo.
* 🔍 **Làm Nét Chữ Mảnh & Barcode (Detail Enhancement):**
  * Áp dụng Thresholded Unsharp Masking có kiểm soát ngưỡng.
  * Tăng độ tương phản của chữ in nhỏ (6pt–8pt) và vạch mã vạch (1D/2D Barcode) mà **không sinh quầng sáng viền (halo artifacts)**.
* 📑 **Đóng gói PDF Chuẩn DPI & Searchable PDF (Bôi đen được):**
  * **Vector Wrapper PDF:** Đóng gói ảnh chất lượng cao vào container PDF chuẩn $200 / 300\text{ DPI}$ qua `pdf-writer`.
  * **Searchable Sandwich PDF (`-s`):** Nhúng lớp văn bản ẩn (Invisible Text Layer, `render_mode = 3`) đè chính xác lên từng dòng/từ của ảnh gốc. Cho phép **chuột bôi đen chọn chữ, copy (`Ctrl+C`) và tìm kiếm từ khóa (`Ctrl+F`)** mà vẫn giữ nguyên $100\%$ bố cục trang giấy.
  * Xuất đồng thời text thuần (`.txt`) và metadata JSON tọa độ từng khối từ.

---

## 🏗️ Kiến trúc Pipeline 5+1 Bước (Architecture)

```
[Raw Smartphone Image] (JPG/PNG/WEBP)
   │
   ▼
1. Page Detector (Multi-pass: Otsu Mask + Adaptive Canny -> Bắt 4 góc lồi [TL, TR, BR, BL])
   │
   ▼
2. Geometric Rectifier (Tính Euclidean aspect ratio, snap A4/Letter, Homography 3x3 Warp)
   │
   ▼
3. Illumination Normalizer (Chuyển LAB, khử bóng trên L*, bảo tồn mộc đỏ a* và chữ ký b*)
   │
   ▼
4. Detail Enhancer (Thresholded Unsharp Masking trên L*, chống halo artifacts)
   │
   ▼
5. PDF Exporter (Đóng gói PDF vector 300 DPI qua pdf-writer + Xuất PNG/JPG + JSON log)
   │
   ▼
[Optional: -s / --searchable-pdf]
6. OCR Text Overlay (Nhúng invisible text layer render_mode=3, bôi đen & copy text)
```

---

## 🛠️ Cài đặt & Biên dịch (Installation & Build)

### Yêu cầu Tiên quyết
* **Rust & Cargo:** Phiên bản $\ge 1.80$ ([Cài đặt Rust](https://rustup.rs/)).
* **Python (Tùy chọn, cho tính năng Searchable PDF):** Phiên bản $\ge 3.10$ với các thư viện:
  ```bash
  pip install pymupdf rapidocr-onnxruntime pillow
  ```

### Biên dịch Binary Tối ưu (Release Mode)
```bash
git clone https://github.com/giahung25/MyOpenScanner.git
cd MyOpenScanner

# Biên dịch binary release
cargo build --release

# Chạy test suite toàn diện (17 tests)
cargo test
```
File thực thi nằm tại: `target/release/my_open_scanner.exe` (Windows) hoặc `target/release/my_open_scanner` (Linux/macOS).

---

## 💻 Hướng dẫn Sử dụng CLI (Usage)

### 1. Quét Một Ảnh Đơn lẻ
```bash
# Quét cơ bản (xuất cả PDF 300 DPI và ảnh PNG đã làm phẳng)
my_open_scanner -i invoice.jpg -o ./output --format both --dpi 300

# Quét và tạo Searchable PDF (bôi đen, copy và tìm kiếm chữ được)
my_open_scanner -i don_xin_xac_nhan.png -o ./output -s --dpi 300
```

### 2. Quét Hàng Loạt Thư Mục Ảnh (Batch Mode - Đa luồng Rayon)
```bash
# Quét song song tất cả ảnh trong thư mục invoices/
my_open_scanner -b ./invoices -o ./output -s --dpi 300
```

### 3. Toàn bộ Tham số Dòng lệnh (CLI Options)
| Tham số | Viết tắt | Mặc định | Mô tả |
| :--- | :---: | :---: | :--- |
| `--input <FILE>` | `-i` | *None* | Đường dẫn file ảnh đầu vào đơn lẻ. |
| `--batch-dir <DIR>` | `-b` | *None* | Thư mục chứa hàng loạt ảnh để quét song song. |
| `--output-dir <DIR>` | `-o` | `./output` | Thư mục lưu kết quả xuất ra. |
| `--dpi <DPI>` | | `300` | DPI trang tài liệu PDF ($200$ hoặc $300$). |
| `--format <FMT>` | | `both` | Định dạng xuất: `pdf`, `png`, `jpg`, `both`. |
| `--target-luminance <0-255>`| | `230` | Độ sáng mục tiêu cho nền tài liệu. |
| `--searchable-pdf` | `-s` | `false` | Bật module OCR tạo file PDF có lớp text bôi đen được. |

---

## 📊 Cấu trúc File Đầu ra (Output Artifacts)

Mỗi lần quét sẽ sinh ra bộ tệp chuẩn hóa trong thư mục output:
```
output/
├── document_scanned.pdf             # File PDF vector chuẩn in ấn (300 DPI)
├── document_scanned_searchable.pdf  # File PDF có lớp text bôi đen, copy và Ctrl+F được
├── document_scanned.png             # Ảnh độ phân giải cao đã cắt biên & làm phẳng
├── document_scanned_text.txt        # Toàn bộ nội dung văn bản trích xuất
├── document_scanned_ocr.json        # Metadata chi tiết tọa độ từng khối từ OCR
└── document_scanned.json            # Metadata JSON log của pipeline
```

### Hợp đồng Dữ liệu (JSON Metadata Contract)
```json
{
  "file_name": "don_xin_xac_nhan.png",
  "status": "success",
  "detected_corners": {
    "tl": [337.92, 212.48],
    "tr": [1295.36, 245.76],
    "br": [1484.80, 1881.60],
    "bl": [199.68, 1832.96]
  },
  "deskew_angle_deg": 2.0,
  "confidence_score": 0.98,
  "inferred_paper_size": "LETTER",
  "target_dpi": 300,
  "processing_time_ms": 674,
  "output_path": "./output/don_xin_xac_nhan_scanned.pdf",
  "searchable_pdf_path": "./output/don_xin_xac_nhan_scanned_searchable.pdf",
  "error_message": null
}
```

---

## 📚 Tài liệu Kỹ thuật Chuyên sâu (Documentation)

Hệ thống cung cấp trọn bộ tài liệu chi tiết trong thư mục [`docs/`](docs/):
* 📐 [Architecture Documentation](docs/architecture.md): Thiết kế tổng quan 5 module pipeline và luồng dữ liệu.
* 🧮 [Algorithms Specification](docs/algorithms.md): Chi tiết toán học các giải thuật (Convex Quad, Homography $3\times3$, LAB Integral Blur, Thresholded Unsharp Masking, PDF coordinate mapping).
* 📝 [Data Contract](docs/data_contract.md): Đặc tả JSON schema, CLI parameters, mã trạng thái và mã lỗi.
* 🧪 [Benchmark & QA Guidelines](docs/benchmark_and_qa.md): Phương pháp kiểm thử test cases biên và tiêu chuẩn đo lường độ trễ.

---

## ⚖️ Giấy phép (License)
Dự án được phát hành theo giấy phép mã nguồn mở MIT License.
Mọi đóng góp (Pull Requests, Bug Reports) đều được hoan nghênh tại [giahung25/MyOpenScanner](https://github.com/giahung25/MyOpenScanner).
