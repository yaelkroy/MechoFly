# Brain Lab v2

Brain Lab v2 intentionally does not reproduce the earlier navy/cyan layout.
It uses a warm field-notebook visual system and moves experiment controls to a
left rail.

## Layout

- **Left experiment rail:** session identity, compute mode, pinned model tier,
  replay selection, intervention authoring, and safety limits.
- **Center evidence canvas:** population map in overview mode; a two-row
  multi-frame filmstrip in comparison mode. The rows are `ACTUAL` and
  `ALTERNATIVE`; an aligned divergence strip sits below them.
- **Right evidence inspector:** claim badges, selected-neuron details, exact
  graph/model hashes, adapter, and receipt status.
- **Bottom timeline:** rolling spike raster, behavior states, replay cursor,
  and the explanation for state persistence.

The alternative area is not an empty permanent panel. It appears only after a
comparison is generated.

## Palette and encoding

- canvas `#F4EFE3`
- surfaces `#FFFDF8`
- ink `#17212B`
- actual cobalt `#005AA9`
- alternative vermilion `#B23A2B`
- positive `#176B52`
- warning `#8A4B08`

Actual and alternative data also differ by labels, line shape, and glyphs;
color is never the only channel. The palette avoids rainbow maps and preserves
at least WCAG AA text contrast. Reduced-motion mode removes nonessential
animation without changing model timing.
