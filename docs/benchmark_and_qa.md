# MyOpenScanner - Benchmark & Quality Assurance

This document defines the benchmarking methodology, quality metrics, test datasets, and acceptance criteria for MyOpenScanner.

---

## 1. Benchmark Methodology

### 1.1 Hardware Reference Platforms

| Platform | CPU | RAM | OS | Purpose |
|----------|-----|-----|-----|---------|
| **Ref-Desktop** | Intel Core i7-12700 (12C/20T) | 32GB DDR5 | Ubuntu 22.04 / Win11 | Primary benchmark |
| **Ref-Laptop** | AMD Ryzen 7 7840HS (8C/16T) | 16GB LPDDR5 | Ubuntu 24.04 | Mobile/thermal test |
| **Ref-CI** | GitHub Actions ubuntu-latest (2 vCPU) | 7GB | Ubuntu 22.04 | CI regression |

### 1.2 Build Configuration
```bash
# Release build with target-cpu=native
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Profile-guided optimization (optional)
cargo build --release --features pgo
```

### 1.3 Measurement Protocol
- **Warmup**: 5 iterations discarded
- **Sample size**: 100 iterations per image
- **Metrics**: Mean, median, p50, p95, p99, min, max
- **Isolation**: Single-threaded measurement; disable ASLR, turbo boost fixed
- **Timing**: `std::time::Instant` around full pipeline; exclude I/O

---

## 2. Performance Benchmarks

### 2.1 Latency Targets (Single Image, 8MP → A4@300DPI)

| Stage | Target (ms) | Budget % | Notes |
|-------|-------------|----------|-------|
| Load & Decode | ≤ 15 | 10% | JPEG decode dominates |
| Page Detection | ≤ 35 | 23% | Thumbnail 800px |
| Geometric Rectify | ≤ 30 | 20% | Warp + fine deskew |
| Illumination Norm | ≤ 25 | 17% | Box blur + bilateral |
| Detail Enhance | ≤ 20 | 13% | Convolutions |
| PDF Export | ≤ 20 | 13% | JPEG encode |
| **Total** | **≤ 150** | **100%** | p95 on Ref-Desktop |

### 2.2 Throughput Targets (Batch, 4 workers)

| Configuration | Target (img/s) | Notes |
|---------------|----------------|-------|
| Ref-Desktop (12C) | ≥ 30 | Sustained, SSD I/O |
| Ref-Laptop (8C) | ≥ 18 | Thermal throttling considered |
| Ref-CI (2C) | ≥ 6 | CI baseline |

### 2.3 Memory Profile

| Metric | Target |
|--------|--------|
| Peak RSS (single) | < 300 MB |
| Peak RSS (batch ×4) | < 800 MB |
| Allocation rate | < 50 MB/s |
| No memory leaks | Verified via `valgrind` / `cargo-instruments` |

### 2.4 Scaling Characteristics

| Input Resolution | Expected Latency (ms) | Memory (MB) |
|-----------------|----------------------|-------------|
| 2MP (1600×1200) | ~60 | ~100 |
| 5MP (2592×1944) | ~100 | ~180 |
| 8MP (3264×2448) | ~150 | ~300 |
| 12MP (4000×3000) | ~220 | ~450 |
| 16MP (4608×3456) | ~300 | ~600 |

*Approximately linear with output pixel count (rectified A4@300DPI ≈ 2550×3300 ≈ 8.4MP).*

---

## 3. Quality Metrics

### 3.1 Geometric Accuracy

| Metric | Definition | Target |
|--------|------------|--------|
| **Corner IoU** | IoU(gt_quad, pred_quad) | > 0.95 (clean), > 0.85 (cluttered) |
| **Corner L2 Error** | Mean ||gt - pred||₂ (px) | < 3px @ 8MP |
| **Aspect Ratio Error** | \|detected - standard\| / standard | < 2% |
| **Deskew Residual** | \|angle_after\| | < 0.2° |
| **Line Straightness** | Max deviation from fitted line | < 1px per 100px |

### 3.2 Photometric Quality

| Metric | Definition | Target |
|--------|------------|--------|
| **Background Uniformity** | StdDev(L*) in flat regions | < 5 (0-255 scale) |
| **Shadow Removal** | Mean L* in shadow vs non-shadow | Ratio > 0.9 |
| **Color Fidelity (ΔE*ab)** | CIEDE2000 vs flatbed reference | < 2.0 |
| **Red Stamp Preservation** | Δa* in stamp region | < 5 |
| **Blue Ink Preservation** | Δb* in ink region | < 5 |
| **Paper Tint Preservation** | ΔE*ab in blank paper area | < 3.0 |

### 3.3 Sharpness & Artifact Metrics

