# MechoFly

<p align="center">
  <img src="docs/assets/mechofly-firefly-prism.svg" width="760" alt="MechoFly Firefly artwork">
</p>

MechoFly is a Rust desktop companion and transparent connectome-model lab. It
combines an animated procedural pet, deterministic modeled neural dynamics,
automatic CPU/GPU capacity calibration, bounded replay, aligned multi-frame
actual-versus-alternative comparison, and an explicitly authored stimulation
preview.

The application has two presentation-only skins:

- **Drosophila Natural** is the repository and application default.
- **Firefly Lantern** is the optional alternate skin.

Skins never change graph structure, dynamics, replay, learning, or scientific
claims.

## What is scientifically claimed

MechoFly keeps four layers separate:

1. an imported graph is `DERIVED_CONNECTOME_STRUCTURE`;
2. neural activity is `MODELED_NEURAL_DYNAMICS`, never a recording;
3. pet adaptation is `MODELED_SOFTWARE_LEARNING` from explicit feedback;
4. skins and motion are `AUTHORED_PRESENTATION`.

The FlyWire connectome is a structural wiring diagram, not a complete dynamics
or learning model. The first learning layer is therefore a bounded contextual
pet policy stored outside the connectome. It can be disabled, reset, exported,
or deleted. It never rewrites connectome edges and is not labeled as biological
synaptic plasticity.

## CPU, Radeon, Intel, and NVIDIA

MechoFly benchmarks CPU at every startup and also benchmarks the active
compute-capable GPU when one exists. `Auto` selects the faster exact-matching
backend and the largest named model tier projected to stay inside the model
step budget. With no suitable GPU, the same application runs on CPU.

GPU work uses portable WGSL through `wgpu`; there is no CUDA dependency and no
vendor allowlist. Brain Lab's **Re-evaluate capacity** button repeats the
benchmarks and starts a new identified session if the backend or tier changes.

## Brain Lab v3

Double-click or right-click the pet, use `Ctrl+Alt+N`, or use its tray menu to
open Brain Lab.
The dark Neural Observatory interface has:

- a compact left model/replay dock with automatic CPU/GPU re-evaluation;
- the restored high-contrast modeled-population field from the accepted
  `43ba4aa…` Rust experience;
- an aligned actual-versus-authored comparison with its composer, transport,
  divergence measures, and immutable receipt;
- a right trust layer with exact source, graph, compute, neuron, and learning
  identities; and
- a compact bottom timeline for retained spikes, behavior, replay context,
  warnings, and the latest event.

On Windows the pet is not an `eframe` swap-chain window. Rust supplies a small
premultiplied BGRA bitmap to the native layered-window compositor, so zero-alpha
pixels are real desktop holes and no black rectangle or chroma key is exposed.
Per-pixel hit testing also passes clicks through those holes, so only visible
insect pixels capture interaction.
All visible pet controls live in the tray; the desktop surface contains only
the fly.

Global shortcuts preserve the accepted legacy control contract:

- `Ctrl+Alt+N` toggles Brain Lab;
- `Ctrl+Alt+H` hides or shows the pet;
- `Ctrl+Alt+L` presents loom → escape → landing;
- `Ctrl+Alt+G`, `Ctrl+Alt+B`, and `Ctrl+Alt+W` present grooming, reverse, and
  walk respectively; and
- `Ctrl+Alt+Q` or `Ctrl+Shift+F12` exits.

The native host uses `RegisterHotKey` plus an edge-triggered
`GetAsyncKeyState` fallback for all eight bindings. This keeps the controls
available when Windows reserves F12 or another program owns a registration.

A valid preview targets at most 64 unique neurons, has amplitude in `(0, 0.25]`,
lasts 33–990 ms, stays under a dosage ceiling, and runs on a full deep clone of
a retained checkpoint. There is no apply or commit path. Its receipt proves the
live digest did not change.

## Build and run on Windows

Requirements:

- Windows 10 or 11, x64;
- Windows PowerShell 5.1 or newer; and
- stable Rust with Cargo (Rust 1.95 or newer).

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\host-windows\build.ps1
.\host-windows\Start-MechoFly.ps1
```

Useful launch options:

```powershell
.\host-windows\Start-MechoFly.ps1 -Skin drosophila -Compute auto
.\host-windows\Start-MechoFly.ps1 -Skin firefly -Compute cpu -BrainLab
.\host-windows\Start-MechoFly.ps1 -Compute gpu -ReducedMotion
```

An unavailable explicitly requested GPU falls back visibly to CPU. `Auto` is
the normal setting.

## FlyWire FAFB v783

MechoFly does not bundle or redistribute a connectome connection table. It
does embed a compact set of 23,210 FlyWire-derived soma coordinates as a
validated reference asset. They are not simulated and are not used to place
unregistered model neurons in the default population view. After agreeing to
the FlyWire citation guidelines and principles, download the filtered
connection table from Codex and choose the local CSV or CSV.GZ path in Brain Lab. Import
records the source URL, snapshot, column mapping, SHA-256, transform, counts,
and validation warnings before starting a pinned imported-graph session.

## AI100

`tools\Setup-AI100-MechoFly.ps1` synchronizes a clean selected GitHub branch at
`D:\Projects\MechoFly`, builds and tests it, and records the exact branch,
commit, Git tree, and executable SHA-256. AI100 defaults to Drosophila/Auto.
Start refuses a checkout or binary that differs from that receipt; the Sync
shortcut performs a guarded fast-forward and rebuild.

```powershell
.\tools\Setup-AI100-MechoFly.ps1 -Launch
```

`tools\Capture-AI100-Evidence.ps1` rechecks GitHub identity, exercises CPU and
Auto, captures only MechoFly windows (not the full desktop), and creates one
upload ZIP in Downloads for runtime and design review.

See [architecture](docs/ARCHITECTURE.md),
[compute profiles](docs/COMPUTE_PROFILES.md),
[Brain Lab v3](docs/BRAIN_LAB_V3.md),
[connectome and learning](docs/LEARNING_AND_CONNECTOME.md),
[data provenance](docs/DATA_PROVENANCE.md), and
[AI100 setup](docs/AI100.md).
