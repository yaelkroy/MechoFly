//! Build the unchanged application baseline harness without the desktop UI.
//! Production AI100 validation still uses the staged MechoFly executable.

#[path = "../../mechofly-app/src/behavior_baseline.rs"]
mod behavior_baseline;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .iter()
        .position(|arg| arg == "--behavior-baseline")
        .and_then(|index| args.get(index + 1));
    let Some(path) = path else {
        eprintln!("provide --behavior-baseline <output.json>");
        std::process::exit(2);
    };
    if let Err(error) = behavior_baseline::run(std::path::Path::new(path), &args) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
