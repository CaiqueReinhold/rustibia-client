# bevy_text_outline — Design Document

## What it does

`TextOutline { width, color }` on a UI `Text` or a `Text2d` draws four copies of the text's glyphs
in `color`, offset by `round(width × scale_factor)` **physical** pixels on the four cardinals, behind
the fill. The outline's alpha is multiplied by the fill's `TextColor` alpha, so a fading text fades
its outline too.

At width 1 the footprint is the centre plus the four cardinals: a one-pixel outline with no
diagonal coverage. With `FontSmoothing::None`, glyph alpha is binary, and the copies produce
exactly the pixels that a one-pixel morphological dilation did.

## How

The copies are made during render extraction; no entities are spawned. The mechanism is Bevy's
own `TextShadow`/`Text2dShadow` extraction, run four times with four offsets:

| Text | System | Pushes into | Ordered |
|---|---|---|---|
| UI `Text` | `extract_ui_text_outlines` | `ExtractedUiNodes` | in `RenderUiSystems::ExtractText`, before `extract_text_sections` |
| `Text2d` | `extract_text2d_outlines` | `ExtractedSprites` / `ExtractedSlices` | after `SpriteSystems::ExtractSprites`, before `extract_text2d_sprite` |

**The copies need no z offset.** They are pushed at the fill's own z, and the phase sort is stable,
so push order puts them behind the fill. Bevy's shadows rely on the same property.

**The `Text2d` offset is applied after the `1 / scale_factor` scaling**, in glyph-layout space,
which is physical pixels. So ±1 is one physical pixel on screen whatever transform the text sits
under.

## Why four copies rather than dilation

- The outline was earlier made by dilating each glyph into a separate outline atlas. For the widths
  this game uses, it produced the same pixels as four copies, at the cost of an atlas, a prepare
  pass, and UI-only support.
- Four copies work unchanged on `Text2d`, which the agent HUD and all map text now use.
- The cost is four extra glyph quads per glyph. That is negligible at the text volumes here.

## Known trade-off

On text with font smoothing on, the copies composite: an edge pixel of alpha `a` becomes
`1 − (1 − a)^4`. The edges come out darker but still soft. Dilation took a max instead. Every
outlined text in the client uses `FontSmoothing::None` except the chat tab titles.

## Bevy version

Written against Bevy 0.18: `ExtractedUiNodes`, `ExtractedGlyph`, `ExtractedSprites`,
`ExtractedSlices`, and the ordering sets above. These are internal render types and change between
releases; check Bevy's own shadow extraction when upgrading.
