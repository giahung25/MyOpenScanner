# MyOpenScanner - Algorithm Documentation

This document provides detailed mathematical and algorithmic descriptions for each pipeline module.

---

## Module 1: Page Detector

### Objective
Detect the four corners of a document in a cluttered scene with perspective distortion up to 35°, wrinkles, and complex backgrounds.

### Algorithm Pipeline

#### 1.1 Multi-Scale Preprocessing
```
Input: I_orig (H × W × 3, BGR)
Scale factor: s = min(1, 800 / max(H, W))
I_proc = resize(I_orig, s) using INTER_AREA
```

#### 1.2 Edge Detection
```
Gray = cvtColor(I_proc, BGR2GRAY)
Blurred = bilateralFilter(Gray, d=9, σ_color=75, σ_space=75)

# Adaptive Canny thresholds via Otsu
high_thresh = otsu_threshold(Blurred)
low_thresh = 0.4 × high_thresh
Edges = Canny(Blurred, low_thresh, high_thresh)
```

#### 1.3 Morphological Closing (Gap Bridging)
```
Kernel = getStructuringElement(MORPH_RECT, 7×7)
Closed = morphologyEx(Edges, MORPH_CLOSE, Kernel)
```

#### 1.4 Contour Extraction & Filtering
```
Contours = findContours(Closed, RETR_LIST, CHAIN_APPROX_SIMPLE)
Contours_sorted = sort_by_area_desc(Contours)

Min_area = H_proc × W_proc × min_area_ratio (default 0.12)
```

#### 1.5 Quadrilateral Detection
For each contour with area ≥ Min_area:
```
Perimeter = arcLength(contour, closed=true)

For ε_factor in [0.015, 0.02, 0.03, 0.04]:
    Approx = approxPolyDP(contour, ε_factor × Perimeter, true)
    If len(Approx) == 4 and isContourConvex(Approx):
        If is_valid_quad(Approx):
            Return Approx as corners
```

#### 1.6 Fallback Strategies
**Level 1 - Convex Hull**:
```
Hull = convexHull(largest_contour)
Approx = approxPolyDP(Hull, ε ∈ [0.02, 0.03, 0.05])
```

**Level 2 - Min Area Rectangle**:
```
Rect = minAreaRect(largest_contour)
Box = boxPoints(Rect)  # 4 corners
```

**Level 3 - Image Bounds**:
```
Margin = 2% of image dimension
Corners = [[margin, margin], [W-margin, margin], [W-margin, H-margin], [margin, H-margin]]
```

#### 1.7 Corner Ordering (TL, TR, BR, BL)
```
pts = 4×2 array of corner coordinates

# Sum heuristic: TL has min(x+y), BR has max(x+y)
s = pts.sum(axis=1)
tl = pts[argmin(s)]
br = pts[argmax(s)]

# Difference heuristic: TR has min(y-x), BL has max(y-x)
diff = pts[:,1] - pts[:,0]
tr = pts[argmin(diff)]
bl = pts[argmax(diff)]

Ordered = [tl, tr, br, bl]
```

#### 1.8 Quadrilateral Validation
```
For each vertex i:
    p1 = pts[i], p2 = pts[(i+1)%4], p3 = pts[(i+2)%4]
    v1 = p1 - p2, v2 = p3 - p2
    cos_angle = (v1·v2) / (|v1| × |v2|)
    angle = arccos(clamp(cos_angle, -1, 1)) × 180/π

    If angle < 35° or angle > 145°: INVALID
```

#### 1.9 Deskew Angle Calculation
```
angle_top = atan2(tr.y - tl.y, tr.x - tl.x)
angle_bottom = atan2(br.y - bl.y, br.x - bl.x)
deskew_angle = (angle_top + angle_bottom) / 2  # in degrees
```

#### 1.10 Coordinate Scaling
```
If scale != 1.0:
    corners = corners / scale  # Map back to original image coordinates
Clip to [0, W-1] × [0, H-1]
```

---

## Module 2: Geometric Rectifier

### Objective
Rectify perspective distortion and normalize aspect ratio to standard paper sizes.

### 2.1 Target Dimension Estimation
```
Given ordered corners: tl, tr, br, bl

width_top = ||tr - tl||₂
width_bottom = ||br - bl||₂
max_width = max(width_top, width_bottom)

height_left = ||bl - tl||₂
height_right = ||br - tr||₂
max_height = max(height_left, height_right)
```

