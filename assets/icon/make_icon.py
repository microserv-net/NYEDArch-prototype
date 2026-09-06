#!/usr/bin/env python3
"""Draw the NYEDArch application icon.

A small, extremely chonky block of data that has decided it is a fortress:
stubby arms, a helmet two sizes too big, a shoulder missile pod, a riot shield,
a bandolier, and a minigun it can barely hold up.

The joke is the product's thesis. This is not a file waiting to be protected.

Three things that matter in the drawing:

* **Transparent background.** An opaque tile looks wrong in a dock, on a dark
  desktop, and anywhere the platform applies its own mask.
* **Supersampled 4x, then reduced.** Pillow does not anti-alias, so anything
  drawn at final size comes out jagged. Drawing large and shrinking with Lanczos
  is what makes the curves clean.
* **Everything is a fraction of the canvas**, so it regenerates at any size.
"""

from PIL import Image, ImageDraw, ImageFilter
import os

# The interface palette, so the icon belongs to the same product.
SKY = (14, 165, 233)
SKY_LIGHT = (125, 211, 252)
SKY_BRIGHT = (56, 189, 248)
SKY_DEEP = (3, 105, 161)
SKY_SHADOW = (2, 78, 122)
VIOLET = (139, 92, 246)
INK = (12, 20, 38)
STEEL = (100, 116, 139)
STEEL_DARK = (51, 65, 85)
STEEL_LIGHT = (203, 213, 225)
WHITE = (255, 255, 255)
AMBER = (245, 158, 11)
AMBER_DARK = (180, 83, 9)
ROSE = (244, 63, 94)

SS = 4          # supersampling factor
BASE = 1024     # nominal size
S = BASE * SS   # working canvas


def p(f):
    """Fraction of the canvas, in pixels."""
    return f * S


def ell(d, cx, cy, rx, ry, **kw):
    d.ellipse([cx - rx, cy - ry, cx + rx, cy + ry], **kw)


