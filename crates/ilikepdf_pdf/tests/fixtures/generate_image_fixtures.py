"""Regenerate deterministic, project-owned Image-to-PDF fixtures."""

from pathlib import Path

from PIL import Image, ImageDraw


OUTPUT = Path(__file__).with_name("images")


def striped(size: tuple[int, int], color: tuple[int, int, int]) -> Image.Image:
    image = Image.new("RGB", size, color)
    draw = ImageDraw.Draw(image)
    draw.rectangle((0, 0, size[0] // 4, size[1] - 1), fill=(255, 255, 255))
    return image


def main() -> None:
    OUTPUT.mkdir(exist_ok=True)

    striped((80, 120), (220, 30, 30)).save(OUTPUT / "portrait.png")
    striped((120, 80), (30, 180, 70)).save(OUTPUT / "landscape.png")

    transparent = Image.new("RGBA", (100, 100), (0, 0, 0, 0))
    draw = ImageDraw.Draw(transparent)
    draw.rectangle((25, 25, 74, 74), fill=(35, 90, 220, 255))
    transparent.save(OUTPUT / "transparent.png")

    jpeg = striped((120, 80), (30, 90, 220))
    jpeg.save(OUTPUT / "photo.jpg", quality=92, optimize=False, progressive=False)
    jpeg.save(OUTPUT / "photo.jpeg", quality=92, optimize=False, progressive=False)

    oriented = striped((120, 80), (235, 145, 25))
    exif = Image.Exif()
    exif[274] = 6
    oriented.save(
        OUTPUT / "oriented.jpg",
        quality=92,
        optimize=False,
        progressive=False,
        exif=exif,
    )

    striped((100, 70), (135, 45, 190)).save(
        OUTPUT / "sample.webp", lossless=True, method=6
    )
    animation_frames = [
        Image.new("RGB", (24, 16), (220, 30, 30)),
        Image.new("RGB", (24, 16), (30, 90, 220)),
    ]
    animation_frames[0].save(
        OUTPUT / "animated.webp",
        save_all=True,
        append_images=animation_frames[1:],
        duration=100,
        loop=0,
        lossless=True,
    )
    animation_frames[0].save(
        OUTPUT / "animated.png",
        save_all=True,
        append_images=animation_frames[1:],
        duration=100,
        loop=0,
    )
    (OUTPUT / "malformed.png").write_bytes(b"not a decodable image")


if __name__ == "__main__":
    main()
