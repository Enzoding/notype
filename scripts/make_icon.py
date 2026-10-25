#!/usr/bin/env python3
import os
import sys
import shutil
import subprocess
from PIL import Image, ImageDraw, ImageFilter
import numpy as np

def make_macos_icon(input_path, output_dir="assets"):
    os.makedirs(output_dir, exist_ok=True)
    src = Image.open(input_path).convert("RGBA")
    w, h = src.size

    # Check if image has squircle tile or is full bleed
    arr = np.array(src)
    # Check edges to see if background is outside the tile
    bg_corners = [arr[20, 20], arr[20, w-20], arr[h-20, 20], arr[h-20, w-20]]
    bg_mean = np.mean(bg_corners, axis=0)[:3]
    
    # Analyze horizontal and vertical cross-sections
    h_line = arr[h // 2, :, :3].astype(float)
    v_line = arr[:, w // 2, :3].astype(float)
    
    h_diff = np.linalg.norm(h_line - bg_mean, axis=1)
    v_diff = np.linalg.norm(v_line - bg_mean, axis=1)
    
    threshold = 30.0
    h_idx = np.where(h_diff > threshold)[0]
    v_idx = np.where(v_diff > threshold)[0]
    
    # If the image is already a framed tile inside background
    if len(h_idx) > 0 and len(v_idx) > 0 and (h_idx[0] > 60 or h_idx[-1] < w - 60):
        # Detected framed tile
        left, right = h_idx[0], h_idx[-1]
        top, bottom = v_idx[0], v_idx[-1]
        tile_w = right - left
        tile_h = bottom - top
        dim = min(tile_w, tile_h)
        cx, cy = (left + right) // 2, (top + bottom) // 2
        crop_box = (cx - dim // 2, cy - dim // 2, cx + dim // 2, cy + dim // 2)
        tile = src.crop(crop_box)
    else:
        # Full bleed icon
        tile = src

    # Standard macOS App Icon is 824x824 squircle centered in 1024x1024 canvas
    icon_size = 824
    canvas_size = 1024
    radius = 185

    tile_resized = tile.resize((icon_size, icon_size), Image.Resampling.LANCZOS)

    # Squircle mask with continuous curvature approximation
    mask = Image.new("L", (icon_size, icon_size), 0)
    draw_mask = ImageDraw.Draw(mask)
    draw_mask.rounded_rectangle([0, 0, icon_size, icon_size], radius=radius, fill=255)
    tile_resized.putalpha(mask)

    # Canvas with drop shadow
    canvas = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    shadow = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    shadow_draw = ImageDraw.Draw(shadow)
    
    pad = (canvas_size - icon_size) // 2
    shadow_draw.rounded_rectangle(
        [pad, pad + 18, pad + icon_size, pad + icon_size + 18],
        radius=radius,
        fill=(0, 0, 0, 100)
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(radius=26))

    canvas.alpha_composite(shadow)
    canvas.paste(tile_resized, (pad, pad), tile_resized)

    # Save 1024 PNG
    out_1024 = os.path.join(output_dir, "app_icon_1024.png")
    canvas.save(out_1024, "PNG")
    canvas.save(os.path.join(output_dir, "logo.png"), "PNG")
    print(f"Saved: {out_1024}")

    # Generate iconset
    iconset_dir = os.path.join(output_dir, "AppIcon.iconset")
    os.makedirs(iconset_dir, exist_ok=True)

    sizes = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ]

    for filename, sz in sizes:
        resized = canvas.resize((sz, sz), Image.Resampling.LANCZOS)
        resized.save(os.path.join(iconset_dir, filename), "PNG")

    # Run iconutil
    icns_path = os.path.join(output_dir, "AppIcon.icns")
    subprocess.run(["iconutil", "-c", "icns", iconset_dir, "-o", icns_path], check=True)
    shutil.rmtree(iconset_dir, ignore_errors=True)
    print(f"Saved: {icns_path}")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python3 scripts/make_icon.py <input_image>")
        sys.exit(1)
    make_macos_icon(sys.argv[1])