### 2.2 Aspect Ratio Normalization
```
ratio = max_height / max_width

# A4: 1 : √2 ≈ 1.4142 (tolerance ±6%)
If 1.33 ≤ ratio ≤ 1.50:
    max_height = max_width × √2
    paper_size = A4

# Letter: 1 : 1.2941 (tolerance ±5%)
Else if 1.22 ≤ ratio ≤ 1.33:
    max_height = max_width × 1.2941
    paper_size = Letter

Else:
    paper_size = Custom
```

### 2.3 Homography Computation (DLT + SVD)

#### Source Points (detected corners)
```
src = [tl, tr, br, bl]  # 4 × 2
```

#### Destination Points (rectified rectangle)
```
dst = [[0, 0], [W-1, 0], [W-1, H-1], [0, H-1]]
```

#### Direct Linear Transform (8 equations, 9 unknowns)
For each correspondence (x, y) → (x', y'):
```
[ -x  -y  -1   0   0   0  x·x'  y·x'  x' ] [h₁]   [0]
[  0   0   0  -x  -y  -1  x·y'  y·y'  y' ] [h₂] = [0]
                                                         ...
                                                         [h₉]
```

Stack 8 rows → A (8×9). Solve Ah = 0 via SVD:
```
A = U Σ Vᵀ
h = last column of V  (corresponding to smallest singular value)
H = reshape(h, 3×3)
```

### 2.4 Inverse Perspective Warp (Bilinear Interpolation)

For each output pixel (x_dst, y_dst):
```
# Homogeneous coordinates
p_dst = [x_dst + 0.5, y_dst + 0.5, 1]ᵀ

# Map to source
p_src_h = H⁻¹ × p_dst
x_src = p_src_h.x / p_src_h.z
y_src = p_src_h.y / p_src_h.z

# Bilinear sampling
x₀ = floor(x_src), y₀ = floor(y_src)
x₁ = min(x₀+1, W-1), y₁ = min(y₀+1, H-1)
dx = x_src - x₀, dy = y_src - y₀

I_dst = (1-dx)(1-dy)I(x₀,y₀) + dx(1-dy)I(x₁,y₀)
       + (1-dx)dy I(x₀,y₁) + dx dy I(x₁,y₁)
```

Border mode: REPLICATE (clamp to edge)

### 2.5 Fine Deskew (Optional)

#### Horizontal Line Extraction
```
Gray = cvtColor(rectified, BGR2GRAY)
Thresh = adaptiveThreshold(Gray, GAUSSIAN_C, THRESH_BINARY_INV, 15, 8)
Kernel = getStructuringElement(MORPH_RECT, 25×1)  # Horizontal
Lines = morphologyEx(Thresh, MORPH_OPEN, Kernel)
```

#### Hough Line Angle Estimation
```
Lines = HoughLinesP(Lines, ρ=1, θ=π/180, threshold=100,
                    minLineLength=W/8, maxLineGap=10)

For each line (x₁,y₁,x₂,y₂):
    angle = atan2(y₂-y₁, x₂-x₁) × 180/π
    If |angle| ≤ 3°: collect angle

If count ≥ 5:
    median_angle = median(collected_angles)
    If |median_angle| ≥ 0.2°:
        Rotate image by median_angle around center
```

---

## Module 3: Illumination Normalizer

### Objective
Remove shadows and uneven illumination while preserving paper tint, red stamps, and blue ink.

### 3.1 Color Space Conversion: sRGB → CIE LAB

#### sRGB → Linear RGB
```
For each channel c ∈ {R, G, B} normalized to [0,1]:
    If c ≤ 0.04045: c_lin = c / 12.92
    Else: c_lin = ((c + 0.055) / 1.055)²·⁴
```

#### Linear RGB → XYZ (D65)
```
[ X ]   [ 0.4124564  0.3575761  0.1804375 ] [ R_lin ]
[ Y ] = [ 0.2126729  0.7151522  0.0721750 ] [ G_lin ]
[ Z ]   [ 0.0193339  0.1191920  0.9503041 ] [ B_lin ]
```

#### XYZ → LAB (D65 reference: Xₙ=95.047, Yₙ=100.0, Zₙ=108.883)
```
f(t) = t^(1/3)                    if t > ε
       (κ·t + 16)/116             otherwise
where ε = 216/24389, κ = 24389/27

f_x = f(X/Xₙ), f_y = f(Y/Yₙ), f_z = f(Z/Zₙ)

L* = 116·f_y - 16
a* = 500·(f_x - f_y)
b* = 200·(f_y - f_z)
```

### 3.2 Background Illumination Estimation

