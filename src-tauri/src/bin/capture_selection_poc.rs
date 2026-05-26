use std::{env, thread, time::Duration};

fn main() {
    let delay_ms = env::args()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3000);

    thread::sleep(Duration::from_millis(delay_ms));

    let diagnostics = lexi_lib::selection::capture_selection_diagnostics();
    println!(
        "{}",
        serde_json::to_string_pretty(&diagnostics).expect("selection diagnostics should serialize")
    );
}
