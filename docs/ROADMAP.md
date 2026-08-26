# MechoFly product and science roadmap

**Status:** proposed master roadmap for the Rust product line  
**Updated:** 2026-08-26  
**Owner:** Yael Demedetskaya  
**Scope:** product direction, scientific boundaries, delivery order, and release gates  
**Implementation claim:** none; a roadmap item is not complete until its stated evidence gate passes

## 1. Baselines and authority

This roadmap is written against these frozen references:

- MechoFly default branch: `main` at `fa0ca7653bda7042028c7353d1e648de60f5f9e8`.
- Active Rust v3 line at authoring time: PR #5, branch
  `feat/transparent-pet-dark-brainlab-v3`, head
  `42ffb50c969c19f9290ebf509cd2087e8711a731`.
- DesktopFly public behavior reference: `DenisSergeevitch/desktop-fly` `master`
  at `7014d37d7e252a3f16b173aca9b49f6f6c91d3b9`.
- Ethological design reference: *DesktopFly Scientific, Behavioral, Visual,
  and Adoption Improvement Plan*, V10, 2026-08-21.

The exact branch heads can move after this document is written. Runtime and
release statements must always cite the exact commit, tree, executable hash,
and evidence packet actually tested.

The V10 ethology plan retains its own ten-clean-pass planning certification.
This master roadmap is a new integration document and does not inherit that
certification. Any material roadmap revision resets its own clean-pass count.

## 2. Product goal

MechoFly will eventually provide every user-visible and scientifically relevant
DesktopFly capability, then exceed that reference in causal auditability,
connectome scale, experimental tooling, compute portability, behavioral
realism, accessibility, and safe local learning.

Parity is a floor, not the architecture. MechoFly remains an independently
authored Rust implementation. Public behavior, scientific claims, and
user-approved visual geometry are acceptance references; source code, assets,
and restricted data are not copied.

The accepted Drosophila companion silhouette, orientation, gait readability,
and motion character remain visual acceptance references during the Rust
reinterpretation. Scientific corrections such as two functional wings,
halteres, parked rest wings, and coherent grooming take precedence over an
older decorative inconsistency and must be documented.

The product has three coordinated surfaces:

1. **Companion** — a transparent, calm, responsive desktop animal.
2. **Neural Observatory** — an auditable view of structure, modeled dynamics,
   interventions, behaviors, counterfactuals, and uncertainty.
3. **Research runtime** — deterministic model packs, reproducible experiments,
   exact provenance, CPU/GPU equivalence, and exportable evidence.

## 3. Non-negotiable boundaries

### 3.1 Scientific truthfulness

Every visible state and exported record must identify its layer:

```text
DERIVED_CONNECTOME_STRUCTURE
MODELED_NEURAL_DYNAMICS
DERIVED_BEHAVIOR_CONTROLLER_STATE
MODELED_SOFTWARE_LEARNING
AUTHORED_PRESENTATION
ENGINEERING_GUARDRAIL
UNRESOLVED_CALIBRATION_PARAMETER
```

A wiring diagram does not supply membrane dynamics, complete synaptic signs,
neuromodulation, embodiment, behavior, or learning. MechoFly must never label a
modeled spike as a recording or a software policy update as biological
plasticity.

### 3.2 Neural authority

Normal neural-driven behavior must follow one inspectable causal chain:

```text
environment or authored stimulus
  -> sensory transform
  -> identified model inputs
  -> modeled circuit activity
  -> behavior evidence
  -> duration-aware controller
  -> motor program
  -> authored pose
```

No hidden animation timer may independently command escape, walking, reverse,
grooming, nervous darting, or sleep while the UI claims the brain caused it.
Demo shortcuts may author a stimulus or explicitly preview a pose, but the UI
and receipts must distinguish those operations.

If the neural runtime is paused, stopped, invalid, late, or disconnected, all
neural-driven locomotion must stop within one bounded model step and fail closed
to `quiet_wake` or another declared stationary safe state. The user interface,
emergency stop, diagnostics, and recovery controls must remain available.

### 3.3 Independent implementation and data licensing

- MechoFly remains Rust-first and has no C# or Swift runtime dependency.
- Drosophila Natural is the default on every machine profile, including AI100.
- Firefly Lantern is optional presentation art and is off by default.
- Skins never change graph, dynamics, behavior evidence, learning, or claims.
- FlyWire-derived data keeps its own provenance, citation, and license. It is
  not silently redistributed under the software's MIT license.
