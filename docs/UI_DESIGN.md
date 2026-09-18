# Interface design

**Light enterprise.** A white base, a sky-blue accent that carries every
interactive affordance, and one violet reserved for a single job: showing that
the application is alive and working.

## Three rules

1. **Depth comes from light, not lines.** Elevation is a soft shadow and a
   hairline. Boxes inside boxes read as clutter, and a security tool people use
   for hours should feel calm.
2. **Colour carries meaning.** Sky is interactive, emerald is engaged, amber is
   attention, rose is refusal, violet is activity. Nothing is coloured to
   decorate.
3. **Motion has a reason.** Anything that moves is reporting something: work
   happening, a value changing, focus arriving.

## The motion language

One idea runs through every animation: **a thin line of light that travels**.
Because the same gesture recurs, the interface reads as one object rather than a
collection of effects.

| Element | Motion | What it reports |
|---|---|---|
| **Perimeter pulse** | A blacklight comet circling the window, forever | The application is alive. Slow while idle, visibly faster while building |
| Progress bar | A sheen sweeping the filled portion | Work is still moving even when the percentage stalls |
| Posture ring | Segments sweep in as protections engage | A protection changed, visible away from the control just touched |
| Build indicator | Aperture closes as the seal completes; scanning rings while running | How far the build has got |
| Rail indicator | Slides between steps | Which step, and which way you moved |
| Segmented control | Selection pill glides | Which option, and which way the choice moved |
| Cards | Lift on hover, tint when consequential | What is interactive, and what has a consequence |

### The perimeter pulse

The signature element. A thin violet comet traces the window edge continuously —
the client is never "off", so its pulse never stops. Its **speed reports state**:
a slow drift while idle, a faster lap while a build runs. Someone glancing at the
window from across a desk can tell whether work is happening without reading a
word.

Two implementation details that matter:

- The phase is **integrated**, not derived from the clock. Changing speed changes
  the *rate*; it never teleports the comet to a new position.
- It is inset by **half the widest stroke** so it sits *on* the window edge with
  no gap and nothing clipped. Insetting further left a visible margin between
  the light and the frame, which broke the illusion that the window is glowing.
- The head is the **deepest** violet and the tail fades lighter — the opposite of
  what reads well on a dark background, where a pale head glows. On white, a pale
  head simply disappears.

It is not drawn over the licence gate: that is the one screen where nothing
should compete with a decision.

## Consequence tinting

Where a setting has a consequence, the card says so by tinting once it is on,
rather than by adding a warning icon and a paragraph:

- **One-shot** tints rose — the capsule destroys itself.
- **Public repository** tints amber — build logs become readable by anyone.

An unselected card stays plain white, so the interface is calm until something
actually warrants attention.

## Layout: a spine and a standing object

There is no sidebar. A vertical list of numbered steps down the left edge is
what every settings window looks like, and it spent a fifth of the screen saying
where you were rather than showing you anything.

Instead:

* **The stage spine** runs across the top — a chain of nodes with a light
  travelling along the completed section, in the same direction as the perimeter
  pulse, so the window has one direction of travel rather than two competing
  ones. Only the active stage shows its hint, so the row stays quiet.
* **The capsule stands beside the work**, full height, for the whole session. It
  used to sit at the bottom of the sidebar beneath six navigation items — the
  thing being built, filed under furniture.
* **The work takes the rest**, with a hairline between object and controls so the
  eye reads two things rather than one crowded column.

The result is a console rather than a form: you are always looking at the thing
you are making, and it is visibly changing as you configure it.

## The vault core

The centrepiece, and the reason this does not read as a settings form.

A security tool that looks like a preferences pane teaches people to treat it
like one. Here the thing being built is on screen the whole time: four rings
orbit a core, one per protection, and each **snaps into place** when that
protection engages. The core's iris closes as the build completes.

Nothing on it is decorative:

| Element | Meaning |
|---|---|
| Ring locked and bright | That protection is engaged |
| Ring drifting and faint | Available, not engaged |
| Lug seated into the bezel | The moment a protection takes hold |
| Iris closed | Sealed |
| Spin rate | Idle, working, or done |
| Violet sweep | A build is running |

It leans very slightly toward the pointer, which is enough to feel like an object
rather than a printed diagram, and not enough to distract.

It replaced a segmented ring that counted protections. A count is a number you
read; a lock closing is something you watch happen.

## Controls are drawn, not themed

Recolouring a stock control leaves it looking stock. Text fields, sliders and
the segmented control are painted by this crate:

| Control | Before | Now |
|---|---|---|
| Text field | Grey box, hard outline that changed colour on hover | Soft inset that lifts to white on focus, with a sky ring that fades in |
| Slider | Default track and square grip | Thin track, sky fill, soft round knob that grows slightly under the cursor |
| Compression | Four outline buttons plus a caption saying which was active | Segmented control with a pill that glides |

## Hover behaves like a surface, not a box

The stock hover state draws a **border** around whatever the cursor is over,
which reads as a box snapping into existence. Every interactive surface now
tints instead — no outline appears on hover anywhere, and `expansion` is zero so
nothing grows or jitters. Pressed goes one shade deeper.

This required setting `weak_bg_fill` as well as `bg_fill`: the first is what a
plain button paints, and setting only the second left toolbar and menu entries
looking like filled boxes with outlines.

## The toolbar is part of the window, not a strip on top of it

It is 30 px, shares the rail's surface colour, and has **no rule beneath it**. A
hairline there cut the window in two and left a visible seam; without it the top
of the window reads as one continuous plane with the content floating on it.

## Accessibility notes

- Meaning is never carried by colour alone: engaged protections also show a
  filled switch and a check, and denials carry text.
- Locked switches use a keyhole ring rather than a padlock glyph, so the meaning
  does not depend on font coverage.
- Contrast targets ink `#0F172A` on white for body text, with muted tones
  reserved for secondary information.
