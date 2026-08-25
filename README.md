# MechoFly

MechoFly is a Windows desktop companion and transparent neural-model lab. The
current tree is a single-language, independently structured C# implementation:
it contains a deterministic modeled neural engine, a procedural fly overlay,
bounded replay, multi-frame actual-versus-alternative comparison, and an
explicitly authored stimulation preview.

The preview is intentionally incapable of mutating live simulation state. It
runs only on a deep copy of a bounded replay snapshot, applies a strict local
policy, and emits before/after state digests in its receipt. It does not drive
hardware and is not evidence of a biological intervention.

## Requirements

- Windows 10 or 11, x64
- Windows PowerShell 5.1
- .NET Framework 4.8 developer tools (the build script locates `csc.exe`)

## Build and run

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\host-windows\build.ps1
.\host-windows\Start-MechoFly.ps1
```

Open **Brain Lab** from the tray menu. The lab continuously retains a bounded
window of modeled frames. Enter target neuron indices, amplitude, and duration,
then choose **Generate preview**. The paired display can be scrubbed or played
without affecting the live overlay.

## Verification

```powershell
.\tools\Verify-Tree.ps1
.\host-windows\build.ps1
.\host-windows\bin\MechoFly.exe --self-test .\artifacts\self-test.json
```

See `docs/ARCHITECTURE.md` for trust boundaries and
`docs/DATA_PROVENANCE.md` before importing any FlyWire/FlyWire Codex product.