- All sensors are local, minimal, visible, permission-aware, and individually
  disableable. No screen content, keystroke content, or cloud inference is
  required for the companion.

### 3.4 Determinism and intervention safety

- Identical model pack, seed, state, sensory sequence, timing, and backend
  produce the declared deterministic result.
- CPU and GPU must pass exact or explicitly bounded equivalence fixtures before
  GPU execution is accepted.
- Replay and counterfactual branches clone all causally relevant state and
  cannot mutate the live session.
- Interventions are bounded, receipted, cancellable, and have no live biological
  hardware authority.
- Learning is separate from the imported connectome, explicit-feedback-only by
  default, bounded, resettable, exportable, and deleteable.

## 4. Current product state

Status labels in this table are intentionally conservative.

| Area | Current assessment | Meaning |
| --- | --- | --- |
| Published `main` | Legacy C# baseline | Current default branch is not the target architecture |
| Rust replacement | In-flight candidate | PRs #3, #4, and #5 are stacked and not yet consolidated to `main` |
| Transparent Windows pet | Candidate | Native per-pixel alpha and hit testing exist on the Rust line; exact-head AI100 review remains required |
| Drosophila and Firefly skins | Candidate | Drosophila is the application default, but the AI100 Firefly default must be removed |
| Global controls | Candidate | Eight Windows bindings exist; runtime evidence at the exact final head is required |
| Neural Observatory | Candidate | Dark v3 interface exists; biological identity mapping and full parity remain incomplete |
| Replay and bounded preview | Candidate | Multi-frame actual-versus-alternative inspection exists; async/cancel/export hardening remains |
| Adaptive compute | Candidate | Auto/CPU/cross-vendor `wgpu` paths exist; exact-head hardware evidence remains required |
| FAFB ingestion | Candidate | Local CSV/CSV.GZ import and manifests exist; full-scale performance and biological registration remain unproven |
| Pet learning | Candidate experimental layer | Separate bounded policy exists; it is not biological learning and is not yet a release claim |
| Desktop ecology | Not accepted | Window terrain, cursor loom, taps, typing vibration, thermal state, circadian rhythm, and multi-display behavior are not parity-complete |
| Causal behavioral parity | Not accepted | Current authored presentations do not prove sensory-to-neuron-to-motor authority |
| Ethological temporal realism | Not accepted | V10 semi-Markov behavior dynamics, grooming hierarchy, wing parking, and guarded sleep criteria remain to be implemented |

The detailed feature-by-feature contract is in
[DesktopFly parity and beyond](DESKTOPFLY_PARITY.md).

## 5. Delivery order

The sequence is dependency-driven. Later work must not hide an incomplete
causal or validation foundation.

### Gate 0 — Freeze, review, and consolidate the Rust foundation

1. Install the exact PR #5 candidate on AI100 without moving its reviewed head.
2. Review full-resolution transparent-pet and Neural Observatory evidence.
3. Verify all eight shortcuts, tray commands, CPU/Auto/GPU selection,
   transparent hit testing, start/stop/restart, and source identity.
4. Resolve any visual or interaction defects on a new exact candidate.
5. Require Windows CI, strict Clippy, formatting, deterministic tests, release
   build, PowerShell 5.1 parsing, GUI startup, and safety self-test at that exact
   candidate.
6. Consolidate the stacked Rust replacement to `main` only after the accepted
   candidate and its evidence agree exactly.

**Exit gate:** one Rust `main`, clean source identity, no legacy runtime, no
unreviewed candidate claim, and a reproducible Windows evidence packet.

### Gate 1 — Neural Authority and fail-closed motor control

1. Make one runtime owner authoritative for modeled state and neural-derived
   behavior evidence.
2. Separate authored stimulus commands from authored pose previews.
3. Route cursor loom, escape test, grooming test, reverse test, and walk test
   through declared stimulus channels when a neural claim is shown.
4. Add a receipt spanning sensory input, target neurons, spikes/rates,
   behavior decision, controller transition, and pose.
5. Stop all neural-driven locomotion within one bounded step when the model is
   paused, dead, invalid, or disconnected.
