# Recording-parity contract — Rust v5

The supplied original recording is the visual and interaction acceptance
reference for v5. This is an independent Rust implementation: behavior,
rendering, model views, safety boundaries, and receipts remain owned by this
repository.

## Animated companion

| Program | Required presentation |
|---|---|
| Rest | exact stillness; parked wings; no bob, aura, or trail |
| Walk | smooth horizontal translation; alternating six-leg gait; head leads |
| Reverse | backward translation while the body keeps its orientation |
| Groom | no translation; alternating foreleg wipes across eyes and antenna base |
| Pre-escape | orient away from the cursor; brace; rapid wing preparation |
| Flight | continuous 2D motion; smooth turns; the recording's independently animated wing pair |
| Landing | deceleration; reduced wing amplitude; leg extension; settle |

Cursor proximity is encoded into the modeled Loom population. The controller
enters the escape sequence only after that population crosses its activation
threshold; zero sensory input is regression-tested against escape entry. This
is a modeled software circuit claim, not a measured biological response.

The Prism skin must retain the recording's paired glass-wing set, visible
veins, orange eyes, dark-green head, orange pronotum, round green posterior
thorax, segmented lantern abdomen, twin green elytra, six jointed legs, two
antennae, and the faint airborne orbit cue. Flight wings stay parked and invisible
during rest, walking, reverse, grooming, alert, and quiet states. Drosophila
Natural uses the same behavioral articulation with a natural palette.

All movement is elapsed-time based. Tests compare refresh-rate trajectories and
require distinct rendered frames for walking, grooming, escape, flight, and
landing. Rest frames at different timestamps must be byte-identical.

## Live Brain

Live Brain is a separate compact window and includes:

- current behavior, frame, spikes, and behavior age;
- 23,210-point fixed FlyWire X–Y context projection;
- clearly labeled modeled-neuron activity overlay;
- pathway/model summary and viridis relative-activity scale;
- LC4 loom, LPLC2 loom, GF escape, DNA steer, MDN reverse, DNP09 walk,
  DNG11 groom, ESCw wing, and Landing bars;
- top active and selected-neuron details; and
- retained five-second spike raster.

Pausing Live Brain pauses only the view. It never pauses or targets the model.

## Brain Lab

The experiment window keeps all four reference work areas visible at once:

1. neuron search;
2. selected structural neighborhood;
3. paired modeled counterfactual; and
4. bounded replay plus stimulation preview.

The bottom strip shows the macro motor program and grooming/motor substate
timeline. Structural rows use the active graph's CSR edges and signed modeled
weights. Presentation role names are labeled as presentation groupings.

## Scientific and safety boundary

- Context anatomy is derived structure, not measured activity.
- Neural activity is deterministic modeled dynamics.
- Authored alternatives start from a retained full-state checkpoint, execute
  on CPU, are discarded, and have no apply path.
- A PASS comparison receipt requires an unchanged live-state digest.
- Learning remains a separately stored, explicit-feedback software policy and
  cannot alter graph edges or neural state.
- Autonomous policy choices enter only as bounded modeled-population drives;
  they cannot substitute a presentation behavior for the neural state.
- Losing or stopping neural authority cannot leave a hidden autonomous motor
  path.
