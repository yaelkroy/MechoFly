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
- **Center evidence surface:** a 23,210-point FlyWire-derived anatomical
  context behind current modeled activation/spikes, a synchronized selected
  circuit, an aligned actual-versus-authored filmstrip, provenance cards, or
  the learning ledger. A five-stage causal strip explains context → model →
  spikes → behavior → authored pet pose.
- **Counterfactual composer:** targets, amplitude, duration, frame count,
  authored purpose, and the immutable preview boundary appear directly above
  the comparison rather than in an unrelated rail.
- **Right trust layer:** claim badges, graph identity and digest, AI100 source
  branch/commit/tree and executable SHA-256, adapter/API, neuron lookup, and
  the separate software-learning digest. Builds without a complete runtime
  receipt are labeled as unrecorded development builds. Index/root-ID search
  and canvas clicks share one selection and expose strongest modeled inbound
  and outbound connections.
- **Bottom signal timeline:** exact current spike fraction, stable-scale mean
  activation, selected-neuron spike marks, behavior state, replay cursor,
  hovered-frame readout, runtime warning, and the latest user-facing event.

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

## Anatomical context and modeled registration

The prior synthetic bilateral projection created a generic triangular field
that did not resemble the accepted legacy brain view. V3 now draws the
FlyWire-derived soma atlas as faint immutable context and distributes displayed
model neurons over it with a deterministic ordinal hash. The atlas is labeled
`NOT SIMULATED`; the overlay is labeled `NO IDENTITY MAPPING`. This makes the
population and real modeled spike events legible without claiming that a
synthetic or imported graph node has been anatomically registered when it has
not.

## Safety boundary

A valid authored preview targets at most 64 unique neurons, has amplitude in
`(0, 0.25]`, lasts 33–990 ms, stays under the dosage ceiling, and runs only on
a full clone of a retained checkpoint. Its receipt records the live digest
before and after execution. No Brain Lab control has live hardware authority.
