# Brain Lab v3 — Neural Observatory

Brain Lab v3 uses a dark scientific-workspace visual system. It is deliberately
distinct from both the earlier navy visualization and the retired warm
field-notebook prototype. Controls sit beside the evidence they affect, while
scientific claim and safety boundaries remain continuously visible.

## Layout

- **Top observatory deck:** session, frame, graph size, selected backend,
  timing, modeled-running status, and the absence of live authority.
- **Left model-control dock:** skin/session identity, CPU/GPU preference,
  capacity re-evaluation, bounded replay-source selection, and local
  connectome import.
- **Center evidence surface:** a bilateral population field, an aligned
  actual-versus-authored filmstrip, provenance cards, or the learning ledger.
- **Counterfactual composer:** targets, amplitude, duration, frame count,
  authored purpose, and the immutable preview boundary appear directly above
  the comparison rather than in an unrelated rail.
- **Right trust layer:** claim badges, graph identity and digest, adapter/API,
  neuron lookup, and the separate software-learning digest.
- **Bottom signal timeline:** retained-frame spike activity, behavior state,
  runtime warning, and the most recent user-facing event.

The alternative surface is created only after a bounded preview succeeds. It
is never presented as a second live model and has no apply or commit action.

## Palette and encoding

- canvas `#05090F`
- surfaces `#0A111B` and `#0F1927`
- text `#E4EEF7`
- actual/model cyan `#38DCCA`
- authored alternative amber `#FFB552`
- safety/positive lime `#B9F162`
- warning coral `#FF7470`
- provenance violet `#A483FF`

Actual and authored-alternative data also differ by labels and mark shape;
color is never the only channel. Reduced-motion mode removes nonessential pet
animation without changing model timing, replay, or receipts.

## Projection correction

The synthetic graph stores bilateral positions by alternating index parity.
The earlier display sampled with an even stride at large tiers and therefore
selected only one parity, creating a misleading triangular half-field. V3 uses
an odd deterministic stride, preserving both modeled hemispheres without
changing graph positions or neural state.

## Safety boundary

A valid authored preview targets at most 64 unique neurons, has amplitude in
`(0, 0.25]`, lasts 33–990 ms, stays under the dosage ceiling, and runs only on
a full clone of a retained checkpoint. Its receipt records the live digest
before and after execution. No Brain Lab control has live hardware authority.
