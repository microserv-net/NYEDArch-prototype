#!/usr/bin/env python3
"""Draw the NYEDArch application icon.

A chonky little data block that is armed to a ridiculous degree — a cylinder
of data with a determined face, a helmet, a shoulder-mounted launcher, a riot
shield, and far too much ammunition. The joke is the product's thesis: this is
not a file waiting to be protected, it is a file that fights back.

Drawn programmatically so it regenerates at any size and stays under version
control as source rather than as an opaque binary.
"""

from PIL import Image, ImageDraw
import math
import os

# The interface palette, so the icon belongs to the same product.
SKY = (14, 165, 233)
SKY_BRIGHT = (56, 189, 248)
SKY_DEEP = (2, 105, 166)
VIOLET = (124, 58, 237)
INK = (15, 23, 42)
STEEL = (71, 85, 105)
STEEL_LIGHT = (148, 163, 184)
WHITE = (255, 255, 255)
AMBER = (217, 119, 6)
ROSE = (225, 29, 72)

S = 1024  # master size; everything is expressed as a fraction of this


def px(f):
    return int(round(f * S))


def ellipse(d, cx, cy, rx, ry, fill=None, outline=None, width=0):
    d.ellipse([cx - rx, cy - ry, cx + rx, cy + ry], fill=fill, outline=outline, width=width)


def rounded(d, x0, y0, x1, y1, r, fill=None, outline=None, width=0):
    d.rounded_rectangle([x0, y0, x1, y1], radius=r, fill=fill, outline=outline, width=width)


