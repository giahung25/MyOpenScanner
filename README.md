# MyOpenScanner

> High-performance document scanning engine in Rust for real-world mobile captures (invoices, receipts, contracts).

## Overview
MyOpenScanner transforms smartphone-captured documents into clean, perspective-corrected, illumination-normalized, and print-ready PDF/Image files optimized for downstream OCR and archiving.

## 5-Step Functional Pipeline
1. **Page Detector:** Edge detection, convex quadrilateral contour detection, and ordered 4-corner extraction (`[TL, TR, BR, BL]`).
2. **Geometric Rectifier:** Euclidean aspect ratio estimation, A4 snapping, and inverse bilinear perspective warp using Homography $3\times3$.
3. **Illumination Normalizer:** CIE LAB color space conversion, background estimation on $L^*$ to remove shadows while preserving chrominance $a^*, b^*$ (red stamps and blue ink signatures).
4. **Detail Enhancer:** Thresholded unsharp masking to enhance fine text (6pt–8pt) and barcodes without halo artifacts.
5. **PDF Exporter:** High-resolution DPI-accurate vector container packaging with JSON metadata contract.

## Tech Stack
- **Language:** Rust (2021 Edition)
- **Crates:** `image`, `imageproc`, `nalgebra`, `palette`, `pdf-writer`, `serde`, `clap`, `rayon`
