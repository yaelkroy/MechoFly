# N6 paired multi-frame counterfactual replay

## Entry decision

The accepted R9 checkpoint foundation proves that complete modeled and visible
product state can be captured, serialized, and cloned into discarded CPU
branches. It deliberately does not produce the bounded, portable paired replay
required by the complete N6 contract.

This increment adds that missing evidence layer. It does not change controller
thresholds, behavior selection, grooming, walking, flight, landing, parameter
JSON, screen ecology, or deployment state.

## Compile and command boundary

The implementation is available only with the additive
`n6-counterfactual-replay` feature. That feature depends on
`n6-product-checkpoint`; neither feature is enabled by default.

The isolated verification command is:

```text
MechoFly.exe --n6-counterfactual-replay-self-test <receipt.json>
```

The canonical binary and the R9 checkpoint-only binary both reject this
command. The replay build handles it before diagnostics initialization, uses a
fixed CPU-only fixture, writes only the explicitly supplied receipt, and never
starts the desktop pet.

## Portable replay capsule

The receipt embeds a versioned `CounterfactualReplayCapsule` containing:

- the complete R9 `ProductCheckpoint`;
- exact graph, seed, source-frame, and checkpoint identities;
- a bounded 96-frame actual lane;
- a bounded 96-frame changed-input lane;
- the declared cursor-loom intervention and its modeled-only claim;
- per-frame behavior, neural-state, motor-state, and product-state digests;
- the first divergence offset, model frame, and reason; and
- both final product-state digests.

The history ceiling is 128 frames. A capsule whose declared history exceeds
that ceiling fails closed.

## Verification contract

The self-test proves all of the following:

1. both lanes restart from the byte-identical embedded product checkpoint;
2. actual and alternative outputs remain identical before the intervention;
3. the only authored difference is the declared cursor-loom input schedule;
4. common event-keyed randomness is preserved;
5. rebuilding and re-verifying both lanes produces exact frame-for-frame
   equality;
6. JSON serialization preserves the complete checkpoint and both lanes;
7. unknown fields, intervention tampering, and frame-digest tampering are
   rejected; and
8. the live fixture's neural, policy, and motor digests remain unchanged.

This is deterministic offline replay, not authorization to restore a running
desktop-pet session. Both branches are discarded after verification.

## Deferred work

Brain Lab presentation, causal explanation copy, live-session capture, live
restoration, Screen Ecotope geometry, food cues, hunger, resource learning, and
search behavior remain separate reviewed increments. N7 explanation work can
consume this capsule only after the complete N6 replay contract is accepted.
