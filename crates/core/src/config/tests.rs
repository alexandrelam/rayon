use super::load_config;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[allow(clippy::unwrap_used)]
fn unique_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rayon-config-test-{suffix}"))
}

#[allow(clippy::unwrap_used)]
#[test]
fn loads_toml_providers_from_xdg_path() {
    let _env_guard = env_lock().lock().unwrap();
    let config_home = unique_dir();
    let rayon_dir = config_home.join("rayon");
    fs::create_dir_all(&rayon_dir).unwrap();
    fs::write(
        rayon_dir.join("commands.toml"),
        r#"
plugin_id = "user.commands"

[[commands]]
id = "user.echo"
title = "Echo"
program = "/bin/echo"
base_args = ["hello"]
"#,
    )
    .unwrap();

    let previous = env::var_os("XDG_CONFIG_HOME");
    env::set_var("XDG_CONFIG_HOME", &config_home);
    let loaded = load_config().unwrap();
    if let Some(previous) = previous {
        env::set_var("XDG_CONFIG_HOME", previous);
    } else {
        env::remove_var("XDG_CONFIG_HOME");
    }

    assert_eq!(loaded.command_providers.len(), 1);
    let commands = loaded.command_providers[0].commands();
    assert_eq!(commands[0].owner_plugin_id, "user.commands");
    assert!(!commands[0].close_launcher_on_success);
}

#[allow(clippy::unwrap_used)]
#[test]
fn loads_bookmarks_from_manifest() {
    let _env_guard = env_lock().lock().unwrap();
    let config_home = unique_dir();
    let rayon_dir = config_home.join("rayon");
    fs::create_dir_all(&rayon_dir).unwrap();
    fs::write(
        rayon_dir.join("bookmarks.toml"),
        r#"
plugin_id = "user.links"

[[bookmarks]]
id = "user.github"
title = "GitHub"
url = "https://github.com"
subtitle = "Code hosting"
keywords = ["git", "repos"]
"#,
    )
    .unwrap();

    let previous = env::var_os("XDG_CONFIG_HOME");
    env::set_var("XDG_CONFIG_HOME", &config_home);
    let loaded = load_config().unwrap();
    if let Some(previous) = previous {
        env::set_var("XDG_CONFIG_HOME", previous);
    } else {
        env::remove_var("XDG_CONFIG_HOME");
    }

    assert!(loaded.command_providers.is_empty());
    assert_eq!(loaded.bookmarks.len(), 1);
    assert_eq!(loaded.bookmarks[0].owner_plugin_id, "user.links");
    assert_eq!(loaded.bookmarks[0].url, "https://github.com");
}

#[allow(clippy::unwrap_used)]
#[test]
fn rejects_invalid_bookmark_urls() {
    let _env_guard = env_lock().lock().unwrap();
    let config_home = unique_dir();
    let rayon_dir = config_home.join("rayon");
    fs::create_dir_all(&rayon_dir).unwrap();
    fs::write(
        rayon_dir.join("invalid.toml"),
        r#"
plugin_id = "user.links"

[[bookmarks]]
id = "user.bad"
title = "Bad"
url = "github.com"
"#,
    )
    .unwrap();

    let previous = env::var_os("XDG_CONFIG_HOME");
    env::set_var("XDG_CONFIG_HOME", &config_home);
    let result = load_config();
    if let Some(previous) = previous {
        env::set_var("XDG_CONFIG_HOME", previous);
    } else {
        env::remove_var("XDG_CONFIG_HOME");
    }

    assert!(matches!(result, Err(ref error) if error.contains("invalid bookmark url")));
}

#[allow(clippy::unwrap_used)]
#[test]
fn loads_legacy_structured_command_fields_but_ignores_them() {
    let _env_guard = env_lock().lock().unwrap();
    let config_home = unique_dir();
    let rayon_dir = config_home.join("rayon");
    fs::create_dir_all(&rayon_dir).unwrap();
    fs::write(
        rayon_dir.join("invalid-command.toml"),
        r#"
plugin_id = "user.commands"

[[commands]]
id = "user.echo"
title = "Echo"
input_mode = "structured"
program = "/bin/echo"

[[commands.arguments]]
id = "message"
label = "Message"
type = "string"
required = true
positional = 0
"#,
    )
    .unwrap();

    let previous = env::var_os("XDG_CONFIG_HOME");
    env::set_var("XDG_CONFIG_HOME", &config_home);
    let loaded = load_config().unwrap();
    if let Some(previous) = previous {
        env::set_var("XDG_CONFIG_HOME", previous);
    } else {
        env::remove_var("XDG_CONFIG_HOME");
    }

    let commands = loaded.command_providers[0].commands();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].id.to_string(), "user.echo");
    assert!(commands[0].arguments.is_empty());
}

#[allow(clippy::unwrap_used)]
#[test]
fn loads_close_launcher_on_success_for_custom_commands() {
    let _env_guard = env_lock().lock().unwrap();
    let config_home = unique_dir();
    let rayon_dir = config_home.join("rayon");
    fs::create_dir_all(&rayon_dir).unwrap();
    fs::write(
        rayon_dir.join("commands.toml"),
        r#"
plugin_id = "user.commands"

[[commands]]
id = "user.echo"
title = "Echo"
program = "/bin/echo"
close_launcher_on_success = true
"#,
    )
    .unwrap();

    let previous = env::var_os("XDG_CONFIG_HOME");
    env::set_var("XDG_CONFIG_HOME", &config_home);
    let loaded = load_config().unwrap();
    if let Some(previous) = previous {
        env::set_var("XDG_CONFIG_HOME", previous);
    } else {
        env::remove_var("XDG_CONFIG_HOME");
    }

    let commands = loaded.command_providers[0].commands();
    assert_eq!(commands.len(), 1);
    assert!(commands[0].close_launcher_on_success);
}
