# N7 scientific explanation and provenance foundation

## Entry decision

The accepted R10 replay provides the bounded actual and changed-input product
lanes required by N6. This increment consumes that exact replay and adds a
deterministic, machine-readable explanation layer. It does not change how the
fly behaves and it does not yet place the explanation in Live Brain or Brain
Lab.

The source capsule is pinned to SHA-256
`d93d88d8ab2b2d0da15605e761adcdb9b303345756ed14c2f2f41e6ddc7c7835`.
Generation fails closed if the R10 capsule, graph, replay window, intervention,
or divergence identity changes.

## Compile and command boundary

The implementation is available only with the additive
`n7-scientific-explanation` feature. That feature depends on
`n6-counterfactual-replay`; no experimental feature is enabled by default.

The isolated command is:

```text
MechoFly.exe --n7-scientific-explanation-self-test <receipt.json>
```

Canonical, checkpoint-only, and R10 replay-only binaries reject that command.
The N7 build handles it before diagnostics initialization, writes only the
explicit receipt, and never starts the desktop pet.

## Explanation bundle

The receipt embeds both 96-frame controller lanes from R10. Each frame exposes:

- macro state and motor substate;
- state age and entry frame;
- minimum dwell and sampled target duration;
- deterministic duration draw and context bucket;
- hysteresis latches and spike-alert latch;
- per-behavior refractory limits and interruptibility;
- internal context and population-intent evidence;
- modeled spike and activation summary;
- state, motor, and product digests; and
- explicit statements for why the state began, why it persisted, and what
  remains scientifically unresolved.

Contiguous controller states are also grouped into bounded behavior episodes.
An episode whose ending lies beyond the replay window is marked `UNRESOLVED`;
the implementation never fabricates an ending.

## Evidence vocabulary

Every scientific quantity intended for later presentation is wrapped with one
of the required evidence classes:

```text
MEASURED
DERIVED
MODELED
INFERRED
AUTHORED
ENGINEERING PRIOR
UNRESOLVED
PRESENTATION SAFETY OVERRIDE
```

The bundle includes a complete provenance catalog explaining how each class is
used. The paired-lane causal statement is limited to this controlled modeled
fixture; it is not a claim of biological causation or empirical calibration.

## Verification contract

The self-test proves that:

1. the exact accepted R10 capsule and graph are the source;
2. controller observations reproduce every R10 state, motor, and product
   digest;
3. both lanes contain exactly 96 frames under the 128-frame ceiling;
4. the authored intervention remains 12 frames beginning at offset 24;
5. first divergence remains offset 24, model frame 37;
6. all eight evidence classes are present;
7. unknown fields, causal-copy tampering, and provenance-tag tampering are
   rejected;
8. serialization and repeated generation are exact; and
9. the live neural, policy, and motor digests remain unchanged.

R9 and R10 self-test receipts are separately required to remain byte-identical
to their accepted hashes.

## Deferred work

Live Brain and Brain Lab presentation, selected-neuron provenance, retained
modeled-spike UI, palette and scaling work, live-session capture, live restore,
Screen Ecotope geometry, food cues, hunger, resource learning, biological
calibration, promotion, and deployment remain separate reviewed increments.
