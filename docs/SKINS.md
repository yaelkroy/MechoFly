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

Firefly Lantern restores the accepted `43ba4aa…` Rust presentation:
large emerald elytra, an amber shield, orange eyes, a clearly visible
chartreuse lantern and halo, six readable legs, antennae, and translucent
flight wings. It remains optional; Drosophila Natural is also the AI100
default.

```powershell
.\host-windows\Start-MechoFly.ps1 -Skin firefly
```

Walking uses floating-point screen position and advances in the direction the
head faces. Translation and animation phase are elapsed-time based, so 30 Hz,
60 Hz, 120 Hz, and variable-refresh presentation cover the same distance in
the same wall-clock time. Rest has no translation or decorative bob; its
accepted subtle antenna motion remains bounded well below escape-wing motion.
Flight animates wings, and grooming lifts the forelegs without translating the
pet.
Reduced-motion mode removes nonessential animation but does not alter model
timing.

The Windows host draws either skin into a supersampled premultiplied-alpha
bitmap and presents it through `UpdateLayeredWindow`. Transparent pixels are
not painted and pass mouse hit tests through to the desktop; there is no
rectangular control panel, color key, or magenta background.

Skin names describe artwork only. They do not imply that synthetic topology or
modeled activity belongs to the pictured species.
