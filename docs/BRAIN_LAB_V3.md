# Brain Lab v3 — restored Neural Observatory

> Historical design note. The active recording-parity interface is documented
> in [Rust v5 recording parity](VIDEO_PARITY_V5.md).

Brain Lab v3 restores the accepted Rust interface from commit
`43ba4aa406c5a39f642dace33951fcf10a7c00cf`. The restoration is deliberate:
the later anatomical-canvas redesign reduced the contrast of modeled activity,
made the main field appear empty during quiet states, and displaced useful
controls and evidence.

This is a native Rust MechoFly interface. It is not a port of the upstream
SceneKit brain window.

## Default layout

- **Top observatory deck:** session, exact frame, graph size, selected backend,
  step timing, model-running state, build identity, and the absence of live
  authority.
- **Left model-control dock:** skin/session identity, CPU/GPU preference,
  capacity re-evaluation, bounded replay-source selection, and local
  connectome import.
- **Center population field:** a high-contrast bilateral projection of every
  displayed modeled population, with activation intensity and current spikes
  visible even when the model is quiet.
- **Counterfactual tab:** targets, amplitude, duration, frame count, authored
  purpose, paired actual/alternative filmstrip, transport, inspection cursor,
  divergence counts, and the immutable preview receipt.
- **Right trust layer:** scientific claim badges, graph identity and digest,
  AI100 branch/commit/tree/executable identity, adapter/API, neuron lookup, and
  the separate software-learning digest.
- **Bottom timeline:** retained spike history, current behavior, replay context,
  runtime warnings, and the latest operator-facing event.

Provenance and learning remain separate tabs so they are available without
covering the live population view.

## Information rules

The main plot shows positions owned by the active modeled graph. It does not
scatter unrelated model nodes over anatomical points. The embedded 23,210-point
FlyWire-derived atlas remains a validated reference asset. Anatomical placement
would require an explicit root-ID registration; this restored display does not
infer one.

The alternative surface appears only after a bounded preview succeeds. It is
never presented as a second live model and has no apply or commit action.

## Palette and encoding

- canvas `#05090F`
- surfaces `#0A111B` and `#0F1927`
- text `#E4EEF7`
- actual/model cyan `#38DCCA`
- authored alternative amber `#FFB552`
- safety/positive lime `#B9F162`
- warning coral `#FF7470`
- provenance violet `#A483FF`

Actual and authored-alternative data differ by labels and mark shape as well as
color. Reduced-motion mode changes pet presentation only; it never changes
model timing, replay, or receipts.

## Safety boundary

A valid authored preview targets at most 64 unique neurons, has amplitude in
`(0, 0.25]`, lasts 33–990 ms, stays under the dosage ceiling, and runs only
on a full clone of a retained checkpoint. Its receipt records the live digest
before and after execution. No Brain Lab control has live hardware authority.