#### Fast Box Blur (Integral Image) - O(1) per pixel
```
Integral image: S(x,y) = Σ_{i≤x, j≤y} I(i,j)

For radius r:
    Box at (x,y): sum = S(x+r,y+r) - S(x-r-1,y+r)
                           - S(x+r,y-r-1) + S(x-r-1,y-r-1)
    Blur(x,y) = sum / (2r+1)²
```

Adaptive kernel: `r = max(15, min(H,W) × 0.025)`

#### Bilateral Filter (Edge-Preserving)
```
I_bilateral(p) = 1/W_p Σ_{q∈N(p)} G_σs(||p-q||) · G_σr(|I(p)-I(q)|) · I(q)

G_σs(d) = exp(-d² / 2σs²)     # Spatial Gaussian
G_σr(d) = exp(-d² / 2σr²)     # Range Gaussian

Default: σs = kernel_size/2, σr = 15.0 (in L* scale 0-100)
```

### 3.3 Illumination Division (Retinex-inspired)
```
L_bg = bilateral_filter(L_channel, σs, σr)
L_bg_smooth = gaussian_blur(L_bg, σ = kernel_size/2)

# Target luminance from 92nd percentile of background
L_target = clip(percentile(L_bg_smooth, 92), 210, target_luminance)

# Normalization
L_norm(x,y) = clip( L(x,y) / max(L_bg_smooth(x,y), 1.0) × L_target, 0, 255 )
```

### 3.4 Local Contrast Enhancement (CLAHE)
```
If clahe_clip_limit > 0:
    L_clahe = CLAHE(L_norm, clip_limit, tile_grid=8×8)
    L_final = 0.4 × L_norm + 0.6 × L_clahe
Else:
    L_final = L_norm
```

### 3.5 Chromaticity Preservation & Enhancement

#### Red Stamp Enhancement (a* channel)
```
# Red stamp: a* >> 0 (typically > 132 in OpenCV LAB where neutral=128)
red_mask = (a* > 132).astype(float)
a_enhanced = a* + red_mask × (a* - 128) × 0.15
```

#### Blue Ink Preservation (b* channel)
```
# Blue ink: b* << 128 (typically < 124)
blue_mask = (b* < 124).astype(float)
b_enhanced = b* - blue_mask × (128 - b*) × 0.15
```

### 3.6 LAB → sRGB (Inverse Transform)

#### LAB → XYZ
```
f_y = (L* + 16) / 116
f_x = a*/500 + f_y
f_z = f_y - b*/200

f⁻¹(t) = t³                    if t³ > ε
         (116·t - 16)/κ        otherwise

X = Xₙ × f⁻¹(f_x), Y = Yₙ × f⁻¹(f_y), Z = Zₙ × f⁻¹(f_z)
```

#### XYZ → Linear RGB
```
[ R_lin ]   [  3.2404542  -1.5371385  -0.4985314 ] [ X ]
[ G_lin ] = [ -0.9692660   1.8760108   0.0415560 ] [ Y ]
[ B_lin ]   [  0.0556434  -0.2040259   1.0572252 ] [ Z ]
```

#### Linear RGB → sRGB
```
If c_lin ≤ 0.0031308: c = 12.92 × c_lin
Else: c = 1.055 × c_lin^(1/2.4) - 0.055

Clip to [0, 1], scale to [0, 255]
```

---

## Module 4: Detail Enhancer

### Objective
Sharpen fine text (6-8pt), signatures, and barcodes while suppressing halo artifacts.

### 4.1 Thresholded Unsharp Masking

#### Gaussian Blur
```
L_blur = gaussian_blur(L_channel, σ = unsharp_radius)
```

#### Difference Map
```
Δ = L_channel - L_blur
```

#### Adaptive Mask
```
mask(x,y) = 1  if |Δ(x,y)| > threshold
            0  otherwise
```

#### Sharpening
```
L_sharp = L_channel + amount × mask ⊙ Δ
        = L_channel + amount × mask ⊙ (L_channel - L_blur)

Clip to [0, 255]
```

**Parameters**: amount=0.8, radius=1.2, threshold=8

**Effect**: Only edges with contrast > threshold get sharpened → no halo on flat regions.

### 4.2 Barcode Region Detection & Enhancement

#### 1D Barcode Detection (Horizontal Edge Density)
```
Sobel_x = ∂I/∂x, Sobel_y = ∂I/∂y
Mag = √(Sobel_x² + Sobel_y²)
Dir = atan2(Sobel_y, Sobel_x) × 180/π  # 0° = horizontal gradient

# Barcode bars → vertical edges → horizontal gradient (90°)
barcode_mask_1d = (Dir > 70°) ∧ (Dir < 110°) ∧ (Mag > 30)
```

