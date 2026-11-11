# MyOpenScanner

[English](README.md) | [Tiếng Việt](README.vn.md)

MyOpenScanner là engine quét và chuẩn hóa tài liệu xây dựng bằng Rust. Hệ thống xử lý ảnh chụp tài liệu từ điện thoại (hóa đơn, chứng từ, vận đơn, hợp đồng) thành tệp PDF chuẩn in ấn và Searchable PDF có thể bôi đen chữ, đồng thời bảo tồn nguyên vẹn bố cục thực tế của tài liệu.

---

## Tính năng Nổi bật

* **Hiệu năng & Tốc độ:** Triển khai bằng Rust với khả năng xử lý đa luồng qua Rayon. Thời gian xử lý từ 100ms đến 350ms mỗi tài liệu trên CPU thông thường.
* **Bắt 4 Góc & Nắn Phối cảnh:**
  * Nhận diện góc nghiêng phối cảnh lên đến 35° trên các nền phức tạp (bàn gỗ, sàn nhà).
  * Trích xuất đường biên đa tầng kết hợp mặt nạ nhị phân Otsu và bộ lọc biên Canny thích ứng.
  * Tự động snap theo tỷ lệ khổ giấy chuẩn (A4, Letter) dựa trên khoảng cách Euclidean.
  * Giải ma trận Homography 3x3 và nắn phẳng bằng phép ánh xạ xạ ảnh ngược (Inverse Bilinear Warp).
* **Cân bằng Sáng Không gian Màu CIE LAB:**
  * Ước lượng bản đồ chiếu sáng nền trên kênh $L^*$ bằng thuật toán Integral Box Blur có tính toán chính xác số điểm ảnh vùng biên, loại bỏ bóng râm của bàn tay, điện thoại và đèn phòng.
  * **Bảo tồn Màu sắc Pháp lý:** Giữ nguyên hai kênh sắc độ ($a^*, b^*$). Mộc đỏ ($a^* \gg 0$), chữ ký bút bi xanh ($b^* \ll 0$) và màu nền của giấy liên (vàng, hồng, xanh) giữ nguyên sắc độ tự nhiên, không bị tẩy trắng nhân tạo.
* **Làm Nét Chữ Mảnh & Mã vạch:**
  * Áp dụng Unsharp Masking có ngưỡng trên kênh độ sáng.
  * Tăng độ tương phản của chữ in nhỏ (6pt–8pt) và vạch barcode 1D/2D mà không tạo quầng sáng bao quanh viền chữ (halo artifacts).
* **Đóng gói PDF & Searchable PDF (Bôi đen được):**
  * **Vector Wrapper PDF:** Nhúng ảnh chất lượng cao vào container PDF chuẩn 300 DPI qua `pdf-writer`.
  * **Searchable Sandwich PDF (`-s`):** Nhúng lớp văn bản ẩn (`PDF render_mode = 3`) khớp tọa độ với từng dòng và từ trên ảnh quét. Cho phép chuột bôi đen chọn chữ, sao chép (`Ctrl+C`) và tìm kiếm từ khóa (`Ctrl+F`) mà vẫn giữ nguyên ảnh quét và bố cục gốc.
  * Xuất đồng thời văn bản thuần (`.txt`) và metadata tọa độ các khối từ (`.json`).

---

## Kiến trúc Pipeline 5+1 Bước

```
[Ảnh Chụp Điện Thoại] (.jpg, .jpeg, .png, .webp)
   │
   ▼
1. Page Detector (Multi-pass: Mặt nạ Otsu + Canny thích ứng -> 4 góc [TL, TR, BR, BL])
   │
   ▼
2. Geometric Rectifier (Tỷ lệ Euclidean, snap A4/Letter, nắn Homography 3x3)
   │
   ▼
3. Illumination Normalizer (Chuyển LAB, khử bóng trên L* có bù biên, bảo tồn a*/b*)
   │
   ▼
4. Detail Enhancer (Thresholded Unsharp Masking trên L*, tăng nét chữ và vạch)
   │
   ▼
5. PDF Exporter (Đóng gói PDF 300 DPI qua pdf-writer + Xuất PNG/JPG + Log JSON)
   │
   ▼
[Tùy chọn: -s / --searchable-pdf]
6. OCR Text Overlay (Nhúng lớp chữ ẩn render_mode=3 cho phép bôi đen & copy text)
```

