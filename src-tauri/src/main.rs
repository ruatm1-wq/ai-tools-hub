#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--cli" || a == "-c") {
        // CLI mode — instant terminal chat
        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
        rt.block_on(ai_tools_hub::cli::run_cli());
    } else {
        // Normal GUI mode
        ai_tools_hub::run();
    }
}
