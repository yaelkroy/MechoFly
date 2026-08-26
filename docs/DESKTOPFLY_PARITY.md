# DesktopFly parity and beyond contract

**Status:** acceptance contract; not an implementation-complete claim  
**Updated:** 2026-08-26  
**MechoFly target:** independent Rust implementation  
**External reference:** `DenisSergeevitch/desktop-fly` `master` at
`7014d37d7e252a3f16b173aca9b49f6f6c91d3b9`

## 1. Purpose

MechoFly is required to deliver every material DesktopFly capability described
in the public README and product announcement, then exceed it. This document
turns that requirement into an auditable acceptance matrix.

The matrix does not authorize copying DesktopFly source, artwork, or data.
MechoFly implements the capability contract independently in Rust and preserves
all upstream data licenses and citations.

## 2. Status vocabulary

| Status | Meaning |
| --- | --- |
| `CANDIDATE` | Present on the in-flight Rust line, but not a released or exact-head accepted claim |
| `PARTIAL` | Some enabling work exists; the reference behavior or evidence gate is incomplete |
| `PLANNED` | Required and not accepted yet |
| `BEYOND` | Required MechoFly extension beyond DesktopFly parity |
| `ACCEPTED` | May be used only after the listed evidence exists at an exact immutable identity |

No row becomes `ACCEPTED` through documentation alone.

## 3. Desktop companion and controls

| Reference capability | MechoFly contract | Current status | Acceptance evidence |
| --- | --- | --- | --- |
| Transparent desktop overlay | Render only the insect and its intentional effects; no visible rectangle or chroma-key background | `CANDIDATE` on Windows | Per-pixel alpha screenshots, alpha-mask test, and desktop interaction capture at exact build hash |
| Click-through empty space | Zero-alpha pixels never intercept mouse input | `CANDIDATE` | Automated `WM_NCHITTEST` mask fixture plus real desktop click-through evidence |
| Visible insect interaction | Visible pixels can receive the intended clicks while holes pass through | `CANDIDATE` | Pixel-boundary hit-test matrix at multiple DPI scales |
| System menu/tray controls | Pause/resume, show/hide brain, test stimulus, display move, multi-fly controls, scare/startle, and exit are discoverable | `PARTIAL` | Tray command inventory and end-to-end action tests |
| Pause/resume | Freeze model, controller, and world under a declared pause policy without corrupting time or replay | `PLANNED` | Pause/resume determinism, long-pause, sleep/wake, and replay-boundary tests |
| Show/hide brain | Toggle Neural Observatory without restarting or obscuring the pet | `CANDIDATE` | Shortcut, tray, double-click/right-click, close/reopen, and focus tests |
| Global shortcuts | Controls work outside the app and recover visibly from registration conflicts | `CANDIDATE` | All eight bindings, conflict fallback, reserved-key, focus, and exit tests |
| Wandering | The companion autonomously explores within bounded user-configurable limits | `PARTIAL` | Fixed-seed trajectories, edge avoidance, no-stuck property, and user boundary tests |
| Walking direction | Motion advances in the direction the head/body faces; reverse is visibly distinct | `CANDIDATE` presentation | Trajectory/pose agreement and reverse-specific fixtures |
| Tripod gait | Ground walking uses an inspectable alternating tripod pattern with stable contacts and no foot skating | `PLANNED` | Contact-phase oracle, speed sweep, turn, reverse, and frame-rate-independence tests |
| Procedural Drosophila body | Independently authored Rust geometry preserves the accepted compact fly silhouette while exposing scientifically corrected anatomy | `PARTIAL` | Visual fixtures for six legs, two functional wings, halteres, proportions, and all primary poses |
| Grooming | The fly stops translating and executes anatomically coherent grooming bouts | `PARTIAL` | V10 macro/substate duration, body-region pose, interruption, and visual tests |
| Rest | Standing rest has parked wings, no periodic bob, no trails, and no translation | `CANDIDATE` visual policy | Ten-second deterministic rest fixture and reduced-motion comparison |
| Sleep | Night inactivity is labeled sleep-like only after the operational criterion and modeled arousal change; slow abdomen ventilation is an explicit sparse pose, not generic body bob | `PLANNED` | At least five-minute immobility plus arousal-threshold, ventilation-pose, wake, and false-positive tests |
| Flight and landing | Escape has prepare, takeoff, flight, recovery, and settling stages | `PARTIAL` | Neural authority receipt, continuous trajectory, altitude, wing, and landing tests |
| Move to next display | User can move a companion across connected monitors | `PLANNED` | Monitor hot-plug, scale/orientation, negative coordinates, and persistence tests |
| Multiple flies | Add/remove companions with explicit brain ownership and bounded resources | `PLANNED` | Identity, isolation, synchronized scare, add/remove, persistence, and resource tests |
| Scare all flies | One command delivers a declared startle stimulus to every eligible companion | `PLANNED` | Per-fly input and downstream response receipts; no direct hidden flight command |
| Reduced motion | Remove nonessential animation without changing model or behavior timing | `CANDIDATE` | Metamorphic neural/controller equality plus visual accessibility audit |
| Drosophila default | Scientifically aligned Drosophila art is the default on all profiles | `PARTIAL` | Repository, application, installer, AI100, upgrade, and reset-default tests |
| Optional Firefly art | Firefly Lantern remains opt-in presentation only and never changes model claims | `PARTIAL` | Default-off proof, skin-switch state equality, and persistent provenance label |

