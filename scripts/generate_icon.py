"""生成 CreatorFlow 应用图标源图（1024x1024 PNG，无第三方依赖）。

用法：python scripts/generate_icon.py app-icon.png
"""

import struct
import sys
import zlib

SIZE = 1024
SS = 2  # 超采样倍数，用于抗锯齿


def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def rounded_rect_distance(x: float, y: float, left: float, top: float, right: float, bottom: float, radius: float) -> float:
    half_w = (right - left) / 2
    half_h = (bottom - top) / 2
    cx = left + half_w
    cy = top + half_h
    dx = abs(x - cx) - (half_w - radius)
    dy = abs(y - cy) - (half_h - radius)
    outside_x = max(dx, 0.0)
    outside_y = max(dy, 0.0)
    inside = min(max(dx, dy), 0.0)
    return (outside_x * outside_x + outside_y * outside_y) ** 0.5 + inside - radius


def bar_alpha(x: float, y: float) -> float:
    """三条白色横条，构成“素材分层/工作流”的抽象标记。"""
    bars = [
        (300.0, 400.0, 724.0, 486.0),
        (300.0, 520.0, 640.0, 606.0),
        (300.0, 640.0, 560.0, 726.0),
    ]
    best = 0.0
    for left, top, right, bottom in bars:
        distance = rounded_rect_distance(x, y, left, top, right, bottom, 26.0)
        coverage = min(max(0.5 - distance, 0.0), 1.0)
        best = max(best, coverage)
    return best


def build_pixels() -> bytes:
    rows = []
    samples = SS * SS
    for py in range(SIZE):
        row = bytearray()
        for px in range(SIZE):
            r_acc = g_acc = b_acc = a_acc = 0.0
            for sy in range(SS):
                for sx in range(SS):
                    x = px + (sx + 0.5) / SS
                    y = py + (sy + 0.5) / SS

                    distance = rounded_rect_distance(x, y, 64.0, 64.0, 960.0, 960.0, 210.0)
                    shape_alpha = min(max(0.5 - distance, 0.0), 1.0)

                    t = min(max((y - 64.0) / 896.0, 0.0), 1.0)
                    r = lerp(88.0, 47.0, t)
                    g = lerp(122.0, 78.0, t)
                    b = lerp(226.0, 168.0, t)

                    mark = bar_alpha(x, y) * 0.92
                    r = lerp(r, 255.0, mark)
                    g = lerp(g, 255.0, mark)
                    b = lerp(b, 255.0, mark)

                    r_acc += r
                    g_acc += g
                    b_acc += b
                    a_acc += shape_alpha
            row.extend(
                (
                    int(round(r_acc / samples)),
                    int(round(g_acc / samples)),
                    int(round(b_acc / samples)),
                    int(round(a_acc / samples * 255)),
                )
            )
        rows.append(bytes(row))
    return b"".join(rows)


def write_png(path: str, raw: bytes) -> None:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)
    stride = SIZE * 4
    filtered = b"".join(b"\x00" + raw[i * stride : (i + 1) * stride] for i in range(SIZE))
    payload = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(filtered, 9))
        + chunk(b"IEND", b"")
    )
    with open(path, "wb") as handle:
        handle.write(payload)


if __name__ == "__main__":
    output = sys.argv[1] if len(sys.argv) > 1 else "app-icon.png"
    write_png(output, build_pixels())
    print(f"written {output}")
