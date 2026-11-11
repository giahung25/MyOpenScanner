# MyOpenScanner - Data Contract Specification

This document defines the formal data contracts (schemas, types, and invariants) for all public APIs, CLI interfaces, and inter-module communication.

---

## 1. Core Data Structures

### 1.1 Point2 (nalgebra)
```rust
// nalgebra::Point2<f32>
struct Point2 {
    x: f32,
    y: f32,
}
```
**Invariants**: Finite values, no NaN/Inf.

### 1.2 Lab Color
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Lab {
    pub l: f32,  // Lightness [0, 100]
    pub a: f32,  // Green-Red axis [-128, 127] (OpenCV: +128 offset)
    pub b: f32,  // Blue-Yellow axis [-128, 127] (OpenCV: +128 offset)
}
```
**Note**: OpenCV LAB uses u8 with neutral at 128. Internal representation uses f32 with standard CIE ranges.

### 1.3 DocumentCorners
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DocumentCorners {
    pub tl: Point2<f32>,  // Top-Left
    pub tr: Point2<f32>,  // Top-Right
    pub br: Point2<f32>,  // Bottom-Right
    pub bl: Point2<f32>,  // Bottom-Left
}
```

**Ordering Invariant**: Corners MUST be in clockwise order starting from top-left: `[TL, TR, BR, BL]`.

**Coordinate Space**: Original image pixel coordinates (0 ≤ x < W, 0 ≤ y < H).

**Serialization Format** (JSON):
```json
[
  [120.0, 85.0],   // TL
  [1890.0, 110.0], // TR
  [1920.0, 2650.0],// BR
  [95.0, 2610.0]   // BL
]
```

### 1.4 ScanStatus
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Success,   // All modules completed normally
    Fallback,  // One or more fallbacks triggered (still produced output)
    Error,     // Fatal error, no valid output
}
```

### 1.5 PaperSize
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaperSize {
    A4,      // 210 × 297 mm
    Letter,  // 8.5 × 11 inch (215.9 × 279.4 mm)
    Custom,  // Non-standard aspect ratio
}
```

**Physical Dimensions**:
| Size | Width (mm) | Height (mm) | Aspect Ratio |
|------|------------|-------------|--------------|
| A4 | 210.0 | 297.0 | 1 : √2 ≈ 1.4142 |
| Letter | 215.9 | 279.4 | 1 : 1.2941 |

### 1.6 OutputFormat
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Pdf,   // PDF only
    Png,   // PNG only
    Jpg,   // JPEG only
    Both,  // PDF + raster (PNG)
}
```

---

## 2. Pipeline Configuration

### 2.1 PipelineConfig
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    // Module 1: Page Detector
    pub target_dim: u32,              // Default: 800
    pub min_area_ratio: f32,          // Default: 0.12

    // Module 2: Geometric Rectifier
    pub snap_to_standard_aspect: bool, // Default: true

    // Module 3: Illumination Normalizer
    pub target_luminance: u8,         // Default: 230
    pub clahe_clip_limit: f32,        // Default: 1.5 (0 = disabled)

    // Module 4: Detail Enhancer
    pub unsharp_amount: f32,          // Default: 0.8
    pub unsharp_radius: f32,          // Default: 1.2
    pub unsharp_threshold: u8,        // Default: 8
    pub barcode_sharpen_strength: f32, // Default: 0.3

    // Module 5: PDF Exporter
    pub output_dpi: u32,              // Default: 300 (200 or 300)
    pub output_format: OutputFormat,  // Default: Pdf
}
```

### 2.2 TOML Configuration Schema
```toml
# my_open_scanner.toml
target_dim = 800
min_area_ratio = 0.12
snap_to_standard_aspect = true
target_luminance = 230
clahe_clip_limit = 1.5
unsharp_amount = 0.8
unsharp_radius = 1.2
unsharp_threshold = 8
barcode_sharpen_strength = 0.3
output_dpi = 300
output_format = "pdf"
```

**Validation Rules**:
- `target_dim` ∈ [400, 2000]
- `min_area_ratio` ∈ (0, 0.5]
- `target_luminance` ∈ [180, 250]
- `clahe_clip_limit` ∈ [0, 10]
- `unsharp_amount` ∈ [0, 3]
- `unsharp_radius` ∈ [0.1, 5]
- `unsharp_threshold` ∈ [0, 50]
- `barcode_sharpen_strength` ∈ [0, 1]
- `output_dpi` ∈ {200, 300}

