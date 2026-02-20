#!/usr/bin/env bash
# Creates the app icon (AppIcon.icns) from a generated PNG using sips + iconutil
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ICONSET_DIR="${SCRIPT_DIR}/AppIcon.iconset"
ICON_OUTPUT="${SCRIPT_DIR}/AppIcon.icns"

# If we already have an icns file, skip
if [ -f "$ICON_OUTPUT" ]; then
    echo "AppIcon.icns already exists, skipping."
    exit 0
fi

echo "Generating app icon..."

# Create iconset directory
mkdir -p "$ICONSET_DIR"

# Generate a 1024x1024 PNG icon using Python (available on all macOS)
python3 -c "
import struct, zlib, os

def create_png(width, height, pixels):
    '''Create a minimal PNG file from raw RGBA pixel data'''
    def chunk(chunk_type, data):
        c = chunk_type + data
        crc = struct.pack('>I', zlib.crc32(c) & 0xffffffff)
        return struct.pack('>I', len(data)) + c + crc

    header = b'\\x89PNG\\r\\n\\x1a\\n'
    ihdr = chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))

    raw = b''
    for y in range(height):
        raw += b'\\x00'  # filter none
        for x in range(width):
            idx = (y * width + x) * 4
            raw += pixels[idx:idx+4]

    idat = chunk(b'IDAT', zlib.compress(raw, 9))
    iend = chunk(b'IEND', b'')

    return header + ihdr + idat + iend

size = 1024
pixels = bytearray(size * size * 4)

cx, cy = size // 2, size // 2

for y in range(size):
    for x in range(size):
        idx = (y * size + x) * 4

        # Rounded rectangle background
        rx, ry = abs(x - cx), abs(y - cy)
        corner_r = size // 5
        in_rect = True
        if rx > cx - corner_r and ry > cy - corner_r:
            dx = rx - (cx - corner_r)
            dy = ry - (cy - corner_r)
            if dx*dx + dy*dy > corner_r*corner_r:
                in_rect = False

        if not in_rect:
            pixels[idx:idx+4] = [0, 0, 0, 0]
            continue

        # Background gradient: dark terminal green-black
        t = y / size
        bg_r = int(10 + 8 * t)
        bg_g = int(18 + 12 * t)
        bg_b = int(14 + 8 * t)

        # Git branch icon (simplified)
        r, g, b = bg_r, bg_g, bg_b

        # Normalize coordinates to 0-1
        nx = x / size
        ny = y / size

        # Draw a stylized workflow/git icon
        # Main vertical line (trunk)
        trunk_x = 0.38
        trunk_width = 0.035
        if abs(nx - trunk_x) < trunk_width and 0.22 < ny < 0.78:
            r, g, b = 0, 220, 120  # green

        # Branch line going right
        if 0.35 < ny < 0.42:
            branch_nx = trunk_x + (ny - 0.35) * 3.5
            if abs(nx - branch_nx) < trunk_width and nx > trunk_x:
                r, g, b = 80, 200, 255  # cyan

        # Second branch line
        if 0.55 < ny < 0.62:
            branch_nx = trunk_x + (ny - 0.55) * 3.0
            if abs(nx - branch_nx) < trunk_width and nx > trunk_x:
                r, g, b = 255, 160, 60  # orange

        # Node circles on the trunk
        for node_y in [0.28, 0.48, 0.68]:
            dx = (nx - trunk_x)
            dy = (ny - node_y)
            dist = (dx*dx + dy*dy) ** 0.5
            if dist < 0.045:
                if dist < 0.03:
                    r, g, b = 0, 255, 140  # bright green center
                else:
                    r, g, b = 0, 180, 100  # green ring

        # Branch endpoint circles
        for bx, by, cr, cg, cb in [(0.62, 0.42, 100, 220, 255), (0.56, 0.62, 255, 180, 80)]:
            dx = (nx - bx)
            dy = (ny - by)
            dist = (dx*dx + dy*dy) ** 0.5
            if dist < 0.04:
                if dist < 0.025:
                    r, g, b = cr, cg, cb
                else:
                    r, g, b = cr//2, cg//2, cb//2

        # Play triangle (workflow trigger)
        tri_cx, tri_cy = 0.72, 0.35
        tri_size = 0.08
        tdx = nx - tri_cx
        tdy = ny - tri_cy
        if abs(tdy) < tri_size and tdx > -tri_size * 0.6 and tdx < tri_size * 0.6:
            if tdx < tri_size * 0.6 - abs(tdy) * 0.8:
                r, g, b = 0, 220, 120

        # Checkmark at bottom right
        check_cx, check_cy = 0.68, 0.72
        # Down stroke of check
        if 0.65 < ny < 0.76 and 0.62 < nx < 0.72:
            cdx = nx - 0.62
            cdy = ny - 0.65
            if abs(cdy - cdx * 1.5) < 0.025:
                r, g, b = 0, 220, 120
        # Up stroke of check
        if 0.62 < ny < 0.76 and 0.66 < nx < 0.80:
            cdx = nx - 0.70
            cdy = ny - 0.76
            if abs(cdy + cdx * 2.0) < 0.03 and cdy < 0:
                r, g, b = 0, 220, 120

        # Subtle scanlines (retro CRT feel)
        if y % 4 == 0 and in_rect:
            r = max(0, r - 5)
            g = max(0, g - 5)
            b = max(0, b - 5)

        pixels[idx] = min(255, r)
        pixels[idx+1] = min(255, g)
        pixels[idx+2] = min(255, b)
        pixels[idx+3] = 255

png_data = create_png(size, size, bytes(pixels))
with open('${ICONSET_DIR}/icon_512x512@2x.png', 'wb') as f:
    f.write(png_data)

print('Generated 1024x1024 icon')
"

# Generate all required icon sizes using sips
cd "$ICONSET_DIR"
SOURCE="icon_512x512@2x.png"

# Standard sizes
sips -z 16 16 "$SOURCE" --out "icon_16x16.png" > /dev/null 2>&1
sips -z 32 32 "$SOURCE" --out "icon_16x16@2x.png" > /dev/null 2>&1
sips -z 32 32 "$SOURCE" --out "icon_32x32.png" > /dev/null 2>&1
sips -z 64 64 "$SOURCE" --out "icon_32x32@2x.png" > /dev/null 2>&1
sips -z 128 128 "$SOURCE" --out "icon_128x128.png" > /dev/null 2>&1
sips -z 256 256 "$SOURCE" --out "icon_128x128@2x.png" > /dev/null 2>&1
sips -z 256 256 "$SOURCE" --out "icon_256x256.png" > /dev/null 2>&1
sips -z 512 512 "$SOURCE" --out "icon_256x256@2x.png" > /dev/null 2>&1
sips -z 512 512 "$SOURCE" --out "icon_512x512.png" > /dev/null 2>&1
# icon_512x512@2x.png is already the source (1024x1024)

# Create icns
iconutil -c icns "$ICONSET_DIR" -o "$ICON_OUTPUT"

# Cleanup
rm -rf "$ICONSET_DIR"

echo "Created ${ICON_OUTPUT}"
