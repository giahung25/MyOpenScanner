# MyOpenScanner - Architecture Documentation

## Overview

MyOpenScanner is a high-performance document scanning engine built entirely in **Rust**. It transforms smartphone-captured document images (invoices, receipts, waybills, contracts) into print-ready, archive-quality PDF/Image files, optimized for downstream OCR and barcode processing pipelines.

**Target Performance**: < 150ms end-to-end latency per image on commodity CPU hardware.

---

## System Architecture

### High-Level Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                        INPUT IMAGE                                   │
│                  (JPG/PNG/WebP, 8-12MP)                             │
└─────────────────────────────┬───────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│  MODULE 1: PAGE DETECTOR                                            │
│  • Multi-scale processing (thumbnail ~800px)                        │
│  • Canny edge detection + Morphological closing                     │
│  • Convex quadrilateral contour extraction                          │
│  • Corner ordering: [TL, TR, BR, BL]                                │
│  • Output: DocumentCorners + deskew_angle + confidence              │
└─────────────────────────────┬───────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│  MODULE 2: GEOMETRIC RECTIFIER                                      │
│  • Euclidean dimension estimation from corner pairs                 │
│  • Aspect ratio snap to A4 (1:√2) / Letter (1:1.294)               │
│  • Homography 3×3 computation (nalgebra SVD)                        │
│  • Inverse perspective warp with bilinear interpolation             │
│  • Fine deskew via Hough line analysis (optional)                   │
│  • Output: Rectified RgbImage                                       │
└─────────────────────────────┬───────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│  MODULE 3: ILLUMINATION NORMALIZER                                  │
│  • RGB → CIE LAB conversion (D65 illuminant)                        │
│  • Background illumination map via fast box blur / bilateral filter │
│  • Illumination division: L*_norm = (L* / L*_bg) × L*_target       │
│  • Preserve a*, b* channels (red stamps, blue ink, paper tint)     │
│  • Optional CLAHE for local contrast enhancement                    │
│  • LAB → RGB conversion                                             │
│  • Output: Normalized RgbImage                                      │
└─────────────────────────────┬───────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│  MODULE 4: DETAIL ENHANCER                                          │
│  • Thresholded Unsharp Masking on L* channel                        │
│  • Formula: I_sharp = I + α × mask ⊙ (I - I_gaussian)              │
│  • Mask = |I - I_gaussian| > threshold (prevents halo artifacts)   │
│  • Barcode region detection + targeted sharpening                   │
│  • Fine text / signature enhancement via Laplacian sharpening       │
│  • Output: Enhanced RgbImage                                        │
└─────────────────────────────┬───────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│  MODULE 5: PDF EXPORTER                                             │
│  • Embed processed image into PDF page (A4/Letter @ target DPI)    │
│  • JPEG (lossy) or FlateDecode (lossless) encoding options         │
│  • Optional parallel PNG/JPG raster export                          │
│  • JSON metadata serialization (ScanResult)                         │
│  • Output: .pdf + optional .png/.jpg + .json                        │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Module Responsibilities

| Module | File | Primary Responsibility | Key Dependencies |
|--------|------|------------------------|------------------|
| Page Detector | `src/modules/page_detector.rs` | Document boundary detection, corner extraction | `imageproc`, `image` |
| Geometric Rectifier | `src/modules/geometric_rectifier.rs` | Perspective correction, dimension normalization | `nalgebra`, `image` |
| Illumination Normalizer | `src/modules/illumination_normalizer.rs` | Shadow removal, luminance equalization | `palette`, `image` |
| Detail Enhancer | `src/modules/detail_enhancer.rs` | Sharpening, artifact suppression | `imageproc`, `image` |
| PDF Exporter | `src/modules/pdf_exporter.rs` | PDF generation, metadata output | `pdf-writer`, `serde` |

---

## Shared Utilities

| Utility | File | Purpose |
|---------|------|---------|
| Color Space | `src/utils/color.rs` | sRGB ↔ CIE LAB conversions, fast box blur, bilateral filter |
| Homography | `src/utils/homography.rs` | 3×3 matrix computation, inverse perspective warp (bilinear) |
| Models | `src/models.rs` | Data structures: `DocumentCorners`, `ScanResult`, `PipelineConfig` |

---

## Pipeline Orchestration

### `ScannerPipeline` (src/pipeline.rs)

```rust
pub struct ScannerPipeline {
    config: PipelineConfig,
    page_detector: PageDetector,
    geometric_rectifier: GeometricRectifier,
    illumination_normalizer: IlluminationNormalizer,
    detail_enhancer: DetailEnhancer,
    pdf_exporter: PdfExporter,
}
```