6. Add negative controls: no Giant Fiber spike means no neurally claimed escape;
   no MDN evidence means no neurally claimed reverse; no grooming evidence means
   no neurally claimed grooming.

**Exit gate:** causal end-to-end tests pass, stopping the brain stops claimed
behavior, and no pose timer can masquerade as neural output.

### Gate 2 — Ethological temporal realism and motion coherence

Implement the V10 Ethological Behavior Dynamics Layer between neural evidence
and visible motion:

- behavioral intent with filtered evidence and entry/exit hysteresis;
- an explicit-duration hierarchical semi-Markov controller;
- macro-state duration, minimum dwell, interruptibility, refractory state, and
  transition provenance;
- nested grooming programs for head, foreleg, antenna/eye, abdomen, wing, and
  hindleg actions;
- explicit `escape_prepare`, `escape`, `escape_recover`, `quiet_wake`, `rest`,
  and guarded `sleep_like` states;
- state-specific pose graphs and display-rate interpolation;
- parked wings and no decorative bob/trails during rest or walking;
- deterministic controller serialization and exact counterfactual cloning.

The initial 1.5-second grooming macro-state floor is an engineering guardrail,
not a biological constant. Sleep-like labeling requires at least five minutes
of immobility plus a modeled arousal-threshold criterion; a short pause is not
sleep.

Initial calibration priors, all subject to provenance-controlled fitting:

| Quantity | Initial value or range | Status |
| --- | ---: | --- |
| Individual grooming sweep/rub cycle | 150–200 ms | Empirically informed prior |
| Anterior grooming sub-bout center | near 1 s | Empirically informed prior |
| Abdomen/wing-cleaning sub-bout center | near 250 ms | Empirically informed prior |
| Head-sweep/front-leg-rub alternation | about 1.7–3.3 s per bout | Empirically informed prior |
| Escape preparation visual envelope | about 200 ms | Empirically informed prior |
| Grooming macro minimum dwell | 1.5 s | Temporary engineering guardrail |

Minimum V10 verification includes 30-minute fixed-context scenarios, at least
20 fixed seeds, two repeated runs per seed for byte equality, bout-distribution
and transition telemetry, adversarial threshold chattering, serialization,
counterfactual cloning, visual fixtures, and renderer-rate independence. For a
ten-second uninterrupted rest fixture, wing phase and translation remain
unchanged, periodic wing/body oscillation is absent, wing-tip raster drift is
at most 0.5 px, and body-center RMS drift is at most 0.25 px except during an
explicit receipted micro-action.

Initial controller budgets are no more than 5% CPU overhead relative to the
instrumented pre-controller scenario, no more than 10 MiB steady-state
additional memory, a bounded transition ledger, and no undeclared regression
of the published 99th-percentile model-step latency. A budget may change only
with measured justification recorded in the same evidence set.

**Exit gate:** fixed-seed duration, grooming, escape, rest, wing, replay,
counterfactual, and visual-regression suites pass the V10 acceptance contract.

### Gate 3 — Desktop ecology and embodied closed loop

Gates 3 and 4 are a coupled program. Sensor adapters may be implemented first,
but Gate 3 cannot exit until the compatibility pack identity and relevant
sensory-to-descending-neuron pathways in Gate 4 have passed.

Implement, in a permission-minimal and individually disableable way:

- cursor position, velocity, approach angle, and looming transform;
- window top-edge terrain, landing, walking, dragged-window riding, appearing
  window looms, and support-loss response when a window closes;
- nearby mouse clicks as substrate taps without swallowing desktop input;
- typing activity timing as vibration, never key identity or content;
- local thermal state as an explicitly modeled rate modifier;
- time-of-day/circadian drive, midday siesta, night quiescence, wake response,
  and post-wake grooming;
- a grounded tripod gait with explicit contact phases and no foot skating;
- gait-to-ascending-neuron proprioceptive feedback and fast-cursor wind input;
- multi-monitor topology and user-directed display movement.

Environmental channels must have visible off switches, units, rate limits,
calibration metadata, deterministic replay inputs, and privacy documentation.
Computer thermal state is a proxy input, not a measurement of an insect's body
temperature, and must be labeled accordingly.

**Exit gate:** every ecology feature has an end-to-end sensory-to-behavior
receipt, negative controls, permission/privacy tests, and deterministic replay.

