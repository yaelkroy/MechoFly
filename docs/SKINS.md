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

Firefly Lantern is the Rust reinterpretation of the approved legacy
`neurofly_prism_firefly` / `noctiluca_lantern` appearance: jewel-like emerald
elytra and thorax, amber pronotum, red-orange compound eyes with glints, a
segmented chartreuse lantern and halo, six fine articulated legs, long curved
antennae, and translucent veined flight wings. It is the AI100 machine profile,
not the repository default.

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
not painted and pass mouse hit tests through to the desktop; there is no
rectangular control panel, color key, or magenta background.

Skin names describe artwork only. They do not imply that synthetic topology or
modeled activity belongs to the pictured species.
