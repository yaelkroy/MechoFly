# MechoFly visual skins

Both skins are procedural and presentation-only. They consume modeled behavior
and authored pet-policy action labels but cannot alter neural state, graph
structure, replay, previews, learning values, or receipts.

## Drosophila Natural

Drosophila Natural is the repository default. It uses a compact tan segmented
abdomen, red compound eyes, six articulated legs, and translucent wings.

```powershell
.\host-windows\Start-MechoFly.ps1 -Skin drosophila
```

## Firefly Lantern

Firefly Lantern uses deep green segmented elytra, an amber pronotum, compound
eyes, a chartreuse lantern with a soft alpha glow, six articulated legs, long
antennae, and behavior-specific translucent hindwings. It is the AI100 machine
profile, not the repository default.

```powershell
.\host-windows\Start-MechoFly.ps1 -Skin firefly
```

Walking uses floating-point screen position and visibly advances in the
direction the head faces. Rest has no translation or decorative bob. Flight
unfolds and animates wings, landing adds presentation-only settling rings, and
grooming lifts the forelegs without translating the pet. Reduced-motion mode
removes nonessential animation but does not alter model timing.

The Windows host draws either skin into a supersampled premultiplied-alpha
bitmap and presents it through `UpdateLayeredWindow`. Transparent pixels are
not painted; there is no rectangular control panel, color key, or magenta
background.

Skin names describe artwork only. They do not imply that synthetic topology or
modeled activity belongs to the pictured species.