### Gate 4 — DesktopFly-compatible circuit pack

Build an independently regenerated, provenance-controlled compatibility pack
for the public DesktopFly behavior circuit:

- 668 identified neurons and the verified exact connection count (expected
  18,968 for the accepted source transform, otherwise display the observed
  count rather than forcing it);
- LC4, LPLC2, DNp01/Giant Fiber, DNa01/DNa02, DNp09, DNg11, MDN, and
  DNp02/DNp04/DNp11 populations plus declared partners;
- synapse counts, neurotransmitter-prediction signs, delays, transforms, and
  every modeling assumption in a versioned manifest;
- a 1 ms internal simulation option for the compatibility experiment while
  keeping rendering and UI clocks decoupled;
- independent ETL from permitted FlyWire sources, with source hashes,
  citations, licenses, tests, and no copied restricted data.

This is one model pack, not the universal MechoFly brain. It exists to make
parity experiments reproducible and falsifiable.

**Exit gate:** graph identity, population membership, transform, deterministic
fixtures, resting Giant Fiber silence, loom latency distribution, negative
controls, and behavior mappings pass at the exact pack hash.

### Gate 5 — Neural Observatory parity and experimental superiority

Complete the parity surface and then extend it:

- 23,210 soma points with FlyWire super-class coloring and explicit source
  identity;
- modeled spikes at real locations only for neurons with verified root-ID
  registration; unmapped nodes stay visibly unmapped;
- rotation, hover-to-pause, click selection, root/index search, and strongest
  inbound/outbound neighborhoods;
- bounded spatial stimulation of the nearest eligible modeled neurons;
- behavior, controller, sensory, spike, uncertainty, and intervention lanes;
- asynchronous cancellable analysis with progress, timeouts, stable analysis
  IDs, bounded resource use, and restart-safe receipts;
- actual-versus-counterfactual multi-frame comparison from exact cloned state;
- JSON, CSV, PNG, deterministic snapshot, and short replay export;
- evidence-level labels and a static reduced-motion alternative.

The compatibility experiment surface must include a named preset matching the
public reference interaction: approximately 60 nearest eligible circuit
neurons for 400 ms, still subject to MechoFly's preview-only dosage and
live-state-isolation gates. The Giant Fiber pair must have persistent,
accessible markers when that model pack is active.

**Exit gate:** a user can reproduce every displayed causal claim from exported
identities, inputs, state, and receipts; unmapped or unsupported claims fail
closed instead of being visually implied.

### Gate 6 — Scalable connectome research runtime

1. Support exact imported FAFB v783 sessions without silent truncation. The
   current expected Codex snapshot is 139,255 neurons and 3,732,460 filtered
   connection rows; the runtime must report the observed manifest counts and
   reject a mismatched dataset rather than forcing these expectations.
2. Keep named synthetic tiers clearly separate from derived connectome packs.
3. Auto-detect usable CPU/GPU capacity and choose only at session boundaries.
4. Preserve exact CPU/GPU fixtures and a complete CPU fallback on AMD, Intel,
   NVIDIA, and conformant software adapters.
5. Add bounded memory planning, streaming import, checkpoint budgets,
   cancellation, progress, timeouts, and deterministic out-of-memory refusal.
6. Benchmark model-step latency, energy, memory, startup, replay, and export on
   fixed hardware profiles.
7. Add an explicitly experimental brain–VNC–body closed-loop pack when source
   data and a validated neuromechanical interface are available. Brain, VNC,
   body, and authored presentation identities must remain separately hashed.

**Exit gate:** full-scale sessions either meet their published budgets or refuse
clearly; no marketing statement substitutes for a benchmark receipt.

### Gate 7 — Smart companion learning without false biology

1. Finish the bounded explicit-feedback contextual policy as the safe default.
2. Add opt-in local preference learning for calmness, distance, activity
   windows, and favored non-emergency actions.
3. Prevent learning from suppressing emergency, liveness, accessibility, or
   user-control invariants.
4. Keep policy state separate, bounded, inspectable, resettable, exportable,
   and deleteable.
5. Evaluate learning with preregistered regret, stability, user-control,
   rollback, and distribution-shift tests.
6. Consider an experimental mushroom-body model only as a separately enabled,
   cited, calibrated research mode.