## 4. Desktop ecology and sensing

| Reference capability | MechoFly contract | Current status | Acceptance evidence |
| --- | --- | --- | --- |
| Cursor approach | Sample cursor position and velocity locally and convert approach geometry into a bounded sensory input | `PLANNED` | Units, rate, calibration, replay record, off switch, and privacy test |
| Loom-sensitive escape | Fast looming can produce circuit activity and Giant Fiber escape; slow approach can be tolerated | `PLANNED` | Sensory-to-LC4/LPLC2-to-GF-to-escape receipt, latency distribution, and slow/fast controls |
| No scripted neural escape | A neurally labeled escape occurs only after the declared descending escape evidence | `PLANNED`, P0 | GF ablation, below-threshold, paused-brain, and no-spike negative tests |
| Window top-edge terrain | Window edges act as ledges for landing and walking | `PLANNED` | Geometry, occlusion, DPI, minimized/maximized, and multi-display fixtures |
| Ride dragged windows | A supported fly moves with a dragged window without teleporting | `PLANNED` | Continuous support-frame transform and detach tests |
| Window closes underneath | Support loss produces a declared sensory event and safe recovery/startle behavior | `PLANNED` | Close/minimize race, stale handle, and replay tests |
| Appearing-window loom | Nearby new windows feed the looming pathway | `PLANNED` | Distance/velocity transform, rate limiting, false-positive, and off-switch tests |
| Mouse click as substrate tap | A nearby click becomes a local tap/wind stimulus without intercepting the click | `PLANNED` | Click-through, spatial falloff, debounce, and downstream neural receipt |
| Typing activity as vibration | Detect activity timing only; never key identity or content | `PLANNED` | Privacy review, no-key-content proof, rate limiting, off switch, and replay surrogate |
| Thermal rate modulation | Local thermal state is a labeled proxy that modifies declared ectotherm parameters monotonically and within safe bounds | `PLANNED` | Sensor provenance, units, direction, clamping, unavailable-sensor fallback, and sensitivity tests |
| Circadian rhythm | Local time drives dawn/dusk activity, midday siesta, and night quiescence through modeled context | `PLANNED` | Time-zone/DST, clock-jump, fixed-time, disabled-mode, and occupancy tests |
| Wake grooming | Waking can increase grooming intent through the controller rather than a direct animation | `PLANNED` | State-history and causal transition receipt |
| Nervous darting | Looming-population evidence raises a duration-aware arousal state that can produce bounded corrections without perpetual wing flapping | `PLANNED` | Rate sweep, decay, wing-negative-control, and trajectory tests |
| Gait proprioception | Leg phase feeds declared ascending/proprioceptive model inputs | `PLANNED` | Phase-coupling, ablation, delay, and closed-loop stability tests |
| Fast-cursor wind | Cursor motion feeds declared sensory/wind partners separately from loom geometry | `PLANNED` | Channel separation, ablation, saturation, and replay tests |
| Permission-minimal operation | Base companion works locally without administrator rights, with visible and disableable sensor channels | `PLANNED` program gate | Platform permission inventory and cold-start test with every optional channel off |

## 5. Neural model and behavior authority

