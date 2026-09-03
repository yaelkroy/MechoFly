use std::{fs, path::Path};

const NEURAL_SHADER: &str = "src/shaders/neural_step.wgsl";

fn main() {
    println!("cargo::rerun-if-changed={NEURAL_SHADER}");

    let path = Path::new(NEURAL_SHADER);
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|error| {
        panic!(
            "{} failed WGSL parsing:\n{}",
            path.display(),
            error.emit_to_string(&source)
        )
    });
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("{} failed WGSL validation: {error:#?}", path.display()));
}
