# Data provenance

The repository does not bundle FAFB, BANC, MANC, MAOL, MCNS, or another
connectome download. Built-in tiers are deterministic synthetic demo graphs and
are visibly labeled `SYNTHETIC_DEMO_TOPOLOGY`.

## Imported connection tables

Brain Lab accepts a user-selected CSV or CSV.GZ connection table. It recognizes
declared source-root, target-root, and synapse-count column aliases, preserves
64-bit root IDs, retains repeated source-target rows, builds deterministic
incoming CSR, and emits a manifest containing:

- provider dataset, snapshot, and product;
- source URL, local source filename, and retrieval marker;
- compressed/original source-file SHA-256;
- exact source/target/synapse column mapping;
- filter declaration;
- transform version;
- neuron and connection-row counts;
- transformed graph SHA-256;
- validation warnings; and
- measured-versus-modeled and citation-required flags.

The first transform maps unsigned structural strength into a bounded authored
model coefficient. That coefficient is part of `MODELED_NEURAL_DYNAMICS`; it is
not relabeled as a measured conductance or a known complete synaptic sign.

For FlyWire Codex products, follow the citation guidelines and principles shown
at download time. Do not redistribute downloaded material under MechoFly's MIT
software license. Dataset citations remain required regardless of software
authorship.

Primary sources:

- FlyWire whole-brain connectome: <https://www.nature.com/articles/s41586-024-07558-y>
- Codex static-download guidance: <https://codex.flywire.ai/faq>
- FlyWire citation guidance: <https://codex.flywire.ai/about_flywire>
- FlyWire principles: <https://flywire.ai/principles.html>
