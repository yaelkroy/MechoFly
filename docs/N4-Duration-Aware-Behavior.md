# N4.1 - Explicit duration and context-driven autonomy

Parent: `d9c4944f382c6cff0167acc9a0dd5c2be3d8a31f` (validated N3).
This is an intentional behavior-policy change, not another parity-only refactor.

## Ownership and scope

The active application constructs `ModelEngine::new_duration_aware`. The old
`ModelEngine::new` is retained as a compatibility constructor for the frozen N3
oracle and the explicitly named legacy `--behavior-baseline` command. The N4
application never consults the legacy periodic schedule. It also no longer
injects a policy action every 90 model frames; user interaction and hotkeys still
supply bounded neural stimuli. No hotkey sets a pose directly.

`BehaviorDynamicsState` owns macro state, settling substate, entry time, elapsed
frames, minimum dwell, sampled target, transition sequence, interruptibility,
per-state cooldowns, last reason/evidence, duration key, hysteresis latches,
fault state, and bounded internal context. Timing uses authoritative 33 ms
model steps, never display or wall-clock time.

Parameters live in `crates/mechofly-core/parameters/n4-engineering-v1.json`.
The exact file bytes have an ordinary SHA-256 identity. The parameter set is
**MODELED / ENGINEERING PRIOR**, not fitted biology. Duration draws are bounded
multiply-high mappings of a specified integer hash keyed by seed, transition
sequence, state, entry-context bucket, and parameter salt. No global mutable
random generator is used.

Ordinary bouts have minimum dwell and sampled target durations. Escape preempts
any non-airborne ordinary state on the next model step at the existing looming
threshold. The already active escape sequence completes with the validated
198/3993/495 ms preparation/flight/landing envelopes. Landing enters a 495 ms
stationary settling substate. Grooming's initial engineering floor is 46 ticks
(1518 ms), not a universal biological constant.

Arousal, fatigue, contamination pressure, and exploration motivation are bounded
Q15 software variables with declared rates. Zero input can produce modeled
exploration or grooming but never an escape or autonomous reversing. Their
units are not ATP, metabolic energy, measured contamination, or measured affect.
A compact Brain Lab panel identifies these as engineering priors.

## Hysteresis and safety

Ordinary neural latches enter at 4600 and release at 3800 Q15. The spike-rate
alert latch has a separate 1200/1000 per-10k band. Existing escape entry remains
5200 Q15 and bypasses ordinary dwell/cooldown. Invalid controller state produces
a latched Quiet safety state; restoring a malformed checkpoint returns an error.

The old state layout is extended with an optional controller record. Legacy
states serialize and hash exactly as before when that record is absent. New
state digests and model identities include controller state and parameter
identity. Replay already clones ModelState, so it now includes duration draws,
context, and cooldowns. Counterfactual validation checks graph, state digest,
and model identity before cloning; missing N4 state cannot silently downgrade a
properly identified N4 checkpoint. This is not full product-state replay: motor,
UI, sensory-history and policy state remain N6 work.

Telemetry is observational, but **controller semantics changed relative to N3**.
N4 event schema 2 includes duration/cooldown provenance. Legacy event schema 1
and the N3 compatibility oracle remain unchanged. Runtime history stays bounded
at 512 events. The new campaign additionally streams complete transition JSONL
files to disk; it does not pretend an evicted ring contains the full history.

## Evaluation declared before AI100 execution

Run the same five scenarios, graph seeds, model seeds and durations as D0/N3.
Reuse the original stimulus function rather than copy its protocol. Only the
N4 candidate needs a new campaign. Compare distributions against the existing
N3 archive; require exact equality only across N4 repeats, not N4 versus N3.

Hard gates: zero premature ordinary exits, zero non-emergency grooming bouts
under 1500 ms, zero missed eligible looming preemptions, zero controller faults,
zero legacy schedule events, complete duration provenance, exact repeats,
checkpoint next-step equality, bounded context, and preserved neural arrays
under identical input. Quiet-context escape/reverse occupancy must be zero.
A majority of quiet-context time should be Quiet/Rest (an engineering product
criterion, not an empirical ethological fit).

Completed runs are atomically sealed with candidate-executable authority and
parameter identity. Subsequent runs reuse only matching report/event hashes;
incomplete runs restart individually. Percent progress reports completed runs,
not an invented estimate from CPU utilization. The full campaign preserves raw
transition histories for independent event-chain and invariant validation.

## Deliberate non-goals

No new graph, learned parameters, biological sleep, full grooming grammar,
sensorimotor loom redesign, ecological perception, new shadow/landing geometry,
cloud telemetry, or full product checkpoint is claimed. The frozen comparison
product, shortcuts, D0 and N3 evidence remain untouched. No merge is authorized by CI.