**Exit gate:** learning measurably improves the declared companion objective
without altering connectome evidence, violating controls, or exceeding its
claim label.

### Gate 8 — Multi-fly, platform, accessibility, and adoption

- Multiple companions with clear modes: one brain with labeled follower pets,
  or independent brains with explicit resource budgets and identities.
- Windows production quality first; then native macOS feature parity, Linux
  desktop support, and later Android/iOS companions sharing the Rust core where
  platform constraints permit.
- Multi-monitor persistence, DPI changes, sleep/wake, session restart, and
  display hot-plug handling.
- Reduced motion, high contrast, color-vision-safe palettes, remappable
  keyboard controls, screen-reader labels, phobia-safe abstract mode, and no
  high-frequency flashes.
- Companion, Explorer, and Classroom/Lab lanes with reproducible seeds,
  lessons, shareable replay cards, and clear limitations.
- Local-first operation, no mandatory account, no cloud telemetry by default,
  and no engagement dark patterns.

**Exit gate:** accessibility, privacy, crash-free, battery/CPU, first-run,
retention, comprehension, and sharing studies meet preregistered thresholds.

## 6. Priority map

| Priority | Work | Why |
| --- | --- | --- |
| P0 | Exact Rust candidate review and consolidation | Prevents building on an unaccepted or moving foundation |
| P0 | Neural authority and fail-closed movement | Required before any neural-causation claim |
| P0 | V10 ethological controller and rest/wing repair | Removes visible biological contradictions and state flicker |
| P1 | Cursor/window/click/typing/thermal/circadian ecology | Delivers the advertised DesktopFly companion experience |
| P1 | 668-neuron compatibility pack and closed-loop embodiment | Makes parity causal and reproducible rather than cosmetic |
| P1 | Brain interaction, real-location registration, and exports | Completes the public experiment surface |
| P1 | Multi-display and multi-fly controls | Completes desktop usability parity |
| P2 | Full FAFB scale and advanced analyses | Extends beyond the compact parity circuit |
| P2 | Bounded personalized learning | Creates the smarter pet without corrupting scientific evidence |
| P2 | macOS/Linux/mobile and adoption program | Expands reach after correctness and usability are defensible |

## 7. Program-level acceptance gates

No milestone is called complete merely because code exists.

### Exact identity

- branch, commit, tree, dependency lock, model-pack hash, executable hash, and
  evidence archive agree;
- clean worktree and non-moving remote head are proved before and after build;
- no force push is used for an evidence-bearing branch.

### Verification

- canonical formatting;
- strict linting with warnings denied;
- unit, property, metamorphic, integration, visual, adversarial, and performance
  suites appropriate to the change;
- Windows PowerShell 5.1 parser checks for deployment tooling;
- real GUI startup and interaction evidence on target hardware;
- at least ten consecutive unchanged clean review passes when the governing
  release protocol requires them.

### Scientific and causal validity

- every structural source and transform is versioned and hashed;
- every causal behavior claim has an inspectable end-to-end receipt;
- negative and ablation controls are present;
- train/calibration/validation splits avoid individual, source-video, and
  adjacent-frame leakage;
- held-out sources and distribution-shift results are reported;
- uncertainty and unresolved parameters remain visible.

### Product validity

- transparent surfaces do not block invisible desktop regions;
- user controls, pause, hide, emergency stop, and exit always work;
- stillness looks alive but calm, not frozen or perpetually flapping;
- resource use is bounded and measured;
- privacy and accessibility defaults are tested;
- first-run and upgrade paths are recoverable.

## 8. Immediate next actions

1. Keep PR #5's exact candidate unchanged while AI100 evidence is reviewed.
2. Land this roadmap through a documentation-only child branch.
3. After Rust consolidation, make Drosophila Natural the AI100 default and keep
   Firefly Lantern opt-in.
4. Implement Neural Authority and fail-closed motor control before adding new
   behavioral spectacle.
5. Implement the V10 duration-aware behavior controller and rest/wing policy.
6. Add deterministic sensory input records and begin cursor/window ecology.
7. Build the independently regenerated 668-neuron compatibility pack.
8. Complete spatial Brain Lab stimulation, identity registration, cancellation,
   progress, and exports.

The roadmap should be revised when evidence changes the dependency order, not
when a feature merely becomes fashionable.