| Metric | Definition | Target |
|--------|------------|--------|
| **Text MTF** | Modulation at 6pt font Nyquist | > 0.4 |
| **Barcode Decodability** | ZXing decode success rate | 100% (clean), > 95% (noisy) |
| **Halo Artifact Index** | Max overshoot at edges | < 5% of edge contrast |
| **Noise Amplification** | StdDev(flat) after / before | < 1.5× |

### 3.4 OCR Readiness (Downstream)

| Metric | Target |
|--------|--------|
| Tesseract 4.x char accuracy (Latin) | > 99% |
| Tesseract 4.x char accuracy (Vietnamese) | > 97% |
| PaddleOCR char accuracy | > 98% |
| Layout preservation (table structure) | > 95% IoU |

---

## 4. Test Datasets

### 4.1 Dataset Categories

| Category | Count | Description | Source |
|----------|-------|-------------|--------|
| **Clean Studio** | 50 | Flatbed scans, perfect lighting | Synthetic / curated |
| **Phone Camera - Good** | 100 | Well-lit, flat surface, minimal skew | Internal collection |
| **Phone Camera - Challenging** | 100 | 35° skew, wrinkles, shadows, clutter | Internal + public |
| **Adverse Lighting** | 50 | Strong shadows, yellow/blue cast, low light | Internal |
| **Colored Paper** | 50 | Yellow, pink, blue, green paper forms | Government forms |
| **Mixed Content** | 50 | Tables, barcodes, handwriting, stamps | Real invoices |
| **Edge Cases** | 20 | Receipts (long), ID cards, business cards | Various |

**Total**: 420 images with ground truth.

### 4.2 Ground Truth Annotations

Each test image includes:
```json
{
  "file_name": "test_001.jpg",
  "gt_corners": [[x,y], [x,y], [x,y], [x,y]],  // Sub-pixel accurate
  "gt_paper_size": "A4",
  "gt_dpi": 300,
  "categories": ["skew_15", "wrinkle", "shadow_mild"],
  "flatbed_reference": "test_001_flatbed.tiff"  // Optional
}
```

### 4.3 Public Benchmark Datasets (for comparison)
- **MIDV-2020**: Document video dataset (use keyframes)
- **Tobacco-800**: Document classification (repurpose for detection)
- **ICDAR 2019**: Table detection (table-heavy docs)
- **SROIE**: Receipt OCR (long receipts)

---

## 5. Automated Test Suite

### 5.1 Unit Tests (cargo test)
```bash
# Run all unit tests
cargo test --all

# Specific module
cargo test page_detector::
cargo test geometric_rectifier::
cargo test illumination_normalizer::
cargo test detail_enhancer::
cargo test pdf_exporter::
cargo test utils::
```

**Coverage Target**: ≥ 90% line coverage (via `cargo tarpaulin`)

### 5.2 Integration Tests (tests/integration_tests.rs)
```rust
#[test]
fn test_full_pipeline_clean_image() { ... }
#[test]
fn test_full_pipeline_skewed_image() { ... }
#[test]
fn test_batch_processing() { ... }
#[test]
fn test_output_pdf_valid() { ... }
#[test]
fn test_metadata_schema_compliance() { ... }
```

### 5.3 Property-Based Tests (proptest)
```rust
proptest! {
    #[test]
    fn homography_roundtrip(corners in valid_quad_strategy()) {
        let h = compute_homography(&corners, &dst);
        let h_inv = compute_homography(&dst, &corners);
        // h * h_inv ≈ Identity
    }
}
```

### 5.4 Regression Tests (Golden Master)
- Store reference outputs for 50 canonical images
- Compare pixel-wise (SSIM > 0.99) and metadata-wise
- Fail on any deviation beyond tolerance

---

## 6. CI/CD Pipeline

### 6.1 GitHub Actions Workflow (`.github/workflows/ci.yml`)

```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin
            ~/.cargo/registry
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      - name: Run tests
        run: cargo test --all --workspace
      - name: Check formatting
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --all -- -D warnings

  bench:
    runs-on: ubuntu-latest
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - ...
      - name: Run benchmarks
        run: cargo bench --bench pipeline_bench
      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: target/criterion/

  release:
    needs: [test, bench]
    if: startsWith(github.ref, 'refs/tags/v')
    runs-on: ubuntu-latest
    steps:
      - ...
      - name: Build release
        run: cargo build --release
      - name: Create release
        uses: softprops/action-gh-release@v1
```

### 6.2 Benchmark Tracking
- Use `cargo bench` with `criterion`
- Store results as artifacts
- Compare against baseline (main branch)
- Fail CI if p95 latency regresses > 10%

---

## 7. Acceptance Criteria (Definition of Done)

### 7.1 Functional Acceptance

| Criterion | Verification Method |
|-----------|---------------------|
| All 5 modules compile & link | `cargo build --release` |
| CLI processes single image | `my-open-scanner test.jpg out/` |
| CLI processes directory | `my-open-scanner ./docs ./out -j4` |
| Output PDF opens in standard viewers | Manual: Adobe, Edge, Preview, Okular |
| Output JSON validates against schema | `jsonschema -i result.json schema.json` |
| Metadata matches specification | Automated field validation |

