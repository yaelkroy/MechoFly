struct Params {
    neuron_count: u32,
    frame_low: u32,
    frame_high: u32,
    seed_folded: u32,
}

struct U32Values { values: array<u32>, }
struct I32Values { values: array<i32>, }
struct ResultValue { activation: i32, spike: u32, }
struct ResultValues { values: array<ResultValue>, }

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> offsets: U32Values;
@group(0) @binding(2) var<storage, read> sources: U32Values;
@group(0) @binding(3) var<storage, read> weights: I32Values;
@group(0) @binding(4) var<storage, read> previous: I32Values;
@group(0) @binding(5) var<storage, read> stimulus: I32Values;
@group(0) @binding(6) var<storage, read_write> output: ResultValues;

fn mix32(initial: u32) -> u32 {
    var value = initial;
    value = value ^ (value >> 16u);
    value = value * 0x7feb352du;
    value = value ^ (value >> 15u);
    value = value * 0x846ca68bu;
    return value ^ (value >> 16u);
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let target = id.x;
    if target >= params.neuron_count {
        return;
    }

    var drive = clamp(stimulus.values[target], -8192, 8192);
    let start = offsets.values[target];
    let end = offsets.values[target + 1u];
    for (var edge = start; edge < end; edge = edge + 1u) {
        let source = sources.values[edge];
        let contribution = clamp(
            (previous.values[source] * weights.values[edge]) / 4096,
            -512,
            512,
        );
        drive = clamp(drive + contribution, -65536, 65536);
    }

    let noise_hash = mix32(
        params.seed_folded
            ^ ((params.frame_low << 17u) | (params.frame_low >> 15u))
            ^ ((params.frame_high << 7u) | (params.frame_high >> 25u))
            ^ (target * 0x9e3779b9u),
    );
    drive = clamp(drive + i32(noise_hash & 0x1ffu) - 256, -65536, 65536);

    let candidate = (previous.values[target] * 13 + drive * 3) / 16;
    if candidate > 8000 {
        output.values[target].activation = clamp(candidate - 10000, -32768, 32767);
        output.values[target].spike = 1u;
    } else {
        output.values[target].activation = clamp(candidate, -32768, 32767);
        output.values[target].spike = 0u;
    }
}