#### 2D Barcode Detection (High-Frequency Pattern)
```
Binary = otsu_threshold(Gray)
TopHat = morphologyEx(Binary, MORPH_TOPHAT, 3×3)
BlackHat = morphologyEx(Binary, MORPH_BLACKHAT, 3×3)
barcode_mask_2d = (TopHat > 0) ∨ (BlackHat > 0)
```

#### Combined Mask & Dilation
```
barcode_mask = dilate(barcode_mask_1d ∨ barcode_mask_2d, 5×5, iterations=2)
```

#### Targeted Sharpening
```
L_barcode_sharp = L + barcode_strength × barcode_mask ⊙ (L - gaussian_blur(L, σ=0.8))
```

### 4.3 Fine Text & Signature Enhancement

#### Laplacian Sharpening Kernel
```
K = [  0  -1   0 ]
    [ -1   5  -1 ]
    [  0  -1   0 ]
```

#### Text Region Mask
```
L_smooth = gaussian_blur(L, 5×5, σ=1.0)
text_mask = (|L - L_smooth| > 15).astype(float)
```

#### Blended Sharpening
```
L_laplacian = filter2D(L, K)
L_final = L + 0.15 × text_mask ⊙ (L_laplacian - L)
```

---

## Module 5: PDF Exporter

### Objective
Package processed image into standards-compliant PDF with correct DPI and dimensions.

### 5.1 Page Dimensions Calculation
```
Paper size → Physical dimensions (mm):
    A4: 210 × 297 mm
    Letter: 215.9 × 279.4 mm

Target DPI (200 or 300) → Pixel dimensions:
    W_px = round(W_mm / 25.4 × DPI)
    H_px = round(H_mm / 25.4 × DPI)
```

### 5.2 Image Scaling & Placement
```
Input image: (w, h) pixels
Target page: (W_px, H_px) pixels

Scale = min(W_px/w, H_px/h)  # Fit within page, maintain aspect
Scaled: (w×Scale, h×Scale)

Position: Centered
    x_offset = (W_px - w×Scale) / 2
    y_offset = (H_px - h×Scale) / 2
```

### 5.3 PDF Content Stream
```
1. Define page with MediaBox [0, 0, W_pt, H_pt] where 1 pt = 1/72 inch
   W_pt = W_mm × 72/25.4, H_pt = H_mm × 72/25.4

2. Draw image XObject:
   - ColorSpace: DeviceRGB
   - Filter: DCTDecode (JPEG, quality=95) or FlateDecode (lossless)
   - BitsPerComponent: 8
   - Width/Height: scaled pixel dimensions

3. Transform matrix for placement:
   [ w_scale  0        x_offset ]
   [ 0        h_scale  y_offset ]
   [ 0        0        1        ]
```

### 5.4 Metadata JSON (ScanResult)
```json
{
  "file_name": "string",
  "status": "success|fallback|error",
  "detected_corners": [[x,y], [x,y], [x,y], [x,y]],
  "deskew_angle_deg": float,
  "confidence_score": float,
  "inferred_paper_size": "A4|Letter|Custom",
  "target_dpi": integer,
  "processing_time_ms": integer,
  "output_path": "string|null",
  "error_message": "string|null"
}
```

---

## Computational Complexity

| Module | Time Complexity | Space Complexity | Notes |
|--------|-----------------|------------------|-------|
| Page Detector | O(H×W) | O(H×W) | Dominated by Canny + contour finding |
| Geometric Rectifier | O(H'×W') | O(H'×W') | H',W' = rectified dims; bilinear warp |
| Illumination Normalizer | O(H'×W') | O(H'×W') | Box blur O(1)/pixel via integral image |
| Detail Enhancer | O(H'×W') | O(H'×W') | Convolutions + morphology |
| PDF Exporter | O(H'×W') | O(H'×W') | JPEG encoding dominates |

**Total**: ~O(N) where N = output pixel count. Typical 8MP → rectified ~3MP @ 300 DPI A4.

---

## Numerical Stability Considerations

1. **Homography**: SVD solves Ah=0 robustly; condition number checked via σ_min/σ_max
2. **Division**: L_bg_smooth clamped to ≥ 1.0 before division
3. **LAB ↔ RGB**: Clamping at each stage prevents out-of-gamut artifacts
4. **Bilinear**: Coordinates clamped to [0, W-1] × [0, H-1] with REPLICATE border
5. **Accumulation**: All intermediate computations in f32; final output u8