### 7.2 Performance Acceptance

| Criterion | Threshold | Measurement |
|-----------|-----------|-------------|
| Single image p95 latency | < 150ms | Ref-Desktop, 8MP input |
| Batch throughput (4 jobs) | > 20 img/s | Ref-Desktop |
| Peak memory (single) | < 300MB | `valgrind --tool=massif` |
| No regressions vs baseline | < 10% slower | CI benchmark comparison |

### 7.3 Quality Acceptance

| Criterion | Threshold | Test Set |
|-----------|-----------|----------|
| Corner IoU (clean) | > 0.95 | Clean Studio (50) |
| Corner IoU (challenging) | > 0.85 | Challenging (100) |
| Color ΔE*ab | < 2.0 | Colored Paper (50) |
| Barcode decode rate | 100% | Mixed Content (50) |
| OCR char accuracy (VN) | > 97% | Vietnamese subset |

### 7.4 Code Quality Acceptance

| Criterion | Tool | Threshold |
|-----------|------|-----------|
| Zero clippy warnings | `cargo clippy` | 0 |
| Formatted code | `cargo fmt` | Pass |
| Test coverage | `cargo tarpaulin` | ≥ 90% |
| No unsafe without audit | `cargo geiger` | 0 unaudited |
| Dependency audit | `cargo audit` | 0 vulnerabilities |

---

## 8. Profiling & Optimization Guide

### 8.1 CPU Profiling
```bash
# perf (Linux)
perf record -g -- cargo run --release -- input.jpg out/
perf report

# Instruments (macOS)
xcrun xctrace record --template "Time Profiler" -- cargo run --release -- input.jpg out/

# samply (cross-platform)
cargo install samply
samply record cargo run --release -- input.jpg out/
```

### 8.2 Key Hotspots to Monitor
1. **Canny edge detection** (imageproc) - SIMD optimized
2. **Bilateral filter** (illumination) - O(N×r²), consider separable approx
3. **Bilinear warp** (rectifier) - Memory bound, cache-friendly loops
4. **JPEG encode/decode** - Use `mozjpeg` / `libjpeg-turbo`
5. **PDF writing** - Stream, don't buffer full image

### 8.3 Optimization Checklist
- [ ] `target-cpu=native` enabled
- [ ] LTO enabled (`lto = "fat"` in Cargo.toml)
- [ ] `codegen-units = 1` for release
- [ ] Profile-guided optimization (PGO) for hot paths
- [ ] Parallel rayon for batch only (not per-image)
- [ ] Avoid unnecessary clones; use `Arc<ImageBuffer>` if sharing

---

## 9. Release Checklist

### 9.1 Pre-Release
- [ ] All CI checks pass
- [ ] Benchmarks within targets
- [ ] Test dataset passes quality gates
- [ ] CHANGELOG.md updated
- [ ] Version bumped in Cargo.toml
- [ ] Documentation updated

### 9.2 Release Artifacts
- [ ] GitHub Release with binaries (Linux x86_64, Windows x86_64, macOS arm64/x86_64)
- [ ] crates.io publish (`cargo publish`)
- [ ] Docker image (optional)
- [ ] Homebrew formula (optional)

### 9.3 Post-Release
- [ ] Verify crates.io download
- [ ] Verify binary checksums
- [ ] Announce (GitHub Discussions, Discord, etc.)
- [ ] Monitor issue tracker for regressions

---

## 10. Known Limitations & Future Work

### 10.1 Current Limitations
| Limitation | Impact | Mitigation |
|------------|--------|------------|
| No multi-page PDF | Single page per input | Batch → combine externally |
| No auto-rotation (90° steps) | Portrait/landscape swap | EXIF orientation handled |
| No table structure extraction | Downstream OCR harder | Export rectified image for OCR |
| No GPU acceleration | CPU only | Future: `wgpu` / `vulkano` compute |

### 10.2 Planned Enhancements
1. **v0.2**: Multi-page PDF, auto-rotate, EXIF orientation
2. **v0.3**: GPU-accelerated bilateral/warp, WASM build for web
3. **v0.4**: Table detection export (JSON + CSV), barcode decoding built-in
4. **v1.0**: Stable API, plugin architecture, language bindings (Python, Node.js)

---

## 11. Troubleshooting Benchmarks

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| Latency > 200ms | Debug build / no LTO | `cargo build --release` |
| High memory | Image buffering | Stream processing, drop early |
| Variable latency | Thermal throttling | Fix CPU governor, cool down |
| Low corner IoU | Threshold tuning | Adjust `min_area_ratio`, `target_dim` |
| Color shift | LAB conversion bug | Verify D65 matrix, clamping |
| Halo artifacts | Unsharp threshold too low | Increase `unsharp_threshold` |