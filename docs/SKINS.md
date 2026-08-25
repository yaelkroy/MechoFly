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

## Firefly Field

Firefly Field uses deep green elytra, an amber pronotum, a chartreuse lantern,
six articulated legs, and translucent hindwings. It is the AI100 machine
profile, not the repository default.

```powershell
.\host-windows\Start-MechoFly.ps1 -Skin firefly
```

Walking uses floating-point screen position and visibly advances. Rest has no
translation or decorative bob. Flight unfolds and animates wings; grooming
uses legs without translating the pet. Reduced-motion mode removes nonessential
animation but does not alter model timing.

Skin names describe artwork only. They do not imply that synthetic topology or
modeled activity belongs to the pictured species.
