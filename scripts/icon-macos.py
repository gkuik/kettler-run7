# Fabrique l'icône macOS (icon.icns) à partir de src-tauri/icons/source.png.
#
# source.png remplit toute la surface 1024×1024. La grille des icônes macOS
# (Big Sur et après) veut une plaque de 824×824 centrée (100 px de marge) aux
# coins arrondis (~185 px), avec une ombre légère : sans ça, l'icône paraît
# plus grosse que les autres dans le Dock. Les icônes Windows/Linux restent
# pleine surface.
#
#   uv run --with pillow python scripts/icon-macos.py
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

ICONS = Path(__file__).resolve().parent.parent / "src-tauri" / "icons"
CANVAS, PLATE, RADIUS = 1024, 824, 185
SS = 4  # suréchantillonnage du masque (bords lisses)


def rounded_mask(size: int, radius: int) -> Image.Image:
    big = Image.new("L", (size * SS, size * SS), 0)
    ImageDraw.Draw(big).rounded_rectangle(
        (0, 0, size * SS - 1, size * SS - 1), radius=radius * SS, fill=255
    )
    return big.resize((size, size), Image.LANCZOS)


def build() -> Image.Image:
    src = Image.open(ICONS / "source.png").convert("RGBA")
    plate = src.resize((PLATE, PLATE), Image.LANCZOS)
    # Rayon plus grand que celui de source.png : les anciens coins
    # transparents tombent dans la partie rognée.
    alpha = ImageChops.multiply(plate.getchannel("A"), rounded_mask(PLATE, RADIUS))
    plate.putalpha(alpha)

    off = (CANVAS - PLATE) // 2
    # Ombre portée discrète, comme les icônes du système.
    shadow = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    shade = Image.new("RGBA", (PLATE, PLATE), (0, 0, 0, 90))
    shade.putalpha(ImageChops.multiply(alpha, Image.new("L", alpha.size, 90)))
    shadow.paste(shade, (off, off + 12), shade)
    shadow = shadow.filter(ImageFilter.GaussianBlur(14))

    out = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    out.alpha_composite(shadow)
    out.alpha_composite(plate, (off, off))
    return out


def main() -> None:
    icon = build()
    icon.save(ICONS / "source-macos.png")
    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / "icon.iconset"
        iconset.mkdir()
        for base in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                px = base * scale
                name = f"icon_{base}x{base}{'@2x' if scale == 2 else ''}.png"
                icon.resize((px, px), Image.LANCZOS).save(iconset / name)
        subprocess.run(
            ["iconutil", "-c", "icns", str(iconset), "-o", str(ICONS / "icon.icns")],
            check=True,
        )
    print("icon.icns et source-macos.png écrits", file=sys.stderr)


if __name__ == "__main__":
    main()
