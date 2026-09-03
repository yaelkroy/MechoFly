# N3 - Behavioral intent separation, with frozen D0 compatibility

Base: `c06676c930854728536ee6ee8ba6522e597d1794`.
The independently reviewed D0 archive has SHA-256
`8a1d1ea9a3e453a78fe68c6535f37ddcc58295dc24d14fabf615752ac74b8d32`.

## Production ownership

`ModelEngine` orchestrates `NeuralEvidence::collect`,
`BehaviorIntentBuilder::build`, and `LegacyBehaviorSelector::select`.
The selector consumes the same intent record that is retained and serialized
in transition telemetry. It never reads or mutates neural arrays.

The neural update kernel, seeded noise, initialization, behavior discriminants,
`ModelState`, `FrameSummary`, telemetry struct layout and hash framing remain
unchanged. The existing `BehaviorIntentSnapshot` type is reused and re-exported;
no duplicate wire schema is introduced. Self-test schema 9 and model identity
are intentionally unchanged: this is a compatible refactor, not a new model.

The behavior label and age remain in `ModelState` for checkpoint compatibility.
Moving them into a new persisted controller is later work, not silently done
here. Context is a copied read-only view. No new history needs checkpointing.

## Explicitly unchanged limitations

Population scores remain the maximum of their authored strided lanes, not a
mean, probability, or measured firing rate. The periodic fallback, thresholds,
priority, fixed holds and existing escape sequence remain as in D0. Missing
cue types, smoothing, hysteresis, learned duration and internal motivations are
not invented. These belong to N4 and later evidence-controlled changes.
The current imported/synthetic graph claim boundaries remain unchanged.

No application, shadow, landing, desktop-host, neural-window or shortcut source
is changed by the N3 refactor.

## Evidence gates

The test-only legacy oracle freezes four original functions verbatim. Tests
compare 787,320 threshold/hold/priority/schedule cases, sparse/negative/empty
populations, spike-rate boundaries and saturation, exact intent JSON, 9,000
full-graph model frames, checkpoint restoration, backend-result acceptance,
logging disabled, and ring eviction. The reference update uses scalar neuron
traversal against the unchanged parallel production kernel.

The full AI100 candidate campaign must match the already completed D0 reference
for all 200 run summaries and all 100 repeat groups. Recompute native signatures
and compare every deterministic field, excluding repeat index and timing only.
A signature match is not a claim to possess the complete evicted event history.
The original D0 engine must not be rerun or overwritten.

Local tests, warnings-denied Clippy, schema-9 self-test and Windows GUI/topmost
regressions remain required before any candidate deployment. No merge or new
biological-validity claim is authorized by this document.
