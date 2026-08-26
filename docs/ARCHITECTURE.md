# MechoFly architecture and trust boundaries

## Independent implementation boundary

The active application is written in Rust under `crates/`. Earlier software is
used only as acceptance evidence for behavior, scientific labels, and visual
quality. No earlier source or architecture is part of this implementation.
This is a repository-design statement, not a legal opinion or a rewrite of Git
history.

## Runtime ownership

```text
versioned graph ──> immutable incoming CSR ──> deterministic fixed-point step
                                                │
                                                v
                                      one live ModelState owner
                                                │
                              ┌─────────────────┴─────────────────┐
                              v                                   v
                    bounded checkpoint ring                pet presentation
                              │                                   │
                    full checkpoint clone              separate policy ledger
                              │                          (explicit feedback only)
                  ┌───────────┴───────────┐
                  v                       v
              actual branch       authored alternative
                  └───────────┬───────────┘
                              v
                  filmstrip + immutable receipt
```

`SimulationSession` is the sole owner of mutable live neural state. A preview
receives a copied checkpoint and immutable shared graph. The comparison code
has no reference that can mutate the session, policy, filesystem, network, or a
device. Before/after live digests are included in the receipt.

The pet policy is a separate serialized object. It selects authored companion
actions; it does not write the graph or model state. Only explicit encourage or
discourage input invokes its bounded update rule.

## Compute equivalence

CPU and GPU implement one signed 32-bit fixed-point gather kernel. Every target
reads a stable incoming CSR interval. There are no unordered float atomics.
Startup calibration executes both kernels when GPU compute is available and
requires exact state equality before GPU can be selected.

Capacity evaluation chooses a named synthetic tier only at a session boundary.
An imported graph is never automatically truncated. Re-evaluation starts a new
session identifier and replay epoch.

## UI processes and windows

One `MechoFly.exe` process owns:

- a compact native Win32 layered pet window with per-pixel alpha;
- a system tray menu; and
- an opaque resizable Brain Lab child viewport.

All artwork is procedural. The Windows pet is supersampled in Rust into
premultiplied BGRA and sent to `UpdateLayeredWindow`; zero-alpha pixels are
desktop holes. `eframe` remains hidden at the root and owns the opaque Brain
Lab viewport plus portable `wgpu` compute. No magenta/chroma-key transparency
is used. Visual repaint timing is independent from the fixed 33 ms model step.

## Safety limits

- 240 retained checkpoints maximum;
- 120 comparison frames maximum;
- 64 unique intervention targets maximum;
- amplitude greater than zero and at most 0.25;
- duration 33–990 ms in 33 ms increments;
- four neuron-seconds dosage ceiling; and
- `live_hardware_authority = NONE` in runtime and receipts.
