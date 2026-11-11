# MyOpenScanner 📄⚡

[English](README.md) | [Tiếng Việt](README.vn.md)

> **High-Performance Document Scanning & Searchable PDF Engine in Rust**  
> An open-source, production-grade document scanning engine built in Rust. Transforms noisy, smartphone-captured documents (invoices, receipts, contracts) into clean, perspective-corrected, illumination-normalized, print-ready PDFs and Searchable PDFs with exact layout-preserved selectable text.

---

## 🌟 Key Features

* 🚀 **Blazing Fast Performance:** Implemented 100% in pure **Rust**, multi-threaded with **Rayon**, zero GIL. End-to-end processing latency of **$100\text{ms} - 350\text{ms}$** on standard multi-core CPUs.
* 📐 **Robust 4-Corner Detection & Geometric Rectification:**
  * Handles extreme perspective tilt (up to $35^\circ$ skew), wrinkles, and cluttered backgrounds (wooden tables, floors).
  * Multi-pass contour detection combining **Otsu Binary Masking** and **Adaptive Canny Edges** to reliably eliminate background artifacts.
  * Automatic Euclidean aspect ratio calculation and snapping to standard print dimensions (**A4**, **Letter**).
  * Homography $3 \times 3$ solver with parallelized inverse bilinear perspective mapping.
* 🎨 **CIE LAB Illumination Normalization:**
  * Background illumination estimation on the $L^*$ (Luminance) channel via an integral fast box blur with exact boundary pixel-count correction, completely removing shadows (hands, phones, uneven lighting).
  * **100% Legal Color Preservation:** Strictly preserves chrominance channels ($a^*, b^*$). **Red stamps/seals** ($a^* \gg 0$), **blue ink signatures** ($b^* \ll 0$), and colored carbon copy papers (yellow, pink, blue) retain their natural, vibrant tones without synthetic bleaching.
* 🔍 **Micro-Detail & Barcode Enhancement:**
  * Thresholded unsharp masking selectively sharpens fine printed text (6pt–8pt) and 1D/2D barcode lines while suppressing noise on flat areas and eliminating halo/ringing artifacts.
* 📑 **DPI-Accurate & Searchable PDF Generation:**
  * **Vector Wrapper PDF:** Embeds lossless, high-res images into 200/300 DPI vector PDF containers using `pdf-writer`.
  * **Searchable Sandwich PDF (`-s`):** Embeds an invisible text layer (`PDF render_mode = 3`) aligned with pixel-level precision over the original document. Enables **cursor selection, clipboard copying (`Ctrl+C`), and full-text keyword search (`Ctrl+F`)** while preserving 100% of the original visual layout, paper texture, and signatures.
  * Simultaneously exports plain text (`.txt`) and structured word-level bounding box metadata (`.json`).

---

## 🏗️ 5+1 Step Functional Pipeline

```
[Raw Smartphone Image] (.jpg, .jpeg, .png, .webp)
   │
   ▼
1. Page Detector (Multi-pass: Otsu Mask + Adaptive Canny -> Ordered 4 Corners [TL, TR, BR, BL])
   │
   ▼
2. Geometric Rectifier (Euclidean aspect ratio, A4/Letter snap, Homography 3x3 Inverse Warp)
   │
   ▼
3. Illumination Normalizer (CIE LAB conversion, border-accurate L* shadow removal, a*/b* preservation)
   │
   ▼
4. Detail Enhancer (Thresholded Unsharp Masking on L*, fine text & barcode boost, zero halo)
   │
   ▼
5. PDF Exporter (300 DPI PDF vector embedding via pdf-writer + Raster PNG/JPG + JSON log)
   │
   ▼
[Optional: -s / --searchable-pdf]
6. OCR Text Overlay (Invisible text layer render_mode=3, text selection & clipboard copying)
```

---

## 🛠️ Installation & Build

### Prerequisites
* **Rust & Cargo:** Version $\ge 1.80$ ([Install Rust](https://rustup.rs/)).
* **Python (Optional, for Searchable PDF generation):** Version $\ge 3.10$ with dependencies:
  ```bash
  pip install pymupdf rapidocr-onnxruntime pillow
  ```

### Build from Source (Release Mode)
```bash
git clone https://github.com/giahung25/MyOpenScanner.git
cd MyOpenScanner

# Compile optimized release binary
cargo build --release

# Run comprehensive test suite (17 tests)
cargo test
```
The compiled binary will be located at:
* Windows: `target/release/my_open_scanner.exe`
* Linux / macOS: `target/release/my_open_scanner`

---

## 💻 CLI Usage Guide

### 1. Single Document Scanning
```bash
# Standard scan (exports 300 DPI PDF and rectified high-res PNG)
my_open_scanner -i invoice.jpg -o ./output --format both --dpi 300

# Scan & Generate Searchable PDF (cursor-selectable text, copyable, searchable)
my_open_scanner -i document.png -o ./output -s --dpi 300
```

### 2. Multi-Threaded Batch Scanning (Rayon Parallelism)
```bash
# Scan an entire directory of documents concurrently across all CPU cores
my_open_scanner -b ./invoices -o ./output -s --dpi 300
```

### 3. Command-Line Options
| Option | Short | Default | Description |
| :--- | :---: | :---: | :--- |
| `--input <FILE>` | `-i` | *None* | Path to a single input image. |
| `--batch-dir <DIR>` | `-b` | *None* | Path to directory containing images for batch processing. |
| `--output-dir <DIR>` | `-o` | `./output` | Output directory for scanned documents. |
| `--dpi <DPI>` | | `300` | Target PDF DPI ($200$ or $300$). |
| `--format <FMT>` | | `both` | Output format: `pdf`, `png`, `jpg`, `both`. |
| `--target-luminance <0-255>`| | `230` | Target background luminance (0-255). |
| `--searchable-pdf` | `-s` | `false` | Enable OCR to create a Searchable PDF with selectable text. |

---

## 📊 Output Artifacts

Each document scan generates a standardized set of artifacts in the output directory:
```
output/
├── document_scanned.pdf             # Standard 300 DPI vector PDF document
├── document_scanned_searchable.pdf  # Searchable PDF with copyable & searchable text layer
├── document_scanned.png             # Rectified and normalized high-res image
├── document_scanned_text.txt        # Full extracted plain text with layout preservation
├── document_scanned_ocr.json        # Detailed word-level bounding boxes and confidence scores
└── document_scanned.json            # Structured pipeline JSON metadata log
```

### Data Contract (JSON Metadata Log)
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

## 📚 Technical Documentation

Detailed architectural and mathematical specifications are available in the [`docs/`](docs/) directory:
* 📐 [Architecture Documentation](docs/architecture.md): 5-module pipeline design, invariants, and dataflow.
* 🧮 [Algorithms Specification](docs/algorithms.md): Mathematical formulations for Convex Quad detection, Homography $3\times3$, LAB Integral Box Blur, Thresholded Unsharp Masking, and PDF coordinate transformations.
* 📝 [Data Contract](docs/data_contract.md): JSON schema specifications, CLI parameter interfaces, status codes, and error codes.
* 🧪 [Benchmark & QA Guidelines](docs/benchmark_and_qa.md): Edge-case test suites and latency measurement methodologies.

---

## ⚖️ License
This project is open-source and released under the MIT License.  
Contributions, issues, and pull requests are welcome at [giahung25/MyOpenScanner](https://github.com/giahung25/MyOpenScanner).
