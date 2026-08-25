# MechoFly visual skins

MechoFly has two procedural runtime skins. The skin boundary is deliberately
presentation-only: both skins consume the same modeled behavior label and do
not alter neural state, replay frames, stimulation plans, comparison outputs,
or receipts.

## Drosophila Natural

Drosophila Natural is the application and repository default. It uses a tan
segmented abdomen, red compound eyes, six legs, and one visible wing pair.
Outside modeled flight, the wings fold over the abdomen; during modeled flight,
they extend and animate.

```powershell
.\host-windows\Start-MechoFly.ps1 -Skin drosophila
```

## Firefly Prism

Firefly Prism uses dark green elytra, an amber pronotal shield, a chartreuse
lantern, and translucent hindwings that appear only during modeled flight. It
is the AI100 machine profile, not the repository default.

```powershell
.\host-windows\Start-MechoFly.ps1 -Skin firefly
```

The tray menu can switch between skins for the current process. Starting a new
process uses an explicit `-Skin` argument when supplied; otherwise it reads
`%LOCALAPPDATA%\MechoFly\runtime-profile.json`, falling back to Drosophila.

Skin names describe artwork only. They are not taxonomic labels for the
synthetic topology and do not imply species-specific biological fidelity.
