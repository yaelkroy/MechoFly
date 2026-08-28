# Rust rebuild contract

MechoFly 0.2 is an independent Rust runtime implementation. The accepted legacy
desktop experience is the user-approved behavior, scientific-label,
visual-geometry, and acceptance-test reference for the Firefly Prism and Brain
Lab experience. The result remains native Rust and has no C#, .NET, or earlier
runtime dependency.

## Product invariants

- One portable Rust core owns deterministic model state, replay, stimulation
  policy, provenance, and learning receipts.
- The Windows host uses `eframe`/`egui` with `wgpu`. There is no CUDA path and
  no vendor-name allowlist.
- Drosophila Natural is the repository default. MechoFly Prism is an alternate
  presentation-only skin. AI100 also defaults to Drosophila Natural.
- A fixed 33 ms model clock is independent of visual repaint timing.
- Rest has no translation or decorative bob. Walking advances in floating
  point screen coordinates. Escape has authored preparation, flight, and
  settling phases.
- Brain Lab is a separate opaque `eframe` viewport. On Windows, the pet is an
  independently rendered Win32 layered window supplied with premultiplied
  per-pixel alpha by `UpdateLayeredWindow`; no swap-chain transparency,
  transparency key, or chroma-key color is used.
- Eight legacy-compatible global hotkeys are handled by the native Win32
  pet window with asynchronous edge-triggered fallback for reserved or
  registration-conflicted chords.
- The repository retains 23,210 embedded FlyWire-derived soma points as a
  validated static reference. The default field draws positions owned by the
  active modeled graph; no unregistered node receives an anatomical identity.

## Scientific claim boundary

The application distinguishes four layers in both the UI and machine-readable
receipts:

1. `DERIVED_CONNECTOME_STRUCTURE` — a versioned, hashed graph imported from a
   declared source.
2. `MODELED_NEURAL_DYNAMICS` — MechoFly's deterministic software model. It is
   not an electrophysiological recording.
3. `MODELED_SOFTWARE_LEARNING` — bounded pet-policy adaptation after explicit
   feedback. It is not claimed to be Drosophila synaptic plasticity.
4. `AUTHORED_PRESENTATION` — skins, gestures, animation, and layout.

The FlyWire FAFB connectome is a structural wiring diagram. It does not, by
itself, specify membrane dynamics, complete synaptic signs, neuromodulation,
plasticity, embodiment, or behavior. MechoFly therefore never labels a modeled
spike, action, preference, or learned value as measured biological activity.

## Safety invariants

- Replay retains at most 240 immutable frames.
- Actual and alternative trajectories start from the same full checkpoint,
  including frame, state, seed, history, graph identity, and model version.
- A stimulation request is explicitly authored, targets at most 64 unique
  neurons, has amplitude in `(0, 0.25]`, lasts 33–990 ms, and is limited by a
  target-amplitude-duration dosage ceiling.
- Preview work runs on a deep clone and is discarded. The receipt includes
  before/after digests proving that live state did not change.
- Learning updates only the policy ledger, only after explicit encourage or
  discourage input, and can be disabled, reset, exported, or deleted.
- No runtime path has authority to stimulate biological hardware.

## Primary references

- `wgpu` portability and backend model: <https://docs.rs/wgpu/30.0.0/wgpu/>
- `eframe` native application framework: <https://docs.rs/eframe/0.36.1/eframe/>
- FlyWire whole-brain wiring diagram: <https://www.nature.com/articles/s41586-024-07558-y>
- FlyWire Codex static downloads: <https://codex.flywire.ai/faq>
- Mushroom-body associative-learning anatomy: <https://elifesciences.org/articles/26975>
- A modeled reinforcement-prediction-error rule (not settled ground truth):
  <https://www.nature.com/articles/s41467-021-22592-4>
- Connectome-constrained control precedent in NeuroMechFly v2:
  <https://www.nature.com/articles/s41592-024-02497-y>
