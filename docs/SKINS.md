# MechoFly visual skins

Both skins are procedural and presentation-only. They consume the one
authoritative modeled behavior state but cannot alter neural state, graph
structure, replay, previews, learning values, or receipts.

## Drosophila Natural

Drosophila Natural is the explicit alternate. It uses a compact tan segmented
abdomen, red compound eyes, six articulated legs, and translucent wings.

```powershell
.\host-windows\Start-MechoFly.ps1 -Skin drosophila
```

## MechoFly Prism

MechoFly Prism is the repository, application, and AI100 default. It ports the
recording's segmented lantern abdomen, twin green elytra, green posterior
thorax, orange pronotum, dark-green head, orange eyes, independently animated
glass wing pair with veins, six jointed legs, antennae, and faint airborne
orbit field. Ground states park the flight wings completely and do not draw
motion trails.

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
