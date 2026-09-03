//! Same native campaign and stimulus protocol without a desktop adapter.
#[path = "../../mechofly-app/src/behavior_baseline.rs"]
mod behavior_baseline;
#[path = "../../mechofly-app/src/behavior_campaign.rs"]
mod behavior_campaign;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|s| s == "--behavior-baseline") {
        let result = args
            .get(i + 1)
            .ok_or_else(|| "missing report path".to_owned())
            .and_then(|s| behavior_baseline::run(std::path::Path::new(s), &args));
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let result = args
        .iter()
        .position(|s| s == "--behavior-campaign")
        .and_then(|i| args.get(i + 1))
        .ok_or_else(|| "--behavior-campaign directory is required".to_owned())
        .and_then(|s| behavior_campaign::run(std::path::Path::new(s), &args));
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