---

## 3. Processing Result

### 3.1 ScanResult (Primary Output Contract)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub file_name: String,
    pub status: ScanStatus,
    pub detected_corners: Option<DocumentCorners>,
    pub deskew_angle_deg: f32,
    pub confidence_score: f32,
    pub inferred_paper_size: PaperSize,
    pub target_dpi: u32,
    pub processing_time_ms: u64,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
}
```

### 3.2 JSON Schema (ScanResult)
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": [
    "file_name",
    "status",
    "deskew_angle_deg",
    "confidence_score",
    "inferred_paper_size",
    "target_dpi",
    "processing_time_ms"
  ],
  "properties": {
    "file_name": { "type": "string", "minLength": 1 },
    "status": { "type": "string", "enum": ["success", "fallback", "error"] },
    "detected_corners": {
      "type": ["array", "null"],
      "items": {
        "type": "array",
        "minItems": 2,
        "maxItems": 2,
        "items": { "type": "number" }
      },
      "minItems": 4,
      "maxItems": 4
    },
    "deskew_angle_deg": { "type": "number" },
    "confidence_score": { "type": "number", "minimum": 0, "maximum": 1 },
    "inferred_paper_size": { "type": "string", "enum": ["A4", "Letter", "Custom"] },
    "target_dpi": { "type": "integer", "enum": [200, 300] },
    "processing_time_ms": { "type": "integer", "minimum": 0 },
    "output_path": { "type": ["string", "null"] },
    "error_message": { "type": ["string", "null"] }
  }
}
```

### 3.3 Field Semantics

| Field | Required | Description |
|-------|----------|-------------|
| `file_name` | Yes | Basename of input file (no path) |
| `status` | Yes | Processing outcome |
| `detected_corners` | No* | Null if status=error; 4×2 array [TL,TR,BR,BL] |
| `deskew_angle_deg` | Yes | Average rotation of top/bottom edges |
| `confidence_score` | Yes | 0.0-1.0; contour quality, aspect match, corner angles |
| `inferred_paper_size` | Yes | Detected standard size |
| `target_dpi` | Yes | Output DPI used |
| `processing_time_ms` | Yes | Wall-clock pipeline duration |
| `output_path` | No* | Relative path to output file; null on error |
| `error_message` | No* | Human-readable error; null on success |

*Conditional on status.

### 3.4 Confidence Score Computation
```
confidence = w₁·area_score + w₂·angle_score + w₃·aspect_score + w₄·convexity_score

area_score = min(contour_area / (0.3 × image_area), 1.0)
angle_score = 1.0 - (mean_angle_deviation / 45°)  # Clamped [0,1]
aspect_score = 1.0 - |detected_ratio - standard_ratio| / tolerance
convexity_score = 1.0 if convex else 0.5

Weights: w₁=0.3, w₂=0.3, w₃=0.2, w₄=0.2
```

---

## 4. Module I/O Contracts

### 4.1 Page Detector
```rust
// Input
struct PageDetectorInput {
    image: RgbImage,           // Original image, any size
}

// Output
struct PageDetectorOutput {
    corners: DocumentCorners,  // In original image coordinates
    deskew_angle_deg: f32,
    status: ScanStatus,        // Success | Fallback
}
```

### 4.2 Geometric Rectifier
```rust
// Input
struct GeometricRectifierInput {
    image: RgbImage,           // Original image
    corners: DocumentCorners,  // From PageDetector
}

// Output
struct GeometricRectifierOutput {
    rectified: RgbImage,       // Rectified, normalized aspect
    homography: Matrix3<f32>,  // 3×3 transform (src→dst)
    paper_size: PaperSize,
    rectified_size: (u32, u32),
}
```

### 4.3 Illumination Normalizer
```rust
// Input
struct IlluminationNormalizerInput {
    image: RgbImage,           // Rectified image
}

// Output
struct IlluminationNormalizerOutput {
    normalized: RgbImage,      // Shadow-free, color-preserved
    luminance_stats: LuminanceStats,
}
```

```rust
struct LuminanceStats {
    mean_before: f32,
    mean_after: f32,
    std_dev_before: f32,
    std_dev_after: f32,
}
```

### 4.4 Detail Enhancer
```rust
// Input
struct DetailEnhancerInput {
    image: RgbImage,           // Normalized image
}

// Output
struct DetailEnhancerOutput {
    enhanced: RgbImage,        // Sharpened, artifact-free
}
```