---

## Cài đặt & Biên dịch

### Yêu cầu Tiên quyết
* Rust toolchain $\ge 1.80$ (`cargo`, `rustc`).
* Python $\ge 3.10$ (cần thiết cho chức năng Searchable PDF có OCR):
  ```bash
  pip install pymupdf rapidocr-onnxruntime pillow
  ```

### Biên dịch Binary
```bash
git clone https://github.com/giahung25/MyOpenScanner.git
cd MyOpenScanner

# Biên dịch binary release tối ưu
cargo build --release

# Chạy bộ kiểm thử
cargo test
```

Đường dẫn tệp thực thi:
* Windows: `target/release/my_open_scanner.exe`
* Linux / macOS: `target/release/my_open_scanner`

---

## Hướng dẫn Sử dụng

### 1. Xử lý Tài liệu Đơn lẻ
```bash
# Quét tiêu chuẩn (xuất PDF vector 300 DPI và ảnh PNG đã làm phẳng)
my_open_scanner -i invoice.jpg -o ./output --format both --dpi 300

# Quét tạo Searchable PDF (bôi đen, copy và tìm kiếm văn bản được)
my_open_scanner -i document.png -o ./output -s --dpi 300
```

### 2. Xử lý Hàng loạt (Batch Mode)
```bash
# Xử lý song song đa luồng toàn bộ ảnh trong một thư mục
my_open_scanner -b ./invoices -o ./output -s --dpi 300
```

### 3. Danh mục Tham số CLI
| Cờ lệnh | Viết tắt | Mặc định | Chức năng |
| :--- | :---: | :---: | :--- |
| `--input <FILE>` | `-i` | Không | Đường dẫn ảnh tài liệu đầu vào. |
| `--batch-dir <DIR>` | `-b` | Không | Thư mục chứa ảnh để quét hàng loạt. |
| `--output-dir <DIR>` | `-o` | `./output` | Thư mục lưu kết quả xuất ra. |
| `--dpi <DPI>` | | `300` | Độ phân giải trang PDF (200 hoặc 300 DPI). |
| `--format <FMT>` | | `both` | Định dạng xuất: `pdf`, `png`, `jpg`, `both`. |
| `--target-luminance <0-255>`| | `230` | Độ sáng mục tiêu của nền giấy. |
| `--searchable-pdf` | `-s` | `false` | Tạo file PDF có lớp văn bản bôi đen được. |

---

## Cấu trúc Tệp Đầu ra

Với tệp đầu vào `document.png`, pipeline xuất bộ tệp gồm:
```
output/
├── document_scanned.pdf             # File PDF vector chuẩn 300 DPI
├── document_scanned_searchable.pdf  # File PDF có lớp text bôi đen và tìm kiếm được
├── document_scanned.png             # Ảnh độ phân giải cao đã nắn phẳng
├── document_scanned_text.txt        # Toàn bộ nội dung văn bản trích xuất
├── document_scanned_ocr.json        # Tọa độ và độ tin cậy của từng khối từ OCR
└── document_scanned.json            # Metadata trạng thái và thời gian xử lý
```

### Cấu trúc Metadata JSON
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

## Tài liệu Kỹ thuật Chi tiết

Tài liệu chi tiết được lưu trữ trong thư mục [`docs/`](docs/):
* [Architecture Documentation](docs/architecture.md): Phân rã module và luồng dữ liệu.
* [Algorithms Specification](docs/algorithms.md): Công thức toán học cho Homography, mô hình chiếu sáng LAB và unsharp mask.
* [Data Contract](docs/data_contract.md): Đặc tả schema JSON, giao diện CLI và mã trạng thái.
* [Benchmark & QA Guidelines](docs/benchmark_and_qa.md): Phương pháp kiểm thử trường hợp biên và đo độ trễ.

---

## Giấy phép
Phát hành theo giấy phép MIT License. Kho mã nguồn tại [github.com/giahung25/MyOpenScanner](https://github.com/giahung25/MyOpenScanner).