| Reference capability | MechoFly contract | Current status | Acceptance evidence |
| --- | --- | --- | --- |
| 668-neuron circuit | Provide an independently regenerated, versioned compatibility pack with verified identities | `PLANNED` | Pack hash, 668 unique roots, membership manifest, ETL, and source receipts |
| About 19,000 real connections | Preserve the exact independently observed count; expected 18,968 for the accepted transform | `PLANNED` | Source/target/synapse transform audit and exact edge-count fixture |
| Real synapse counts | Structural strengths come from the declared FlyWire source and are never relabeled as conductance | `PLANNED` | Random sample reconciliation and full aggregate checks |
| Predicted neurotransmitter signs | Signs and unknowns retain prediction source, confidence, transform, and limitations | `PLANNED` | Cell-level provenance, unknown handling, and sign-ablation tests |
| 1 kHz LIF compatibility simulation | Offer a 1 ms internal-step compatibility experiment decoupled from UI/render clocks | `PLANNED` | Step-size convergence, deterministic replay, timing, and performance evidence |
| LC4 and LPLC2 looming populations | Identified sensory populations receive the declared looming transform | `PLANNED` | Population counts, root IDs, stimulus-response, and ablation fixtures |
| DNp01/Giant Fiber escape | Modeled GF evidence is authoritative for neurally claimed escape | `PLANNED`, P0 | Rest silence, loom response, direct bounded stimulation, and GF-ablation controls |
| DNa01/DNa02 steering | Bilateral rate difference drives bounded steering intent | `PLANNED` | Left/right stimulation, sign reversal, symmetry, and trajectory tests |
| DNp09 forward walking | Declared activity controls walk intent and speed | `PLANNED` | Rate sweep, zero-rate, saturation, and controller-persistence tests |
| DNg11 grooming | Declared activity activates a duration-aware grooming program | `PLANNED` | Direct stimulation, ablation, macro/substate, and interruption tests |
| MDN reverse walking | Declared bursts activate reverse locomotion | `PLANNED` | Burst/no-burst, bilateral, duration, and forward/reverse exclusion tests |
| DNp02/04/11 wing/escape effort | Declared evidence affects threat wing posture or flight effort only in valid contexts | `PLANNED` | Rest-wing negative control and flight-context response tests |
| Strong partners | Ascending, sensory, and other included partners retain exact selection rule and provenance | `PLANNED` | Topology reconstruction and selection-rule audit |
| Whole-population arousal | Spontaneous takeoff, if retained, uses an explicit calibrated arousal rule | `PLANNED` | Rate, false-positive, seed, context, and disabling tests |
| Brain pause/death policy | Paused, failed, late, or invalid neural state stops claimed locomotion within one bounded step | `PLANNED`, P0 | Kill/pause/corruption/timeout tests with stationary fail-closed state |
| Manual test semantics | Test commands author stimuli; pose-only previews are explicitly labeled and cannot prove neural causation | `PARTIAL` | UI labels, receipts, and code-path isolation tests |
| Behavior memory | Neural evidence feeds hysteretic explicit-duration controller state, not a framewise label switch | `PLANNED`, P0 | Dwell, refractory, chattering, serialization, and replay properties |
| Controller counterfactual state | Actual and alternative branches clone controller, RNG, grooming, and all causal history | `PARTIAL` | Full-state digest, branch isolation, and discarded-branch mutation tests |

## 6. Brain and experiment interface

| Reference capability | MechoFly contract | Current status | Acceptance evidence |
| --- | --- | --- | --- |
| 23,210 soma context points | Display the exact derived atlas with snapshot, hash, citation, and `NOT SIMULATED` status | `CANDIDATE` | Point count/hash and attribution fixture |
| FlyWire super-class colors | Color anatomical context by verified super-class with accessible redundant encoding | `PLANNED` | Root-to-class audit, legend, unknown handling, and color-vision test |
| Spikes at real neuron locations | Draw activity at a soma only when the modeled root is identity-registered to that soma | `PLANNED` | Root-ID join audit; unmapped nodes never appear mapped |
| Rotating brain | Offer controllable anatomical rotation without tying rotation to simulation time | `PARTIAL` | Input, pause, determinism, and reduced-motion tests |
| Hover pauses rotation | Hover freezes presentation only, not neural time | `PLANNED` | Metamorphic model-state equality while hover changes rotation |
| Click a brain region | Select spatially nearest eligible neurons with radius/count shown before action | `PARTIAL` | Spatial-index oracle and boundary fixtures |
| Bounded stimulation | Author amplitude, duration, targets, purpose, and dosage on a deep-cloned checkpoint | `CANDIDATE` | Limit, clone, cancellation, and unchanged-live-digest tests |
| Downstream reaction | The displayed behavioral consequence comes from the alternative modeled trajectory | `PARTIAL` | Intervention-to-spike-to-controller-to-pose receipt |
| Giant Fiber/groom/turn experiments | Provide one-click reproducible recipes for escape, grooming, and bilateral steering | `PLANNED` | Exact model-pack recipes with expected ranges and negative controls |
| Hover/click synchronization | Canvas, root search, timeline, circuit list, and provenance share one selection | `CANDIDATE` | Cross-view selection and stale-session tests |
| Giant Fiber markers | The identified GF pair is persistently marked when the compatible pack is active, with non-color redundant encoding | `PLANNED` | Root identity, bilateral marker, legend, accessibility, and wrong-pack tests |
| Reference stimulation preset | Offer an explicit approximately-60-nearest-neurons / 400 ms compatibility preset within MechoFly's safer preview boundary | `PLANNED` | Spatial target oracle, exact duration, dosage rejection, and unchanged-live-state receipt |
| Live activity timeline | Show spike counts, activation, selected neurons, behavior, sensory input, and controller state | `PARTIAL` | Time alignment, units, scale stability, and export reconciliation |
| Pause/step/replay | Inspect retained state without changing the live session or learning policy | `CANDIDATE` | Play/pause/step cursor, epoch, retention, and digest tests |
| Actual vs alternative | Compare aligned multi-frame trajectories from exact shared state | `CANDIDATE` | Shared-origin and divergence receipts; no second-live-model implication |
| Explain action persistence | Expose neural evidence, dwell, duration draw, hysteresis, refractory state, and interruptibility | `PLANNED` | Controller-to-UI exact reconciliation |
| Async analysis | Long work is cancellable, bounded, progress-reporting, timeout-safe, and restart-identifiable | `PLANNED` | Cancellation latency, timeout, stable ID, restart, and resource tests |
| Export | JSON, CSV, PNG, deterministic snapshots, and short replay artifacts carry identities and claims | `PLANNED` | Round-trip, schema, hash, visual, and reproducibility tests |

