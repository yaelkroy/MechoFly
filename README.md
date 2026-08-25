# MechoFly

<p align="center">
  <img src="docs/assets/mechofly-firefly-prism.svg" width="760" alt="MechoFly Firefly Prism artwork">
</p>

MechoFly is a Windows desktop companion and transparent neural-model lab. It
combines a deterministic modeled neural engine, a procedural fly overlay,
bounded replay, multi-frame actual-versus-alternative comparison, and an
explicitly authored stimulation preview.

The repository provides two procedural presentation skins:

- **Drosophila Natural** is the application and repository default.
- **Firefly Prism** is an alternate skin and the machine-local default on the
  AI100 development workstation.

The Firefly artwork above is the project hero image; it does not change the
runtime default. Skins affect drawing only. They do not change topology,
dynamics, replay, preview results, or scientific claims.

## Safety boundary

The stimulation preview cannot mutate live simulation state. It runs only on a
deep copy of a bounded replay snapshot, applies a strict local policy, and
emits before/after state digests in its receipt. It has no live-hardware
authority and is not evidence of a biological intervention.

The built-in 1,536-neuron topology is deterministic synthetic demo data. No
FAFB, BANC, MANC, MAOL, MCNS, or other connectome download is bundled.

## Requirements

- Windows 10 or 11, x64
- Windows PowerShell 5.1
- .NET Framework 4.8 developer tools (`csc.exe`)
- Git for Windows for AI100 setup and synchronization

## Build and run

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\host-windows\build.ps1
.\host-windows\Start-MechoFly.ps1
```

Choose a skin explicitly when needed:

```powershell
.\host-windows\Start-MechoFly.ps1 -Skin drosophila
.\host-windows\Start-MechoFly.ps1 -Skin firefly
```

The tray menu can switch skins while MechoFly is running. Open **Brain Lab**
from that menu to retain and inspect a bounded window of modeled frames. Enter
target neuron indices, amplitude, and duration, then choose **Generate
preview**. The paired actual-versus-alternative display can be scrubbed or
played without affecting the live overlay.

## AI100 development workstation

Run the setup script from Windows PowerShell 5.1:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\tools\Setup-AI100-MechoFly.ps1 -Launch
```

It creates or fast-forwards the canonical clean `main` checkout at
`D:\Projects\MechoFly`, writes a machine-local Firefly profile, builds and
self-tests the exact `origin/main` commit, removes only recognized legacy fly
shortcut names, and creates **Start MechoFly**, **Stop MechoFly**, and
**Emergency Stop MechoFly** shortcuts. Rerun it without `-Launch` to safely
synchronize AI100 with GitHub.

The script refuses a dirty checkout, a non-`main` branch, an unexpected remote,
or unrelated content at the target path. It does not delete a legacy project
directory, install drivers, alter WSL, or grant live-hardware authority.

## Verification

```powershell
.\tools\Verify-Tree.ps1
.\host-windows\build.ps1
.\host-windows\bin\MechoFly.exe --self-test .\artifacts\self-test.json
```

See [skins](docs/SKINS.md), [AI100 setup](docs/AI100.md),
[architecture and trust boundaries](docs/ARCHITECTURE.md), and
[data provenance](docs/DATA_PROVENANCE.md).