def draw_master():
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx = p(0.50)
    ow = int(p(0.009))

    bx0, bx1 = p(0.268), p(0.732)
    by0, by1 = p(0.372), p(0.762)
    corner = p(0.125)

    # A soft contact shadow, so the character stands on something.
    shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    ell(sd, cx, p(0.812), p(0.240), p(0.052), fill=(15, 23, 42, 80))
    img.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(p(0.016))))

    # ------------------------------------------------------ missile pod -----
    mx0, my0 = p(0.258), p(0.392)
    d.rounded_rectangle([mx0 - p(0.118), my0 - p(0.088), mx0 + p(0.088), my0 + p(0.058)],
                        radius=p(0.032), fill=STEEL_DARK, outline=INK, width=ow)
    for i in range(2):
        for j in range(2):
            tx = mx0 - p(0.084) + i * p(0.064)
            ty = my0 - p(0.052) + j * p(0.060)
            ell(d, tx, ty, p(0.025), p(0.025), fill=STEEL, outline=INK, width=int(ow * 0.7))
            ell(d, tx, ty, p(0.011), p(0.011), fill=ROSE)
    d.polygon([(mx0 + p(0.088), my0 - p(0.086)), (mx0 + p(0.156), my0 - p(0.156)),
               (mx0 + p(0.156), my0 - p(0.070)), (mx0 + p(0.088), my0 - p(0.018))],
              fill=STEEL, outline=INK, width=ow)

    # ------------------------------------------------------- riot shield ----
    sx, sy = p(0.778), p(0.556)
    d.polygon(
        [
            (sx - p(0.132), sy - p(0.208)),
            (sx + p(0.116), sy - p(0.162)),
            (sx + p(0.126), sy + p(0.116)),
            (sx - p(0.010), sy + p(0.238)),
            (sx - p(0.132), sy + p(0.112)),
        ],
        fill=STEEL_LIGHT, outline=INK, width=ow,
    )
    d.line([(sx - p(0.062), sy - p(0.022)), (sx - p(0.004), sy + p(0.050)),
            (sx + p(0.064), sy - p(0.040))], fill=SKY, width=int(p(0.026)), joint="curve")
    for ry_ in (-0.132, 0.072):
        for rx_ in (-0.096, 0.082):
            ell(d, sx + p(rx_), sy + p(ry_), p(0.011), p(0.011), fill=STEEL)

    # -------------------------------------------------------------- body ----
    d.rounded_rectangle([bx0, by0, bx1, by1], radius=corner, fill=SKY)
    d.rounded_rectangle([cx + p(0.058), by0, bx1, by1], radius=corner, fill=SKY_DEEP)
    d.rounded_rectangle([bx0, by0, bx1, by1], radius=corner, outline=INK, width=ow)
    d.rounded_rectangle([bx0 + p(0.022), by0 + p(0.016), bx1 - p(0.155), by0 + p(0.078)],
                        radius=p(0.031), fill=SKY_LIGHT)
    for f in (0.62, 0.74):
        y = by0 + (by1 - by0) * f
        d.line([(bx0 + p(0.030), y), (bx1 - p(0.030), y)], fill=SKY_SHADOW, width=int(p(0.008)))

    # ------------------------------------------------------------ helmet ----
    hy = by0 + p(0.010)

    # A helmet wraps the head. The first attempt was a semicircle sitting on a
    # long flat bar, which reads as a mushroom cap however it is coloured: the
    # brim, not the dome, was doing all the silhouette work.
    #
    # This is a dome that comes down past the temples, with ear pads at the
    # sides and only a short lip at the front - the shape a helmet actually has.
    ell(d, cx, hy - p(0.012), p(0.250), p(0.152), fill=STEEL_DARK, outline=INK, width=ow)
    # Flatten the underside so it sits on the head rather than floating.
    d.rectangle([cx - p(0.250), hy - p(0.012), cx + p(0.250), hy + p(0.066)], fill=STEEL_DARK)
    d.line([(cx - p(0.250), hy - p(0.012)), (cx - p(0.250), hy + p(0.066))], fill=INK, width=ow)
    d.line([(cx + p(0.250), hy - p(0.012)), (cx + p(0.250), hy + p(0.066))], fill=INK, width=ow)

    # Ear pads.
    for sgn in (-1, 1):
        d.rounded_rectangle(
            [cx + sgn * p(0.250) - p(0.050), hy + p(0.006),
             cx + sgn * p(0.250) + p(0.050), hy + p(0.104)],
            radius=p(0.030), fill=STEEL, outline=INK, width=ow,
        )

    # Front lip, short and centred.
    d.rounded_rectangle([cx - p(0.150), hy + p(0.058), cx + p(0.150), hy + p(0.104)],
                        radius=p(0.022), fill=STEEL, outline=INK, width=ow)

    # Crest stripe.
    d.rounded_rectangle([cx - p(0.024), hy - p(0.166), cx + p(0.024), hy + p(0.044)],
                        radius=p(0.012), fill=SKY_BRIGHT)

    # Antenna and beacon.
    d.line([(cx + p(0.176), hy - p(0.128)), (cx + p(0.228), hy - p(0.278))],
           fill=INK, width=int(p(0.015)))
    ell(d, cx + p(0.230), hy - p(0.292), p(0.034), p(0.034), fill=ROSE, outline=INK, width=int(ow * 0.7))
    ell(d, cx + p(0.220), hy - p(0.303), p(0.012), p(0.012), fill=(255, 210, 220))

    # -------------------------------------------------------------- face ----
    eye_y = by0 + p(0.148)
    for sgn in (-1, 1):
        ex = cx + sgn * p(0.082)
        ell(d, ex, eye_y, p(0.062), p(0.068), fill=WHITE, outline=INK, width=int(ow * 0.8))
        ell(d, ex + sgn * p(0.012), eye_y + p(0.010), p(0.033), p(0.036), fill=INK)
        ell(d, ex + sgn * p(0.023), eye_y - p(0.012), p(0.015), p(0.015), fill=WHITE)
    d.line([(cx - p(0.146), eye_y - p(0.082)), (cx - p(0.036), eye_y - p(0.050))],
           fill=INK, width=int(p(0.019)))
    d.line([(cx + p(0.146), eye_y - p(0.082)), (cx + p(0.036), eye_y - p(0.050))],
           fill=INK, width=int(p(0.019)))
    d.arc([cx - p(0.048), eye_y + p(0.062), cx + p(0.048), eye_y + p(0.130)],
          200, 345, fill=INK, width=int(p(0.016)))

    # --------------------------------------------------------- bandolier ----
    belt_y = by0 + p(0.268)
    d.line([(bx0 - p(0.008), belt_y - p(0.042)), (bx1 + p(0.008), belt_y + p(0.064))],
           fill=AMBER_DARK, width=int(p(0.054)))
    for i in range(6):
        t = i / 5.0
        ax = bx0 + (bx1 - bx0) * t
        ay = belt_y - p(0.042) + p(0.106) * t
        d.rounded_rectangle([ax - p(0.019), ay - p(0.033), ax + p(0.019), ay + p(0.033)],
                            radius=p(0.010), fill=AMBER, outline=AMBER_DARK, width=int(ow * 0.6))
        ell(d, ax, ay - p(0.033), p(0.019), p(0.011), fill=(254, 215, 170))

    # ----------------------------------------------------------- minigun ----
    gx, gy = p(0.398), p(0.648)
    d.rounded_rectangle([gx - p(0.042), gy - p(0.054), gx + p(0.178), gy + p(0.054)],
                        radius=p(0.027), fill=STEEL_DARK, outline=INK, width=ow)
    for k, off in enumerate((-0.036, 0.0, 0.036)):
        d.rounded_rectangle([gx - p(0.282), gy + p(off) - p(0.017),
                             gx - p(0.030), gy + p(off) + p(0.017)],
                            radius=p(0.015),
                            fill=STEEL_LIGHT if k == 1 else STEEL,
                            outline=INK, width=int(ow * 0.7))
    ell(d, gx - p(0.030), gy, p(0.031), p(0.060), fill=STEEL, outline=INK, width=int(ow * 0.7))
    ell(d, gx + p(0.152), gy + p(0.046), p(0.064), p(0.064), fill=AMBER, outline=INK, width=ow)
    ell(d, gx + p(0.152), gy + p(0.046), p(0.027), p(0.027), fill=AMBER_DARK)
    ell(d, gx + p(0.112), gy - p(0.030), p(0.017), p(0.017), fill=VIOLET)

    # -------------------------------------------------------------- arms ----
    d.line([(bx0 + p(0.036), by0 + p(0.318)), (gx + p(0.021), gy - p(0.021))],
           fill=SKY_DEEP, width=int(p(0.058)))
    ell(d, gx + p(0.021), gy - p(0.021), p(0.041), p(0.041),
        fill=SKY_BRIGHT, outline=INK, width=int(ow * 0.8))
    d.line([(bx1 - p(0.036), by0 + p(0.302)), (sx - p(0.097), sy + p(0.011))],
           fill=SKY_DEEP, width=int(p(0.058)))
    ell(d, sx - p(0.097), sy + p(0.011), p(0.041), p(0.041),
        fill=SKY_BRIGHT, outline=INK, width=int(ow * 0.8))

    # ------------------------------------------------------------- boots ----
    for sgn in (-1, 1):
        bx = cx + sgn * p(0.114)
        d.rounded_rectangle([bx - p(0.071), by1 - p(0.012), bx + p(0.071), by1 + p(0.072)],
                            radius=p(0.035), fill=STEEL_DARK, outline=INK, width=ow)

    return img


def main():
    out = os.path.dirname(os.path.abspath(__file__))
    master = draw_master().resize((BASE, BASE), Image.LANCZOS)
    master.save(os.path.join(out, "icon.png"))

    for s in (16, 24, 32, 48, 64, 128, 256, 512):
        master.resize((s, s), Image.LANCZOS).save(os.path.join(out, f"icon-{s}.png"))

    master.save(
        os.path.join(out, "icon.ico"),
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print("wrote transparent icon.png, icon.ico and 8 sizes to", out)


if __name__ == "__main__":
    main()
