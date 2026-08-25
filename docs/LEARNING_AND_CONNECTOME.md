# Connectome and learning policy

## FAFB ingestion

MechoFly accepts user-downloaded FlyWire Codex resources; it does not bundle or
redistribute them. The recommended source is the static Codex resource URL:

`https://codex.flywire.ai/api/download_resource?data_product=connections_princeton&dataset=fafb`

An import creates a local manifest containing dataset (`fafb`), snapshot
(`v783`), product, source URL, retrieval time supplied by the user/workflow,
SHA-256, original column mapping, filter/threshold declaration, node and edge
counts, and MechoFly transform version. Root IDs stay 64-bit values. Repeated
source-target rows (for example, rows split by neuropil) are retained unless an
explicit transform says otherwise.

FAFB v783 currently describes 139,255 neurons and 3,732,460 filtered
connections in Codex. These numbers are validation expectations, not a license
to silently relabel another graph as FAFB.

## Defensible first learning layer

The first adaptive feature is a small contextual bandit over authored pet
actions such as pause, walk, inspect, and groom. Its inputs are local session
features and explicit `Encourage` / `Discourage` feedback. It does not inspect
screen contents, use a cloud model, alter the connectome, or claim biological
plasticity.

Each update appends a ledger record with context, selected action, feedback,
value before/after, learning-rule version, timestamp, and before/after policy
digests. Values and update counts are bounded. The policy has an off switch and
reset/export/delete controls.

A future experimental mushroom-body mode may implement a cited literature
model, but it must be separately enabled, calibrated, and labeled
`EXPERIMENTAL_MODELED_PLASTICITY`. It cannot be presented as a whole-fly
learning reconstruction.

## Companion-product lessons

Desktop-pet products demonstrate the value of varied animation, direct touch
interaction, local-first operation, adjustable compute, and user-controlled
customization. MechoFly adopts those interaction principles while keeping its
scientific labels and receipts visible. It does not adopt silent screen
capture, mandatory cloud inference, or the claim that a language model is a
fly brain.
