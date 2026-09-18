#!/usr/bin/env python3
"""Draw the NYEDArch application icon.

A very round cat, extremely armed, entirely unbothered.

The joke is the product's thesis. The data is not nervous about being out in the
open - it is carrying, and it does not care that you know. The nonchalance is
what makes it funny: half-lidded eyes, a flat little mouth, two oversized
sidearms held like they weigh nothing.

Three things that matter in the drawing:

* **Transparent background.** An opaque tile looks wrong in a dock, on a dark
  desktop, and anywhere the platform applies its own mask.
* **Supersampled 4x, then reduced.** Pillow does not anti-alias, so anything
  drawn at final size comes out jagged. Drawing large and shrinking with Lanczos
  is what makes the curves clean.
* **Everything is a fraction of the canvas**, so it regenerates at any size, and
  the silhouette is checked at 32 px where most icons fall apart.
"""

from PIL import Image, ImageDraw, ImageFilter
import math
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
PINK = (249, 168, 212)

SS = 4
BASE = 1024
S = BASE * SS


def p(f):
    """Fraction of the canvas, in pixels."""
    return f * S


def ell(d, cx, cy, rx, ry, **kw):
    d.ellipse([cx - rx, cy - ry, cx + rx, cy + ry], **kw)


def pistol(d, gx, gy, sgn, scale, ow):
    """One chunky sidearm, gripped at (gx, gy) and angled outward and down.

    The first attempt drew a long horizontal slab from the paw, which read as a
    stick rather than a weapon and stuck out past the canvas edges. Real pistol
    proportions - short slide, deep grip, visible trigger guard - are legible
    even at 32 px, where a thin rectangle is just a line.
    """
    import math as _m
    a = _m.radians(18 * (1 if sgn > 0 else -1))  # tipped down, outward
    ca, sa = _m.cos(a), _m.sin(a)

    def at(u, v):
        u = u * sgn
        return (gx + p(u * scale) * ca - p(v * scale) * sa,
                gy + p(u * scale) * sa + p(v * scale) * ca)

    # Grip: wide, angled back under the paw.
    d.polygon([at(-0.028, 0.005), at(0.030, 0.005), at(0.020, 0.105), at(-0.052, 0.092)],
              fill=STEEL_DARK, outline=INK, width=ow)
    # Slide: short and deep, the part that says "pistol".
    d.polygon([at(-0.060, -0.070), at(0.128, -0.070), at(0.128, 0.006), at(-0.060, 0.006)],
              fill=STEEL, outline=INK, width=ow)
    # Muzzle cap.
    d.polygon([at(0.128, -0.060), at(0.160, -0.060), at(0.160, -0.004), at(0.128, -0.004)],
              fill=STEEL_DARK, outline=INK, width=ow)
    # Trigger guard: a loop, which is what makes it read as a handgun.
    d.polygon([at(0.000, 0.006), at(0.062, 0.006), at(0.056, 0.054), at(0.006, 0.054)],
              fill=STEEL_DARK, outline=INK, width=ow)
    # Sky stripe along the slide, so the weapons belong to this product.
    d.line([at(-0.040, -0.048), at(0.112, -0.048)], fill=SKY_BRIGHT,
           width=int(p(0.012 * scale)))
    # A small muzzle flash hint: one violet spark, the product's "active" colour.
    ell(d, *at(0.176, -0.032), p(0.013 * scale), p(0.013 * scale), fill=VIOLET)


