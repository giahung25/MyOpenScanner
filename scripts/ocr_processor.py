#!/usr/bin/env python3
"""
MyOpenScanner OCR & Searchable PDF Generator
Tạo lớp văn bản ẩn (Invisible Text Layer - PDF render_mode 3) đè lên ảnh quét chuẩn tọa độ,
cho phép bôi đen, sao chép (copy), và tìm kiếm (Ctrl+F) văn bản mà vẫn giữ nguyên 100% bố cục gốc.
"""

import argparse
import io
import json
import os
import sys
from pathlib import Path
from PIL import Image
import fitz  # PyMuPDF
from rapidocr_onnxruntime import RapidOCR


def process_searchable_pdf(image_path: Path, output_pdf_path: Path, output_text_path: Path = None, output_json_path: Path = None, dpi: int = 300) -> dict:
    if not image_path.exists():
        raise FileNotFoundError(f"Không tìm thấy ảnh: {image_path}")

    # 1. Mở và nén ảnh sang JPEG trong bộ nhớ để tối ưu kích thước PDF
    with Image.open(image_path) as img:
        img_w, img_h = img.size
        img_rgb = img.convert("RGB")
        jpeg_buffer = io.BytesIO()
        img_rgb.save(jpeg_buffer, format="JPEG", quality=92, optimize=True)
        jpeg_bytes = jpeg_buffer.getvalue()

    # 2. Nhận diện chữ và tọa độ từng khối văn bản qua RapidOCR (ONNX)
    engine = RapidOCR()
    ocr_results, elapse = engine(str(image_path))

    # 3. Khởi tạo tài liệu PDF với kích thước chuẩn tỉ lệ (1 pt = 1/72 inch)
    doc = fitz.open()
    pt_w = img_w * 72.0 / float(dpi)
    pt_h = img_h * 72.0 / float(dpi)
    page = doc.new_page(width=pt_w, height=pt_h)

    # 4. Chèn ảnh quét làm lớp nền hiển thị (Background Image Layer)
    page.insert_image(fitz.Rect(0, 0, pt_w, pt_h), stream=jpeg_bytes)

    # 5. Chèn lớp văn bản ẩn (Invisible Text Layer: render_mode = 3)
    scale_x = pt_w / float(img_w)
    scale_y = pt_h / float(img_h)

    extracted_lines = []
    ocr_boxes_data = []

    if ocr_results:
        for item in ocr_results:
            box, text, score = item[0], item[1], float(item[2])
            text = text.strip()
            if not text:
                continue

            extracted_lines.append(text)

            # box: [[x0, y0], [x1, y1], [x2, y2], [x3, y3]]
            px_x0 = min(p[0] for p in box)
            px_y0 = min(p[1] for p in box)
            px_x1 = max(p[0] for p in box)
            px_y1 = max(p[1] for p in box)

            pt_x0 = px_x0 * scale_x
            pt_y0 = px_y0 * scale_y
            pt_x1 = px_x1 * scale_x
            pt_y1 = px_y1 * scale_y

            box_h = pt_y1 - pt_y0
            fontsize = max(box_h * 0.85, 4.0)
            baseline_y = pt_y1 - (box_h * 0.15)

            # Chèn text ẩn đúng tọa độ
            try:
                page.insert_text(
                    fitz.Point(pt_x0, baseline_y),
                    text,
                    fontsize=fontsize,
                    fontname="helv",
                    render_mode=3,  # 3 = Neither fill nor stroke (invisible selectable text)
                )
            except Exception:
                pass

            ocr_boxes_data.append({
                "text": text,
                "confidence": score,
                "bbox_px": [px_x0, px_y0, px_x1, px_y1],
                "bbox_pt": [pt_x0, pt_y0, pt_x1, pt_y1],
            })

    # Lưu file PDF có tính năng bôi đen / copy text
    output_pdf_path.parent.mkdir(parents=True, exist_ok=True)
    doc.save(str(output_pdf_path), deflate=True)
    doc.close()

    # 6. Xuất text plain nếu có yêu cầu
    full_text = "\n".join(extracted_lines)
    if output_text_path:
        output_text_path.write_text(full_text, encoding="utf-8")

    # 7. Xuất chi tiết JSON OCR nếu có yêu cầu
    if output_json_path:
        with open(output_json_path, "w", encoding="utf-8") as f:
            json.dump({
                "image_size": [img_w, img_h],
                "page_size_pt": [pt_w, pt_h],
                "dpi": dpi,
                "blocks_count": len(ocr_boxes_data),
                "blocks": ocr_boxes_data,
                "full_text": full_text,
            }, f, ensure_ascii=False, indent=2)

    return {
        "status": "success",
        "output_pdf": str(output_pdf_path),
        "total_blocks": len(ocr_boxes_data),
        "file_size_bytes": output_pdf_path.stat().st_size,
    }


def main():
    parser = argparse.ArgumentParser(description="Tạo Searchable PDF giữ nguyên bố cục tờ giấy")
    parser.add_argument("--image", required=True, type=Path, help="Đường dẫn ảnh đã cắt và cân bằng sáng")
    parser.add_argument("--output-pdf", required=True, type=Path, help="Đường dẫn file PDF đầu ra có thể bôi đen text")
    parser.add_argument("--output-text", type=Path, help="Đường dẫn xuất text thuần")
    parser.add_argument("--output-json", type=Path, help="Đường dẫn xuất metadata OCR")
    parser.add_argument("--dpi", type=int, default=300, help="DPI trang PDF")

    args = parser.parse_args()
    res = process_searchable_pdf(args.image, args.output_pdf, args.output_text, args.output_json, args.dpi)
    print(json.dumps(res, indent=2))


if __name__ == "__main__":
    main()