## 7. Diagnostics, data, and reproducibility

| Reference capability | MechoFly contract | Current status | Acceptance evidence |
| --- | --- | --- | --- |
| Simulation self-test | Test circuit invariants, rest silence, stimulus latency, determinism, and bounds | `PARTIAL` | Versioned CLI receipt at exact executable/model hash |
| Behavior self-test | Exercise neuron/stimulus-to-controller-to-body mappings end to end | `PARTIAL` | Independent oracle and all required behavior recipes |
| Offscreen pet snapshot | Produce deterministic skin/pose fixtures without desktop capture | `PLANNED` | Byte or declared pixel-tolerance fixtures across supported renderers |
| Offscreen brain snapshot | Produce deterministic anatomical/activity fixtures with full provenance | `PLANNED` | Atlas/hash/session labels and image regression |
| Exact build identity | Record branch, commit, tree, clean state, lockfile, executable hash, and remote match | `CANDIDATE`, beyond reference | Pre/post build and pre-launch validation receipts |
| One evidence archive | Collect logs, identities, self-tests, machine capacity, and cropped application evidence | `CANDIDATE`, beyond reference | Schema validation and exact-file inventory |
| Data regeneration | Rebuild model packs from declared upstream resources through independent ETL | `PARTIAL` | Reproducible transform, hashes, counts, sampling audit, and license manifest |
| Data-license separation | FlyWire-derived data is not silently relicensed as MIT software | `CANDIDATE` policy | Package/license scan and download-time citation acknowledgement |
| Model-vs-measurement disclosure | UI, exports, docs, and receipts distinguish structure, dynamics, controller, learning, and art | `CANDIDATE` | Claim-lint and representative screenshot/export review |
| Deterministic replay | Same full input/state identity reproduces behavior and evidence | `CANDIDATE` foundation | Byte-equality across repeated runs and declared backend equivalence |
| Performance budgets | CPU, GPU, memory, startup, step latency, battery/energy, analysis, and export are measured | `PLANNED` | Fixed-machine benchmark suite with baselines and regression thresholds |

## 8. Required MechoFly capabilities beyond DesktopFly

These are not optional polish. They define why MechoFly should become the
stronger product rather than a port.

