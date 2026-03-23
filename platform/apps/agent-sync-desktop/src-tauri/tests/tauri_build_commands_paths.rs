use std::fs;
use std::path::PathBuf;

fn read_tauri_config() -> serde_json::Value {
    let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let content = fs::read_to_string(config_path).expect("failed to read tauri.conf.json");
    serde_json::from_str(&content).expect("tauri.conf.json must be valid json")
}

fn get_build_command(config: &serde_json::Value, key: &str) -> String {
    config["build"][key]
        .as_str()
        .unwrap_or_else(|| panic!("build.{key} must be a string"))
        .to_string()
}

fn assert_supports_ui_prefix_fallbacks(command: &str, key: &str, script: &str) {
    assert!(
        command.contains(&format!("--prefix . run {script}"))
            && command.contains(&format!("--prefix ./ui run {script}"))
            && command.contains(&format!("--prefix ../ui run {script}")),
        "{key} must support cwd fallbacks for ui package, got: {command}"
    );
}

#[test]
fn before_dev_command_targets_ui_package() {
    let config = read_tauri_config();
    let command = get_build_command(&config, "beforeDevCommand");
    assert_supports_ui_prefix_fallbacks(&command, "beforeDevCommand", "dev:tauri");
}

#[test]
fn before_build_command_targets_ui_package() {
    let config = read_tauri_config();
    let command = get_build_command(&config, "beforeBuildCommand");
    assert_supports_ui_prefix_fallbacks(&command, "beforeBuildCommand", "build:tauri");
}

#[test]
fn bundle_resources_do_not_include_dotagents_binary() {
    let config = read_tauri_config();
    let resources = config["bundle"]["resources"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let includes_dotagents = resources
        .iter()
        .filter_map(serde_json::Value::as_str)
        .any(|item| item.contains("dotagents"));

    assert!(
        !includes_dotagents,
        "bundle.resources must not include dotagents — runtime is invoked via npx"
    );
}
