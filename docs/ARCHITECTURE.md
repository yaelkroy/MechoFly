# MechoFly architecture and trust boundaries

## Current implementation boundary

No third-party source tree or source archive is present in this tree. The
application uses a fresh Windows Forms architecture and a deterministic model
defined in `src/MechoFly`. This statement describes repository contents; it is
not a legal clean-room certification and does not rewrite earlier Git history.

## Runtime flow

```text
modeled topology -> live state -> bounded snapshot ring -> deep-copy branch
                                              |               |
                                              |               +-> authored preview
                                              +------------------> actual frames
                                                     paired comparison -> UI
```

`SimulationCoordinator` is the only owner of mutable live state. Preview work
occurs while holding the coordinator boundary long enough to copy a replay
window and calculate the live-state digest. All subsequent stepping uses a
detached `NeuralState` instance. The receipt must report identical live-state
digests before and after generation.

## Replay limits

- at most 240 stored frames;
- at most 120 frames in one comparison;
- fixed 33 ms model step;
- deep copies only;
- no file, network, device, or hardware side effect in preview generation.

## Stimulation preview policy

A valid plan is explicitly marked `user_authored_preview`, is preview-only,
targets no more than 64 unique modeled neurons, uses amplitude in `(0, 0.25]`,
uses duration from 33 through 990 ms, and stays within the dosage ceiling.
The UI never exposes a commit/apply operation. Rejected plans produce no branch.

## Claims

Neural positions, topology, dynamics, spike events, stimulation, and behavior
are labeled as modeled or simulated unless a separately governed dataset is
loaded. A counterfactual branch is an alternative model trajectory, not a
prediction of an animal or a treatment recommendation.

