#!/usr/bin/env python3
"""
Generates macOS AppIcon.icns, app_icon_1024.png, and logo.png according to
Apple Human Interface Guidelines (macOS Big Sur+):
- 1024x1024 canvas
- 824x824 squircle centered with radius 185
- Dual-layer macOS shadow (ambient soft + directional key)
- Supersampled (4x) antialiased continuous rounded mask
"""
import os
import sys
import shutil
import subprocess
from PIL import Image, ImageDraw, ImageFilter
import numpy as np

def make_macos_icon(input_path, output_dir="assets"):
    os.makedirs(output_dir, exist_ok=True)
    src = Image.open(input_path).convert("RGBA")
    arr = np.array(src)

    # Detect content bounds (where alpha > 40)
    alpha = arr[:, :, 3]
    y_idx, x_idx = np.where(alpha > 40)
    if len(y_idx) == 0:
        raise ValueError("Image is completely transparent")

    min_x, max_x = x_idx.min(), x_idx.max()
    min_y, max_y = y_idx.min(), y_idx.max()

    center_x = (min_x + max_x) // 2
    center_y = (min_y + max_y) // 2
    dim = max(max_x - min_x, max_y - min_y)

    # Square crop around content center
    crop_box = (
        center_x - dim // 2,
        center_y - dim // 2,
        center_x + dim // 2,
        center_y + dim // 2
    )
    cropped = src.crop(crop_box)

    # Sample cream background color from corners of content
    cream_rgb = (253, 248, 239)

    # Composite cropped tile over solid cream background to ensure full opacity
    opaque_tile = Image.new("RGBA", cropped.size, cream_rgb + (255,))
    opaque_tile.paste(cropped, (0, 0), cropped)

    # Standard macOS App Icon squircle size on 1024x1024 canvas
    tile_size = 824
    radius = 185
    resized_tile = opaque_tile.resize((tile_size, tile_size), Image.Resampling.LANCZOS)

    # 4x Supersampled antialiased squircle mask
    scale = 4
    large_sz = tile_size * scale
    large_rad = radius * scale
    large_mask = Image.new("L", (large_sz, large_sz), 0)
    draw = ImageDraw.Draw(large_mask)
    draw.rounded_rectangle([0, 0, large_sz, large_sz], radius=large_rad, fill=255)
    squircle_mask = large_mask.resize((tile_size, tile_size), Image.Resampling.LANCZOS)

    # Apply squircle mask to tile
    final_tile = resized_tile.copy()
    final_tile.putalpha(squircle_mask)

    # Build 1024x1024 canvas with authentic macOS dual-layer shadow
    canvas_size = 1024
    canvas = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    pad = (canvas_size - tile_size) // 2

    # Layer 1: Ambient soft shadow
    ambient = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    amb_draw = ImageDraw.Draw(ambient)
    amb_draw.rounded_rectangle(
        [pad, pad + 12, pad + tile_size, pad + tile_size + 12],
        radius=radius,
        fill=(0, 0, 0, 45)
    )
    ambient = ambient.filter(ImageFilter.GaussianBlur(radius=30))

    # Layer 2: Directional key shadow
    key_shadow = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    key_draw = ImageDraw.Draw(key_shadow)
    key_draw.rounded_rectangle(
        [pad, pad + 20, pad + tile_size, pad + tile_size + 20],
        radius=radius,
        fill=(0, 0, 0, 65)
    )
    key_shadow = key_shadow.filter(ImageFilter.GaussianBlur(radius=18))

    canvas.alpha_composite(ambient)
    canvas.alpha_composite(key_shadow)
    canvas.alpha_composite(final_tile, (pad, pad))

    # Save 1024 master PNGs
    out_1024 = os.path.join(output_dir, "app_icon_1024.png")
    out_logo = os.path.join(output_dir, "logo.png")
    canvas.save(out_1024, "PNG")
    canvas.save(out_logo, "PNG")
    print(f"Saved: {out_1024}")
    print(f"Saved: {out_logo}")

    # Generate multi-resolution iconset for macOS
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

    # Run macOS native iconutil to compile AppIcon.icns
    icns_path = os.path.join(output_dir, "AppIcon.icns")
    subprocess.run(["iconutil", "-c", "icns", iconset_dir, "-o", icns_path], check=True)
    shutil.rmtree(iconset_dir, ignore_errors=True)
    print(f"Saved: {icns_path}")

if __name__ == "__main__":
    input_file = sys.argv[1] if len(sys.argv) > 1 else "assets/brand-refresh-20261025/app-icon-concept.png"
    make_macos_icon(input_file)
