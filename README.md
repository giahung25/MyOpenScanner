# MyOpenScanner

[English](README.md) | [Tiếng Việt](README.vn.md)

MyOpenScanner is a document scanning engine written in Rust. It processes smartphone photographs of paper documents (invoices, receipts, waybills, contracts) into perspective-corrected, shadow-normalized, print-ready PDF and Searchable PDF files with exact visual layout preservation.

---

## Key Features

* **High Performance:** Implemented in pure Rust with multi-threaded processing via Rayon. End-to-end processing latency ranges from 100ms to 350ms per document on standard CPUs.
* **Geometric Rectification & Corner Detection:**
  * Detects perspective skew up to 35° on complex backgrounds (wood desks, dark tables, floors).
  * Multi-pass contour extraction combining Otsu binary masking and adaptive Canny edges.
  * Aspect ratio snapping to standard paper formats (A4, Letter) based on Euclidean edge measurements.
  * Inverse bilinear perspective mapping using a 3x3 Homography solver.
* **CIE LAB Illumination Normalization:**
  * Background illumination estimation on the $L^*$ channel using an integral box blur with boundary pixel count correction to eliminate shadows from hands, phones, and ambient light.
  * **Color Preservation:** Preserves chrominance channels ($a^*, b^*$). Red stamps ($a^* \gg 0$), blue ink signatures ($b^* \ll 0$), and colored carbon copy papers retain their original color without synthetic over-bleaching.
* **Edge & Detail Enhancement:**
  * Thresholded unsharp masking on the luminance channel.
  * Enhances small printed text (6pt–8pt) and 1D/2D barcodes without introducing halo or ringing artifacts.
* **Searchable PDF Generation:**
  * **Vector Wrapper PDF:** Embeds processed images into a 300 DPI vector PDF container using `pdf-writer`.
  * **Searchable Sandwich PDF (`-s`):** Overlays an invisible selectable text layer (`PDF render_mode = 3`) aligned with OCR bounding boxes. Allows text selection, clipboard copying, and keyword searching (`Ctrl+F`) while keeping the scanned document image and original layout intact.
  * Exports extracted plain text (`.txt`) and word bounding box metadata (`.json`).

---

## Architecture & Pipeline

```
[Raw Camera Image] (.jpg, .jpeg, .png, .webp)
   │
   ▼
1. Page Detector (Multi-pass: Otsu Mask + Adaptive Canny -> Ordered [TL, TR, BR, BL])
   │
   ▼
2. Geometric Rectifier (Euclidean aspect ratio, A4/Letter snap, 3x3 Homography Warp)
   │
   ▼
3. Illumination Normalizer (CIE LAB conversion, boundary-corrected L* normalization, a*/b* preservation)
   │
   ▼
4. Detail Enhancer (Thresholded Unsharp Masking on L*, stroke contrast boost)
   │
   ▼
5. PDF Exporter (300 DPI vector PDF embedding via pdf-writer + Raster PNG/JPG + JSON log)
   │
   ▼
[Optional: -s / --searchable-pdf]
6. OCR Text Overlay (Invisible text layer render_mode=3 for copy/search capability)
```

---

## Installation & Build

### Prerequisites
* Rust toolchain $\ge 1.80$ (`cargo`, `rustc`).
* Python $\ge 3.10$ (required for Searchable PDF generation with OCR):
  ```bash
  pip install pymupdf rapidocr-onnxruntime pillow
  ```

### Build Binary
```bash
git clone https://github.com/giahung25/MyOpenScanner.git
cd MyOpenScanner

# Build optimized release executable
cargo build --release

# Run test suite
cargo test
```

Binary path:
* Windows: `target/release/my_open_scanner.exe`
* Linux / macOS: `target/release/my_open_scanner`

---

## Usage

### 1. Single Document Processing
```bash
# Standard scan (exports 300 DPI vector PDF and rectified PNG)
my_open_scanner -i invoice.jpg -o ./output --format both --dpi 300

# Searchable scan (creates PDF with copyable text and plain text export)
my_open_scanner -i document.png -o ./output -s --dpi 300
```

### 2. Batch Processing
```bash
# Concurrently process all images in a folder across CPU cores
my_open_scanner -b ./invoices -o ./output -s --dpi 300
```

### 3. CLI Options
| Flag | Short | Default | Description |
| :--- | :---: | :---: | :--- |
| `--input <FILE>` | `-i` | None | Path to input document image. |
| `--batch-dir <DIR>` | `-b` | None | Directory of images for multi-threaded batch processing. |
| `--output-dir <DIR>` | `-o` | `./output` | Output destination directory. |
| `--dpi <DPI>` | | `300` | Target PDF resolution (200 or 300 DPI). |
| `--format <FMT>` | | `both` | Output format: `pdf`, `png`, `jpg`, `both`. |
| `--target-luminance <0-255>`| | `230` | Target paper background luminance. |
| `--searchable-pdf` | `-s` | `false` | Generate searchable PDF with selectable text layer. |

---

## Output Artifacts

For an input file named `document.png`, the pipeline outputs:
```
output/
├── document_scanned.pdf             # Standard 300 DPI vector PDF
├── document_scanned_searchable.pdf  # Searchable PDF with selectable text
├── document_scanned.png             # Rectified high-resolution image
├── document_scanned_text.txt        # Extracted plain text
├── document_scanned_ocr.json        # OCR bounding boxes and confidence scores
└── document_scanned.json            # Pipeline execution metadata
```

### Metadata Schema
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

## Technical Documentation

Detailed specifications are maintained in [`docs/`](docs/):
* [Architecture Documentation](docs/architecture.md): Module decomposition and dataflow invariants.
* [Algorithms Specification](docs/algorithms.md): Formulations for Homography, LAB illumination modeling, and unsharp masking.
* [Data Contract](docs/data_contract.md): JSON schema definitions, CLI parameters, and exit codes.
* [Benchmark & QA Guidelines](docs/benchmark_and_qa.md): Latency test cases and boundary conditions.

---

## License
MIT License. Hosted at [github.com/giahung25/MyOpenScanner](https://github.com/giahung25/MyOpenScanner).