def draw_master():
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx = p(0.50)
    ow = int(p(0.009))

    # A soft contact shadow, so the chonk has weight.
    shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    ell(sd, cx, p(0.845), p(0.255), p(0.048), fill=(15, 23, 42, 85))
    img.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(p(0.015))))

    # ------------------------------------------------------------- tail -----
    # A single curl on the right, sweeping out and up with a visible tip.
    #
    # Two earlier attempts failed differently: one swung it into empty canvas as
    # a detached blob, the other wrapped it so far around the body that it read
    # as a shadow ring. A tail needs a start, one bend, and an end.
    tail_pts = []
    for i in range(24):
        t = i / 23.0
        ang = math.radians(150 - 120 * t)
        r = p(0.215) + p(0.130) * t
        tail_pts.append((cx + r * math.cos(ang) * -1.0, p(0.700) - r * math.sin(ang) * 0.42))
    for i, (tx, ty) in enumerate(tail_pts):
        t = i / (len(tail_pts) - 1)
        w = p(0.052) * (1.0 - 0.40 * t)
        ell(d, tx, ty, w, w, fill=SKY_DEEP)
    # Pale tip, so the end of the tail is legible against the body.
    ell(d, tail_pts[-1][0], tail_pts[-1][1], p(0.030), p(0.030), fill=SKY_LIGHT)

    # ------------------------------------------------------------- body -----
    # One big round blob. The chonk is the silhouette, so it has to be the
    # largest simple shape in the icon.
    body_cy = p(0.615)
    ell(d, cx, body_cy, p(0.268), p(0.225), fill=SKY, outline=INK, width=ow)
    # Right-side shading for volume.
    d.chord([cx - p(0.268), body_cy - p(0.225), cx + p(0.268), body_cy + p(0.225)],
            -70, 110, fill=SKY_DEEP)
    ell(d, cx, body_cy, p(0.268), p(0.225), outline=INK, width=ow)
    # Pale belly.
    ell(d, cx, body_cy + p(0.055), p(0.150), p(0.128), fill=SKY_LIGHT)

    # ------------------------------------------------------------ head ------
    head_cy = p(0.395)
    head_rx, head_ry = p(0.245), p(0.205)

    # Ears, behind the head so the outline stays clean.
    for sgn in (-1, 1):
        base_x = cx + sgn * p(0.150)
        d.polygon(
            [
                (base_x - sgn * p(0.075), head_cy - p(0.150)),
                (base_x + sgn * p(0.060), head_cy - p(0.310)),
                (base_x + sgn * p(0.090), head_cy - p(0.110)),
            ],
            fill=SKY, outline=INK, width=ow,
        )
        # Inner ear.
        d.polygon(
            [
                (base_x - sgn * p(0.035), head_cy - p(0.160)),
                (base_x + sgn * p(0.052), head_cy - p(0.262)),
                (base_x + sgn * p(0.062), head_cy - p(0.140)),
            ],
            fill=PINK,
        )

    ell(d, cx, head_cy, head_rx, head_ry, fill=SKY, outline=INK, width=ow)
    # Cheek floof: two bumps that make the head read as round rather than oval.
    for sgn in (-1, 1):
        ell(d, cx + sgn * p(0.205), head_cy + p(0.055), p(0.070), p(0.062),
            fill=SKY, outline=INK, width=ow)
    ell(d, cx, head_cy, head_rx, head_ry, fill=SKY, outline=INK, width=ow)
    # Forehead highlight.
    ell(d, cx - p(0.075), head_cy - p(0.100), p(0.085), p(0.048), fill=SKY_LIGHT)

    # ------------------------------------------------------------- face -----
    eye_y = head_cy + p(0.005)

    # Half-lidded eyes. This is the whole joke: the cat is not impressed.
    for sgn in (-1, 1):
        ex = cx + sgn * p(0.093)
        # Eye white, mostly covered by the lid.
        ell(d, ex, eye_y, p(0.056), p(0.048), fill=WHITE, outline=INK, width=int(ow * 0.8))
        # Pupil, small and low - a bored gaze.
        ell(d, ex, eye_y + p(0.012), p(0.030), p(0.032), fill=INK)
        ell(d, ex + sgn * p(0.012), eye_y + p(0.002), p(0.010), p(0.010), fill=WHITE)
        # The lid: a filled cap over the top half, with a heavy lash line.
        d.chord([ex - p(0.058), eye_y - p(0.052), ex + p(0.058), eye_y + p(0.046)],
                180, 360, fill=SKY)
        d.line([(ex - p(0.058), eye_y - p(0.004)), (ex + p(0.058), eye_y - p(0.004))],
               fill=INK, width=int(p(0.013)))

    # Nose and a flat, unbothered mouth.
    nose_y = eye_y + p(0.078)
    d.polygon([(cx - p(0.026), nose_y), (cx + p(0.026), nose_y), (cx, nose_y + p(0.028))],
              fill=PINK, outline=INK, width=int(ow * 0.6))
    d.line([(cx, nose_y + p(0.028)), (cx, nose_y + p(0.050))], fill=INK, width=int(p(0.010)))
    # Two small curves: a cat mouth, not a smile.
    d.arc([cx - p(0.058), nose_y + p(0.020), cx, nose_y + p(0.078)], 0, 110,
          fill=INK, width=int(p(0.011)))
    d.arc([cx, nose_y + p(0.020), cx + p(0.058), nose_y + p(0.078)], 70, 180,
          fill=INK, width=int(p(0.011)))

    # Whiskers, kept short so they survive being shrunk.
    for sgn in (-1, 1):
        for dy, dr in ((-0.018, -0.020), (0.010, 0.004), (0.036, 0.030)):
            d.line([(cx + sgn * p(0.140), nose_y + p(dy)),
                    (cx + sgn * p(0.248), nose_y + p(dy + dr))],
                   fill=INK, width=int(p(0.009)))

    # ------------------------------------------------------------- arms -----
    # Stubby arms held out, each with a pistol. They sit low and relaxed.
    for sgn in (-1, 1):
        sx_ = cx + sgn * p(0.215)
        sy_ = body_cy - p(0.020)
        ell(d, sx_, sy_, p(0.072), p(0.062), fill=SKY, outline=INK, width=ow)
        px_ = cx + sgn * p(0.300)
        py_ = sy_ + p(0.055)

        # Gun first, paw second: the paw has to sit on top of the grip or the
        # cat looks like it is standing next to two floating pistols.
        pistol(d, px_ + sgn * p(0.010), py_ - p(0.015), sgn, 1.0, int(ow * 0.8))

        ell(d, px_, py_, p(0.058), p(0.052), fill=SKY_LIGHT, outline=INK, width=ow)
        for k in (-1, 0, 1):
            d.line([(px_ + k * p(0.020), py_ + p(0.018)),
                    (px_ + k * p(0.020), py_ + p(0.042))],
                   fill=INK, width=int(p(0.007)))

    # ------------------------------------------------------- bandolier ------
    # A strap of shells across the chest, because one pair of guns is restraint.
    belt_y = body_cy + p(0.010)
    d.line([(cx - p(0.215), belt_y - p(0.070)), (cx + p(0.205), belt_y + p(0.075))],
           fill=AMBER_DARK, width=int(p(0.050)))
    for i in range(5):
        t = i / 4.0
        ax = cx - p(0.190) + p(0.370) * t
        ay = belt_y - p(0.062) + p(0.130) * t
        d.rounded_rectangle([ax - p(0.018), ay - p(0.030), ax + p(0.018), ay + p(0.030)],
                            radius=p(0.009), fill=AMBER, outline=AMBER_DARK, width=int(ow * 0.6))
        ell(d, ax, ay - p(0.030), p(0.018), p(0.010), fill=(254, 215, 170))

    # ------------------------------------------------------------ feet ------
    for sgn in (-1, 1):
        ell(d, cx + sgn * p(0.105), p(0.812), p(0.078), p(0.042),
            fill=SKY_LIGHT, outline=INK, width=ow)

    # A single violet glint on the collar tag: the one place violet appears,
    # matching the interface, where violet means "this thing is active".
    tag_y = head_cy + p(0.205)
    d.line([(cx - p(0.150), tag_y - p(0.012)), (cx + p(0.150), tag_y + p(0.006))],
           fill=ROSE, width=int(p(0.030)))
    ell(d, cx + p(0.010), tag_y + p(0.034), p(0.040), p(0.040),
        fill=VIOLET, outline=INK, width=int(ow * 0.7))
    ell(d, cx - p(0.002), tag_y + p(0.024), p(0.013), p(0.013), fill=(221, 214, 254))

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