**Execution Flow**:
1. Load image → `DynamicImage` → `RgbImage`
2. Run Module 1 → `DocumentCorners`
3. Run Module 2 → Rectified `RgbImage`
4. Run Module 3 → Normalized `RgbImage`
5. Run Module 4 → Enhanced `RgbImage`
6. Run Module 5 → Output files + `ScanResult`

**Batch Processing**: Uses `rayon::par_iter()` for directory-level parallelism.

---

## CLI Interface (src/main.rs)

```bash
my-open-scanner [OPTIONS] <INPUT_PATH> [OUTPUT_DIR]
```

### Options
| Flag | Description | Default |
|------|-------------|---------|
| `-d, --dpi` | Output DPI (200/300) | 300 |
| `-f, --format` | Output format: pdf, png, jpg, both | pdf |
| `-j, --jobs` | Parallel jobs (0 = auto) | 0 |
| `-c, --config` | TOML config file path | - |
| `--no-deskew` | Disable fine deskew | false |
| `--no-enhance` | Disable detail enhancement | false |
| `-v, --verbose` | Verbose logging | false |

---

## Data Flow Types

```
Input:  RgbImage (original, ~8-12MP)
         │
         ▼
Thumb:  RgbImage (max 800px, for detection)
         │
         ▼
Corners: DocumentCorners { tl, tr, br, bl: Point2<f32> }
         │
         ▼
Rect:    RgbImage (rectified, normalized aspect)
         │
         ▼
Norm:    RgbImage (illumination normalized, LAB-corrected)
         │
         ▼
Enhanced: RgbImage (sharpened, artifact-free)
         │
         ▼
Output:  PDF document + JSON metadata
```

---

## Error Handling Strategy

- **Recoverable**: Fallback strategies at each stage (e.g., hull approximation, min-area rect, image bounds)
- **Non-recoverable**: Return `ScanResult` with `status = Error` and `error_message`
- **No panics**: All `unwrap()`/`expect()` replaced with `?` operator and `anyhow::Result`

---

## Configuration

### PipelineConfig (TOML-serializable)

```toml
target_dim = 800                    # Detection thumbnail size
min_area_ratio = 0.12               # Min contour area / image area
snap_to_standard_aspect = true      # Snap to A4/Letter
target_luminance = 230              # Target L* after normalization
clahe_clip_limit = 1.5              # CLAHE clip limit (0 = disabled)
unsharp_amount = 0.8                # Unsharp mask strength
unsharp_radius = 1.2                # Gaussian sigma for unsharp
unsharp_threshold = 8               # Threshold for halo suppression
barcode_sharpen_strength = 0.3      # Extra sharpening for barcode regions
output_dpi = 300                    # PDF output DPI
output_format = "pdf"               # pdf | png | jpg | both
```

---

## Concurrency Model

- **Pipeline**: Sequential per-image (data dependencies)
- **Batch**: Parallel across images via `rayon`
- **Thread Safety**: All modules are `Send + Sync`, stateless per invocation
- **Memory**: Streaming processing; no full-batch buffering

---

## Extensibility Points

1. **Custom Detectors**: Implement `PageDetector` trait
2. **Alternative Rectifiers**: Swap `GeometricRectifier` implementation
3. **Color Spaces**: Extend `color.rs` for additional color models
4. **Export Formats**: Add new exporters implementing `Exporter` trait
5. **Pre/Post Hooks**: Pipeline supports middleware via callback closures

---

## Dependencies (Cargo.toml)

| Crate | Version | Purpose |
|-------|---------|---------|
| `image` | 0.25 | Core image I/O, `RgbImage`, `DynamicImage` |
| `imageproc` | 0.25 | Canny, Hough, morphology, convolutions |
| `nalgebra` | 0.33 | Linear algebra, SVD, matrix ops |
| `palette` | 0.7 | CIE LAB color space, color conversions |
| `pdf-writer` | 0.12 | PDF generation, JPEG/Flate encoding |
| `serde` | 1.0 | Serialization for config & metadata |
| `clap` | 4.5 | CLI argument parsing |
| `rayon` | 1.10 | Data parallelism for batch processing |
| `anyhow` | 1.0 | Error handling |
| `thiserror` | 2.0 | Derive error types |

---

## Build & Test

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with example
cargo run --release -- input.jpg output/

# Batch process directory
cargo run --release -- ./docs ./output --jobs 4
```