def draw_icon(size=S, with_background=True):
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # ---- background: a soft rounded square so the mark reads on any wallpaper
    if with_background:
        rounded(d, px(0.02), px(0.02), px(0.98), px(0.98), px(0.22), fill=WHITE)
        rounded(d, px(0.02), px(0.02), px(0.98), px(0.98), px(0.22),
                outline=(226, 232, 240), width=px(0.006))

    cx = px(0.50)

    # ---- riot shield, behind the body, on its right ------------------------
    sh_x, sh_y = px(0.735), px(0.545)
    d.polygon(
        [
            (sh_x - px(0.115), sh_y - px(0.175)),
            (sh_x + px(0.105), sh_y - px(0.140)),
            (sh_x + px(0.115), sh_y + px(0.120)),
            (sh_x, sh_y + px(0.215)),
            (sh_x - px(0.115), sh_y + px(0.120)),
        ],
        fill=STEEL_LIGHT, outline=STEEL, width=px(0.010),
    )
    # A sky chevron on the shield, echoing the aperture mark.
    d.line([(sh_x - px(0.055), sh_y - px(0.010)), (sh_x, sh_y + px(0.050)),
            (sh_x + px(0.055), sh_y - px(0.010))], fill=SKY, width=px(0.020), joint="curve")

    # ---- shoulder launcher, behind the body, on its left -------------------
    lx, ly = px(0.245), px(0.400)
    d.rounded_rectangle([lx - px(0.150), ly - px(0.052), lx + px(0.120), ly + px(0.052)],
                        radius=px(0.052), fill=STEEL, outline=INK, width=px(0.008))
    # Muzzle
    ellipse(d, lx - px(0.150), ly, px(0.052), px(0.052), fill=INK)
    ellipse(d, lx - px(0.150), ly, px(0.030), px(0.030), fill=(30, 41, 59))
    # Sight on top
    d.rounded_rectangle([lx - px(0.020), ly - px(0.092), lx + px(0.045), ly - px(0.055)],
                        radius=px(0.014), fill=STEEL_LIGHT, outline=INK, width=px(0.006))
    # A little violet charge light: the one place violet appears.
    ellipse(d, lx + px(0.070), ly - px(0.004), px(0.020), px(0.020), fill=VIOLET)

    # ---- the body: a data cylinder -----------------------------------------
    top_y = px(0.360)
    bot_y = px(0.800)
    rx = px(0.235)
    ry = px(0.072)

    # Barrel
    d.rectangle([cx - rx, top_y, cx + rx, bot_y - ry], fill=SKY)
    ellipse(d, cx, bot_y - ry, rx, ry, fill=SKY)
    # Shading down the right side for volume
    d.rectangle([cx + px(0.120), top_y, cx + rx, bot_y - ry], fill=SKY_DEEP)
    ellipse(d, cx, bot_y - ry, rx, ry, fill=SKY_DEEP)
    d.rectangle([cx - rx, top_y, cx + px(0.120), bot_y - ry], fill=SKY)
    ellipse(d, cx, bot_y - ry, rx, ry, outline=SKY_DEEP, width=px(0.010))
    # Re-draw the lit face of the bottom cap
    d.pieslice([cx - rx, bot_y - ry * 2, cx + rx, bot_y], 0, 180, fill=SKY_DEEP)

    # Disc separations, so it reads as stacked data rather than a tin can
    for f in (0.52, 0.64):
        y = top_y + (bot_y - ry - top_y) * f
        ellipse(d, cx, y, rx, ry, outline=(3, 130, 200), width=px(0.007))

    # Top cap
    ellipse(d, cx, top_y, rx, ry, fill=SKY_BRIGHT, outline=SKY_DEEP, width=px(0.010))

    # ---- helmet -------------------------------------------------------------
    hy = top_y - px(0.020)
    d.pieslice([cx - px(0.255), hy - px(0.230), cx + px(0.255), hy + px(0.120)],
               180, 360, fill=STEEL, outline=INK, width=px(0.009))
    d.rounded_rectangle([cx - px(0.262), hy + px(0.030), cx + px(0.262), hy + px(0.082)],
                        radius=px(0.026), fill=STEEL_LIGHT, outline=INK, width=px(0.008))
    # Antenna with a warning light
    d.line([(cx + px(0.170), hy - px(0.150)), (cx + px(0.215), hy - px(0.290))],
           fill=INK, width=px(0.014))
    ellipse(d, cx + px(0.215), hy - px(0.300), px(0.032), px(0.032), fill=ROSE)
    ellipse(d, cx + px(0.207), hy - px(0.310), px(0.011), px(0.011), fill=(255, 200, 210))

    # ---- face ---------------------------------------------------------------
    eye_y = top_y + px(0.115)
    for sx in (-1, 1):
        ex = cx + sx * px(0.082)
        ellipse(d, ex, eye_y, px(0.046), px(0.050), fill=WHITE)
        ellipse(d, ex + sx * px(0.008), eye_y + px(0.006), px(0.024), px(0.026), fill=INK)
        ellipse(d, ex + sx * px(0.016), eye_y - px(0.006), px(0.009), px(0.009), fill=WHITE)
    # Determined eyebrows
    d.line([(cx - px(0.132), eye_y - px(0.062)), (cx - px(0.036), eye_y - px(0.034))],
           fill=INK, width=px(0.016))
    d.line([(cx + px(0.132), eye_y - px(0.062)), (cx + px(0.036), eye_y - px(0.034))],
           fill=INK, width=px(0.016))
    # Small set mouth
    d.arc([cx - px(0.052), eye_y + px(0.052), cx + px(0.052), eye_y + px(0.125)],
          200, 340, fill=INK, width=px(0.014))

    # ---- ammunition belt across the body ------------------------------------
    belt_y = top_y + px(0.245)
    d.line([(cx - rx - px(0.010), belt_y - px(0.045)), (cx + rx + px(0.010), belt_y + px(0.055))],
           fill=(120, 53, 15), width=px(0.048))
    for i in range(6):
        t = i / 5.0
        bx = cx - rx + (2 * rx) * t
        by = belt_y - px(0.045) + (px(0.100)) * t
        d.rounded_rectangle([bx - px(0.017), by - px(0.030), bx + px(0.017), by + px(0.030)],
                            radius=px(0.008), fill=AMBER, outline=(146, 64, 14), width=px(0.005))

    # ---- tiny arms ----------------------------------------------------------
    # Left arm gripping the launcher
    d.line([(cx - px(0.215), top_y + px(0.300)), (lx + px(0.060), ly + px(0.060))],
           fill=SKY_DEEP, width=px(0.052))
    ellipse(d, lx + px(0.060), ly + px(0.060), px(0.036), px(0.036), fill=SKY_BRIGHT,
            outline=SKY_DEEP, width=px(0.008))
    # Right arm holding the shield
    d.line([(cx + px(0.215), top_y + px(0.300)), (sh_x - px(0.080), sh_y + px(0.020))],
           fill=SKY_DEEP, width=px(0.052))
    ellipse(d, sh_x - px(0.080), sh_y + px(0.020), px(0.036), px(0.036), fill=SKY_BRIGHT,
            outline=SKY_DEEP, width=px(0.008))

    # ---- little boots -------------------------------------------------------
    for sx in (-1, 1):
        bx = cx + sx * px(0.105)
        d.rounded_rectangle([bx - px(0.062), bot_y + px(0.010), bx + px(0.062), bot_y + px(0.082)],
                            radius=px(0.030), fill=INK)

    if size != S:
        img = img.resize((size, size), Image.LANCZOS)
    return img


def main():
    out = os.path.dirname(os.path.abspath(__file__))
    master = draw_icon(S)
    master.save(os.path.join(out, "icon.png"))

    for s in (16, 24, 32, 48, 64, 128, 256, 512):
        draw_icon(s).save(os.path.join(out, f"icon-{s}.png"))

    # Windows .ico with the usual sizes embedded.
    master.save(
        os.path.join(out, "icon.ico"),
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print("wrote icon.png, icon.ico and 8 sizes to", out)


if __name__ == "__main__":
    main()