| Beyond-parity capability | Contract | Status | Acceptance evidence |
| --- | --- | --- | --- |
| Independent Rust core | One portable Rust implementation owns model, controller, replay, provenance, and policy | `CANDIDATE` | Consolidated Rust `main` with no legacy runtime dependency |
| Cross-vendor acceleration | Auto/CPU/GPU with capability-based AMD/Intel/NVIDIA selection and CPU fallback | `CANDIDATE` | Exact fixtures and hardware evidence per backend |
| Named model tiers | Reproducible tier selection only at session boundaries; no silent connectome truncation | `CANDIDATE` | Tier identity and boundary tests |
| Full FAFB sessions | Import and run the exact declared full graph when resources permit | `PARTIAL` | Full-scale hash/count/performance receipt or explicit bounded refusal |
| Rich provenance | Every source, transform, model, controller, intervention, and result is versioned | `CANDIDATE` | Export-to-runtime reconciliation |
| Multi-frame counterfactuals | Inspect aligned causal alternatives without live mutation | `CANDIDATE` | Clone/divergence/live-digest evidence |
| Duration-aware ethology | Hierarchical semi-Markov actions with empirically calibrated bout distributions | `PLANNED`, P0 | V10 unit/property/distribution/visual gates |
| Bounded local learning | Smart pet adapts from explicit feedback without rewriting connectome evidence | `CANDIDATE experimental` | Ledger, bounds, reset/export/delete, and stability tests |
| Experimental biological learning | Any mushroom-body/plasticity model is separate, cited, opt-in, and calibrated | `PLANNED` research-only | Held-out predictive checks and claim review |
| Accessibility modes | Reduced motion, high contrast, non-color encoding, remapping, screen reader, phobia-safe mode | `PARTIAL` | WCAG-oriented audit and assistive-technology tests |
| Privacy controls | Every sensor channel visible, local, rate-limited, disableable, and replayable as data | `PLANNED` | Permission and data-flow audit |
| Classroom/lab mode | Seeds, recipes, exports, limitations, and instructor reset support reproducible teaching | `PLANNED` | Lesson usability and reproduction study |
| Cross-platform core | Windows first, then macOS/Linux and later mobile surfaces without model divergence | `PLANNED` | Shared model fixtures and platform-specific interaction tests |
| Brain–VNC–body mode | Add a separately identified experimental closed-loop pack spanning brain, ventral nerve cord, neuromechanics, and sensors | `BEYOND` | Independent source hashes, interface contract, stability, ablation, and embodiment receipts |
| Safe failure model | Invalid state, compute loss, sensor loss, or stream death fails closed without losing controls | `PLANNED`, P0 | Fault-injection suite |
| Evidence-based adoption | Measure first-run success, retention, comprehension, sharing, stability, and resource complaints | `PLANNED` | Preregistered experiments; no popularity guarantee |

## 9. Parity release definition

MechoFly may claim “DesktopFly functional parity” only when all of the following
are true at one exact release identity:

1. Every non-`BEYOND` row in Sections 3–7 is `ACCEPTED` or has a public,
   explicitly scoped platform exception.
2. The 668-neuron compatibility pack and exact observed edge count are
   reproducible from permitted sources.
3. Cursor, window, click, typing-activity, thermal, circadian, and body-feedback
   channels have deterministic sensory records and end-to-end causal receipts.
4. No neurally labeled escape, walk, reverse, grooming, steering, or wing
   response can occur through an unrecorded direct animation command.
5. Pausing or killing the brain stops all neural-driven locomotion within one
   bounded step while controls remain responsive.
6. Brain points, root identities, super-classes, and displayed spikes are not
   conflated; unmapped model neurons remain visibly unmapped.
7. All desktop controls, multi-display operations, multi-fly operations,
   deterministic diagnostics, and exports pass on supported platforms.
8. Scientific labels, upstream citations, licenses, accessibility, privacy,
   performance, and recovery gates pass.
9. The exact release survives the governing unchanged clean-pass protocol.

A platform-scoped release may say **DesktopFly functional parity on Windows**
only after all applicable rows pass on Windows. An unqualified parity claim
also requires native macOS support for the reference platform and must name any
remaining platform-specific exceptions. Linux and mobile remain beyond-parity
expansion targets rather than shortcuts around the macOS obligation.

## 10. Beyond-parity release definition

MechoFly may claim it exceeds DesktopFly only for named, evidenced dimensions.
Examples include:

- exact CPU/GPU equivalence on a declared model pack;
- full-state multi-frame counterfactual replay;
- a duration-aware empirically calibrated behavior controller;
- full-scale FAFB execution within published budgets;
- stable asynchronous analysis with cancellation and exports;
- bounded inspectable local learning;
- superior provenance, accessibility, privacy, and reproducibility.

“More neurons,” “AI-powered,” or a more elaborate animation is not by itself a
scientific or product improvement.

## 11. Source ledger

- DesktopFly README and source tree at commit
  `7014d37d7e252a3f16b173aca9b49f6f6c91d3b9`.
- FlyWire whole-brain wiring diagram: Dorkenwald et al., *Nature* 634,
  124–138 (2024), DOI `10.1038/s41586-024-07558-y`.
- FlyWire whole-brain annotation: Schlegel et al., *Nature* 634, 139–152
  (2024), DOI `10.1038/s41586-024-07686-5`.
- Ethological design and evidence references are maintained in the V10 plan
  incorporated by [the master roadmap](ROADMAP.md).
