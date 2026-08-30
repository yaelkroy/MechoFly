# N2 Observational Behavior-Transition Telemetry

**Milestone:** N2 — transition telemetry before controller replacement  
**Controller observed:** `legacy-threshold-hold-v1-observed`  
**Telemetry schema:** `1`  
**Self-test schema:** `7`

## Claim boundary

N2 observes the existing deterministic controller. It does not change:

- neural dynamics;
- population thresholds;
- behavior priority;
- fixed hold intervals;
- the legacy autonomous frame schedule;
- model state or frame summaries;
- visible behavior or motor presentation.

The telemetry is instrumentation, not an empirically calibrated ethogram and
not a biological-fit result.

## Recorded transition evidence

Each transition event contains:

```text
sequence
frame and modeled tick
from/to behavior
elapsed frames and milliseconds
transition reason
emergency-override flag
behavior-intent snapshot
pre-transition model-state digest
post-transition model-state digest
```

The intent snapshot records the existing controller evidence without becoming
the controller:

```text
spike count and normalized spike rate
legacy autonomous schedule slot
loom population activation
groom population activation
alert population activation
reverse population activation
walk population activation
existing entry thresholds
```

## Ledger invariants

- bounded to 512 retained events;
- monotonic contiguous retained sequence;
- cumulative dropped-event count;
- hash-chained digest covering every event, including events later evicted from
  the ring buffer;
- exact deterministic equality for identical graph, seed, and input history;
- telemetry-disabled parity oracle proving unchanged model state, summaries,
  and visible behavior.

## Baseline harness

The executable accepts:

```text
--behavior-baseline PATH
--baseline-seeds N
--baseline-repeats N
--baseline-seconds N
```

The harness runs five controlled scenarios:

1. quiet/rest;
2. walking drive;
3. grooming drive;
4. repeated loom;
5. mixed walk/groom/alert/reverse/loom drive.

It records transition counts and reasons, state occupancy, bout-duration bins,
escape latency, deterministic signatures, state digests, and step-time
summaries.

## Evaluation boundary

CI and the initial AI100 package execute a bounded smoke matrix. The planned
D0 campaign remains:

```text
5 scenarios × 20 seeds × 2 exact repeats × 30 modeled minutes
= 200 artifacts
```

The D0 campaign is not complete merely because the smoke matrix passes. No N3
behavior-intent or N4 duration-controller parameter should be accepted until
the full baseline campaign is preserved and reviewed.
