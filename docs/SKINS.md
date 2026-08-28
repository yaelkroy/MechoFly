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

## MechoFly Prism

MechoFly Prism restores the original recording's jewel-like silhouette:
ten iridescent abdominal segments, faceted cyan thorax, orange compound eyes,
four independently animated glass wings with veins and pterostigmata, six
jointed legs, antennae, an orbit field, and escape trails. It remains optional;
Drosophila Natural is also the AI100 default.

```powershell
.\host-windows\Start-MechoFly.ps1 -Skin firefly
```

Movement uses a two-dimensional velocity and a smoothly turning heading.
Cursor looms turn escape away from the cursor; free flight curves across both
axes; landing decelerates, extends the legs, and settles; walking and backward
motion use an alternating tripod-like gait; grooming alternates the forelegs
across the eyes and antenna base. Translation and animation are elapsed-time
based, so 30 Hz, 60 Hz, 120 Hz, and variable refresh follow the same wall-clock
motion. Rest is exactly stationary, with parked wings and no aura or trails.
Reduced-motion mode removes nonessential animation but does not alter model
timing.

The Windows host draws either skin into a supersampled premultiplied-alpha
bitmap and presents it through `UpdateLayeredWindow`. Transparent pixels are
not painted and pass mouse hit tests through to the desktop; there is no
rectangular control panel, color key, or magenta background.

Skin names describe artwork only. They do not imply that synthetic topology or
modeled activity belongs to the pictured species.
