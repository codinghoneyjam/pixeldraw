#!/usr/bin/env python3
# <META - FILE SUMMARY - Generate 5 prototype weapon sprites for the crafting modal>
"""Generate the 5 prototype weapon sprites for the weapon crafting modal."""
from pathlib import Path
from PIL import Image, ImageDraw

OUTPUT_DIR = Path("features/ui/stage/modal/craft/assets/weapons")
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
SIZE = (128, 128)

# <META - ROLE : Draw a 128x128 sword sprite | L12-28>
def make_sword() -> Image.Image:
    img = Image.new("RGBA", SIZE, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    d.polygon([(64, 12), (76, 28), (70, 84), (58, 84), (52, 28)], fill=(226, 232, 240, 255), outline=(15, 23, 42, 255))

    d.polygon([(64, 16), (67, 28), (65, 82), (63, 82), (61, 28)], fill=(148, 163, 184, 255))

    d.line([(64, 20), (64, 80)], fill=(34, 211, 238, 200), width=2)

    d.polygon([(40, 84), (88, 84), (84, 94), (44, 94)], fill=(51, 65, 85, 255), outline=(15, 23, 42, 255))
    d.line([(42, 89), (86, 89)], fill=(34, 211, 238, 255), width=1)

    d.polygon([(59, 94), (69, 94), (69, 112), (59, 112)], fill=(30, 41, 59, 255), outline=(15, 23, 42, 255))

    d.polygon([(56, 112), (72, 112), (68, 120), (60, 120)], fill=(100, 116, 139, 255), outline=(15, 23, 42, 255))
    return img

# <META - ROLE : Draw a 128x128 fire sword sprite with flame blade | L31-48>
def make_fire_sword() -> Image.Image:
    img = Image.new("RGBA", SIZE, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    d.polygon([(64, 6), (82, 24), (74, 40), (84, 56), (72, 84), (56, 84), (44, 56), (54, 40), (46, 24)], fill=(239, 68, 68, 180))

    d.polygon([(64, 12), (75, 28), (70, 84), (58, 84), (53, 28)], fill=(249, 115, 22, 255), outline=(185, 28, 28, 255))

    d.polygon([(64, 16), (68, 28), (66, 82), (62, 82), (60, 28)], fill=(254, 240, 138, 255))
    d.line([(64, 18), (64, 80)], fill=(255, 255, 255, 230), width=2)

    d.polygon([(38, 84), (90, 84), (86, 95), (42, 95)], fill=(30, 41, 59, 255), outline=(15, 23, 42, 255))
    d.line([(40, 89), (88, 89)], fill=(245, 158, 11, 255), width=2)

    d.polygon([(59, 95), (69, 95), (69, 113), (59, 113)], fill=(15, 23, 42, 255), outline=(185, 28, 28, 255))

    d.polygon([(55, 113), (73, 113), (69, 121), (59, 121)], fill=(249, 115, 22, 255), outline=(15, 23, 42, 255))
    return img

# <META - ROLE : Draw a 128x128 double-bladed axe sprite | L51-67>
def make_axe() -> Image.Image:
    img = Image.new("RGBA", SIZE, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    d.polygon([(60, 16), (68, 16), (66, 118), (58, 118)], fill=(51, 65, 85, 255), outline=(15, 23, 42, 255))
    d.polygon([(56, 114), (68, 114), (66, 122), (58, 122)], fill=(100, 116, 139, 255))

    d.polygon([(66, 24), (106, 14), (114, 38), (96, 68), (66, 52)], fill=(226, 232, 240, 255), outline=(15, 23, 42, 255))
    d.polygon([(70, 28), (102, 20), (108, 38), (92, 60), (70, 48)], fill=(148, 163, 184, 255))
    d.line([(106, 14), (114, 38), (96, 68)], fill=(245, 158, 11, 255), width=2)

    d.polygon([(60, 28), (32, 20), (22, 38), (36, 50), (60, 46)], fill=(148, 163, 184, 255), outline=(15, 23, 42, 255))
    d.line([(32, 20), (22, 38), (36, 50)], fill=(245, 158, 11, 255), width=2)

    d.polygon([(57, 24), (69, 24), (69, 52), (57, 52)], fill=(30, 41, 59, 255), outline=(15, 23, 42, 255))
    d.line([(63, 26), (63, 50)], fill=(34, 211, 238, 255), width=2)
    return img

# <META - ROLE : Draw a 128x128 bow and arrow sprite | L70-84>
def make_bow() -> Image.Image:
    img = Image.new("RGBA", SIZE, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    d.polygon([(46, 14), (58, 16), (48, 52), (40, 44)], fill=(226, 232, 240, 255), outline=(15, 23, 42, 255))
    d.polygon([(46, 114), (58, 112), (48, 76), (40, 84)], fill=(226, 232, 240, 255), outline=(15, 23, 42, 255))

    d.polygon([(42, 50), (52, 50), (52, 78), (42, 78)], fill=(30, 41, 59, 255), outline=(15, 23, 42, 255))
    d.line([(47, 54), (47, 74)], fill=(74, 222, 128, 255), width=2)

    d.line([(57, 16), (82, 64), (57, 112)], fill=(34, 211, 238, 220), width=2)

    d.line([(36, 64), (96, 64)], fill=(148, 163, 184, 255), width=2)
    d.polygon([(96, 60), (108, 64), (96, 68)], fill=(34, 211, 238, 255), outline=(15, 23, 42, 255))
    return img

# <META - ROLE : Draw a 128x128 gun sprite | L87-105>
def make_gun() -> Image.Image:
    img = Image.new("RGBA", SIZE, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    d.polygon([(20, 40), (104, 40), (104, 58), (20, 58)], fill=(226, 232, 240, 255), outline=(15, 23, 42, 255))

    d.polygon([(104, 42), (114, 42), (114, 56), (104, 56)], fill=(100, 116, 139, 255), outline=(15, 23, 42, 255))

    d.line([(26, 48), (98, 48)], fill=(245, 179, 1, 255), width=2)

    d.polygon([(30, 58), (74, 58), (74, 78), (30, 78)], fill=(51, 65, 85, 255), outline=(15, 23, 42, 255))

    d.polygon([(46, 78), (62, 78), (60, 92), (48, 92)], fill=(245, 179, 1, 200), outline=(15, 23, 42, 255))

    d.polygon([(36, 78), (56, 78), (50, 112), (38, 112)], fill=(30, 41, 59, 255), outline=(15, 23, 42, 255))

    d.polygon([(56, 68), (68, 68), (68, 86), (56, 86)], fill=(15, 23, 42, 255), outline=(15, 23, 42, 255))
    d.line([(60, 72), (64, 78)], fill=(226, 232, 240, 255), width=2)
    return img

# <META - ROLE : Generate all 5 weapon sprites to the output directory | L108-119>
def main():
    weapons = {
        "sword": make_sword(),
        "fire_sword": make_fire_sword(),
        "axe": make_axe(),
        "bow": make_bow(),
        "gun": make_gun(),
    }
    for name, img in weapons.items():
        out_path = OUTPUT_DIR / f"{name}.png"
        img.save(out_path)
        print(f"Generated {out_path}")

if __name__ == "__main__":
    main()