### 4.5 PDF Exporter
```rust
// Input
struct PdfExporterInput {
    image: RgbImage,           // Final enhanced image
    metadata: ScanResult,      // Accumulated metadata
    config: PipelineConfig,    // For DPI, format
    output_dir: PathBuf,
    base_name: String,         // Without extension
}

// Output
struct PdfExporterOutput {
    pdf_path: PathBuf,         // Always produced if format includes PDF
    raster_path: Option<PathBuf>, // PNG/JPG if format includes raster
    json_path: PathBuf,        // Metadata JSON
}
```

---

## 5. CLI Contract

### 5.1 Command Line Interface
```bash
my-open-scanner [OPTIONS] <INPUT> [OUTPUT_DIR]
```

### 5.2 Arguments
| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `INPUT` | Path | Yes | Image file or directory |
| `OUTPUT_DIR` | Path | No | Output directory (default: `./output`) |

### 5.3 Options
| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--dpi` | `-d` | u32 (200\|300) | 300 | Output DPI |
| `--format` | `-f` | OutputFormat | pdf | Output format |
| `--jobs` | `-j` | u32 | 0 (auto) | Parallel jobs |
| `--config` | `-c` | Path | - | TOML config file |
| `--no-deskew` | | bool | false | Disable fine deskew |
| `--no-enhance` | | bool | false | Disable detail enhancer |
| `--verbose` | `-v` | bool | false | Verbose logging |

### 5.4 Exit Codes
| Code | Meaning |
|------|---------|
| 0 | Success (all images processed) |
| 1 | General error (invalid args, IO error) |
| 2 | Partial failure (some images failed) |
| 3 | Configuration error |

### 5.5 Stdout/Stderr
- **Stdout**: JSON Lines (one ScanResult per line) for successful processing
- **Stderr**: Progress logs, warnings, errors (human-readable)

---

## 6. Batch Processing Output

### 6.1 Summary JSON (batch_summary.json)
```json
{
  "total": 100,
  "success": 95,
  "fallback": 3,
  "error": 2,
  "total_time_ms": 12500,
  "avg_time_ms": 125,
  "results": [
    { ... ScanResult ... },
    ...
  ]
}
```

### 6.2 Directory Structure
```
output/
├── batch_summary.json
├── invoice_001_scanned.pdf
├── invoice_001_scanned.json
├── invoice_002_scanned.pdf
├── invoice_002_scanned.json
└── ...
```

---

## 7. Error Payloads

### 7.1 Module-Level Errors (internal)
```rust
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Image load failed: {0}")]
    ImageLoad(#[from] image::ImageError),

    #[error("No document contour found")]
    NoContour,

    #[error("Homography computation failed: singular matrix")]
    SingularHomography,

    #[error("PDF generation failed: {0}")]
    PdfGeneration(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}
```

### 7.2 Public Error Response (CLI/SDK)
```json
{
  "file_name": "corrupt.jpg",
  "status": "error",
  "detected_corners": null,
  "deskew_angle_deg": 0.0,
  "confidence_score": 0.0,
  "inferred_paper_size": "Custom",
  "target_dpi": 300,
  "processing_time_ms": 15,
  "output_path": null,
  "error_message": "Image load failed: unsupported format"
}
```

---

## 8. Versioning & Compatibility

### 8.1 Contract Version
Current: `v1.0.0` (embedded in ScanResult as optional field `contract_version`)

### 8.2 Compatibility Rules
- **Breaking**: Field removal, type change, enum variant removal
- **Non-breaking**: New optional fields, new enum variants (with `#[non_exhaustive]`), relaxed validation
- **Serialization**: Use `serde(default)` for new fields

### 8.3 Migration Path
Future versions will include `contract_version` field. Consumers should:
1. Check version field
2. Handle missing optional fields gracefully
3. Treat unknown enum variants as `Custom`/`Error` fallback

---

## 9. Performance Contracts (SLA)

| Metric | Target | Measurement |
|--------|--------|-------------|
| Single image latency (8MP→A4@300DPI) | < 150ms | p95 on i7-12700 / Ryzen 7 5800X |
| Memory peak | < 500MB | RSS during processing |
| Batch throughput (4 cores) | > 20 img/s | Sustained on SSD |
| Detection accuracy (clean docs) | > 99% | Corner IoU > 0.9 |
| Detection accuracy (cluttered) | > 95% | Corner IoU > 0.85 |
| Color fidelity (ΔE*ab) | < 2.0 | vs. reference flatbed scan |