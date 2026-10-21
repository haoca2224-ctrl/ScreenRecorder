from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"
SIZES = (16, 24, 32, 48, 64, 96, 128)


def icon(size: int) -> Image.Image:
    scale = size / 128
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    p = lambda value: round(value * scale)
    draw.rounded_rectangle((p(6), p(6), p(122), p(122)), radius=p(28), fill=(18, 25, 37, 255))
    draw.rounded_rectangle(
        (p(22), p(27), p(106), p(91)),
        radius=p(12),
        outline=(234, 240, 248, 255),
        width=max(1, p(7)),
    )
    draw.line((p(64), p(91), p(64), p(104)), fill=(234, 240, 248, 255), width=max(1, p(7)))
    draw.line((p(48), p(104), p(80), p(104)), fill=(234, 240, 248, 255), width=max(1, p(7)))
    draw.ellipse((p(46), p(41), p(82), p(77)), fill=(218, 50, 74, 255))
    draw.ellipse((p(57), p(52), p(71), p(66)), fill=(255, 255, 255, 240))
    return image


def main() -> None:
    images = []
    for size in SIZES:
        value = icon(size)
        value.save(ASSETS / f"app-icon-{size}.png", optimize=True)
        images.append(value)
    images[-1].save(
        ASSETS / "screen-recorder.ico",
        format="ICO",
        sizes=[(size, size) for size in (16, 24, 32, 48, 64, 96, 128)],
    )


if __name__ == "__main__":
    main()
