# N6 product-state checkpoint foundation

## Entry decision

The accepted N4.1-D exploratory-flight candidate closes the immediate visual
motion correction. The next integrated-program increment is N6: make complete
product state inspectable before adding screen ecology, learned resources, or
food-search behavior.

This branch implements the first behavior-neutral N6 foundation. It does not
claim full N6 completion and it does not change the canonical application,
controller thresholds, motor trajectories, parameter JSON, or deployment
state.

## Compile and runtime boundary

The implementation is available only when `mechofly-app` is compiled with the
`n6-product-checkpoint` feature. That feature is absent from `default`.

The feature build exposes one noninteractive verification command:

```text
MechoFly.exe --n6-product-checkpoint-self-test <receipt.json>
```

The canonical binary rejects that option and must not create a receipt. The
feature command uses a fixed CPU-only fixture and exits without starting the
desktop pet. It does not read or write AppData.

## Captured state

The versioned, fail-closed checkpoint captures the state needed to determine
the next modeled and visible frame:

- graph identity, model identity, full neural state, last frame summary, and
  last behavior-intent snapshot;
- sensory input vector, cursor-loom input, and any unexpired authored drive;
- complete presentation-motor position, velocity, heading, behavior age,
  landing state, walk/flight bout identity, and sampled motion profiles;
- policy configuration, current action, and last policy context;
- the seed and event-keyed motor state already materialized in bout counters
  and sampled profiles;
- skin identity; and
- grooming program state, derived deterministically from behavior and modeled
  behavior age.

Wall-clock time is deliberately excluded. Unknown JSON fields, graph identity
changes, inconsistent state digests, frame mismatches, invalid dimensions, and
invalid motor values are rejected.

## Discarded counterfactual branches

A valid checkpoint can construct a CPU-only discarded branch. It cannot
restore into or replace the live application session. The N6 self-test proves:

1. serialization and deserialization preserve the checkpoint exactly;
2. two zero-intervention branches remain identical for 64 modeled frames;
3. a branch receiving a different cursor-loom input diverges;
4. graph, neural-state, and unknown-field tampering fail closed; and
5. running both controls and the changed-input branch leaves the live model,
   policy, and motor digests unchanged.

This establishes capture, exact replay, and counterfactual isolation. A later
reviewed N6 increment may connect the same contract to complete user-session
capture and replay tooling. Live restoration remains explicitly unauthorized.

## Deferred product and biological behavior

N6 adds no screen-region targets, coverage objective, food location, hunger,
odor, taste, resource learning, or food-search state. Those concepts belong to
later screen-ecology increments and require explicit biological evidence and
separate owner review. The accepted walking, grooming, natural-flight, and
uncued exploratory-flight behavior remains frozen.

## Acceptance contract

The branch passes only when the canonical and feature builds both compile, all
workspace tests pass, static frozen identities pass, the canonical binary
rejects the N6 option, two feature-build self-test receipts are byte-identical,
and every receipt safety flag remains false for live restore, AppData writes,
promotion, and deployment.
