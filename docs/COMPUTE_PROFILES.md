# Compute modes and model tiers

## Modes

Startup always runs in capacity-evaluation mode. The user-visible preference
can remain `Auto` or can constrain the evaluation to `CPU` / prefer `GPU`.

- `Auto` performs short CPU and, when available, GPU calibration runs against
  the same integer kernel. It chooses the faster mode that satisfies a 33 ms
  step budget. No GPU is required.
- `CPU` uses deterministic parallel gather over incoming CSR rows.
- `GPU` uses WGSL compute through `wgpu`. On Windows this permits Direct3D 12
  or Vulkan adapters from AMD, Intel, NVIDIA, and conformant software drivers.
  Selection is capability- and benchmark-based, never vendor-based.

An explicitly preferred GPU mode falls back to CPU with a visible reason if no
compute-capable adapter exists. CPU is a complete runtime backend, not an error
or reduced-functionality mode. The selected adapter, backend, limits,
calibration durations, and fallback reason are recorded.

Brain Lab exposes **Re-evaluate capacity**. It reruns the benchmarks and may
select a different backend and named tier. A change closes the old replay epoch
and starts a new identified session; it never resizes a running scientific
comparison in place.

## Named tiers

Automatic sizing selects a named, reproducible tier only at session start:

| Tier | Structure | Intended use |
| --- | --- | --- |
| `demo-4096` | deterministic synthetic graph | low-power and diagnostics |
| `standard-12615` | deterministic synthetic graph or matching declared pack | normal companion |
| `extended-65536` | deterministic synthetic graph | high-throughput visual/model stress |
| `fafb-v783-full` | exact imported FAFB graph | explicit research session only |

Automatic sizing never truncates or randomly samples a measured/derived
connectome. Imported subsets require an authored selection rule and their own
manifest and hash. A scientific comparison pins its tier and backend for the
whole comparison; neither can change mid-run.

All tiers use the same signed fixed-point update order. Incoming CSR gather
avoids unordered floating-point atomics, so the CPU and GPU kernels can be
checked against exact fixtures.
