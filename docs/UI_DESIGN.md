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

## Accessibility notes

- Meaning is never carried by colour alone: engaged protections also show a
  filled switch and a check, and denials carry text.
- Locked switches use a keyhole ring rather than a padlock glyph, so the meaning
  does not depend on font coverage.
- Contrast targets ink `#0F172A` on white for body text, with muted tones
  reserved for secondary information.
