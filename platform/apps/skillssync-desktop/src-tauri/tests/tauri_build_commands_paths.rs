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

#[test]
fn before_dev_command_targets_ui_package() {
    let config = read_tauri_config();
    let command = get_build_command(&config, "beforeDevCommand");
    assert!(
        command.contains("--prefix . run dev")
            && command.contains("--prefix ./ui run dev")
            && command.contains("--prefix ../ui run dev"),
        "beforeDevCommand must support cwd fallbacks for ui package, got: {command}"
    );
}

#[test]
fn before_build_command_targets_ui_package() {
    let config = read_tauri_config();
    let command = get_build_command(&config, "beforeBuildCommand");
    assert!(
        command.contains("--prefix . run build")
            && command.contains("--prefix ./ui run build")
            && command.contains("--prefix ../ui run build"),
        "beforeBuildCommand must support cwd fallbacks for ui package, got: {command}"
    );
}

#[test]
fn bundle_resources_do_not_include_dotagents_placeholders() {
    let config = read_tauri_config();
    let resources = config["bundle"]["resources"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let includes_dotagents = resources.iter().any(|value| {
        value
            .as_str()
            .map(|item| item.contains("dotagents"))
            .unwrap_or(false)
    });

    assert!(
        !includes_dotagents,
        "dotagents placeholder resources must not be bundled"
    );
}
