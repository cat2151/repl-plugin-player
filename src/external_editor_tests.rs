use super::*;

fn directory(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("rpp-external-editor-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn saved_editor_is_read_back() {
    let directory = directory("save");
    let editor = directory.join("editor.exe");
    std::fs::write(&editor, "").unwrap();
    let status = directory.join("status.json");
    let saved = save(&status, &format!("  {}  ", editor.display()));
    let read_back = read(&status);
    let text = std::fs::read_to_string(&status).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(saved, Ok(Some(editor.clone())));
    assert_eq!(read_back, (Some(editor), None));
    assert!(text.contains("\"external_editor\""), "{text}");
}

#[test]
fn empty_input_removes_the_registration() {
    let directory = directory("clear");
    let status = directory.join("status.json");
    std::fs::write(&status, r#"{"external_editor": "C:\\editor.exe"}"#).unwrap();
    let saved = save(&status, "  ");
    let read_back = read(&status);
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(saved, Ok(None));
    assert_eq!(read_back, (None, None));
}

#[test]
fn relative_path_is_rejected_unsaved() {
    let directory = directory("relative");
    let status = directory.join("status.json");
    let saved = save(&status, "editor.exe");
    let exists = status.exists();
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(saved.is_err());
    assert!(!exists);
}

#[test]
fn missing_file_is_rejected_unsaved() {
    let directory = directory("missing");
    let status = directory.join("status.json");
    let saved = save(&status, &directory.join("none.exe").display().to_string());
    let directory_is_rejected = save(&status, &directory.display().to_string());
    let exists = status.exists();
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(saved.is_err());
    assert!(directory_is_rejected.is_err());
    assert!(!exists);
}

#[test]
fn broken_json_is_reported_as_unregistered() {
    let directory = directory("broken");
    let status = directory.join("status.json");
    let mut results = Vec::new();
    for text in ["{", "[]", r#"{"external_editor": 3}"#] {
        std::fs::write(&status, text).unwrap();
        results.push(read(&status));
    }
    std::fs::remove_dir_all(&directory).unwrap();
    for (editor, error) in results {
        assert_eq!(editor, None);
        assert!(error.is_some());
    }
}

#[test]
fn missing_status_is_unregistered_without_error() {
    let directory = directory("absent");
    let result = read(&directory.join("status.json"));
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(result, (None, None));
}

#[test]
fn unregistered_editor_does_not_create_config() {
    let directory = directory("unregistered");
    let config = directory.join("config.toml");
    let result = open_config(None, &config);
    let exists = config.exists();
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(result.is_err());
    assert!(!exists);
}

#[test]
fn launch_failure_is_reported() {
    let directory = directory("launch");
    let config = directory.join("config.toml");
    let result = open_config(Some(&directory.join("none.exe")), &config);
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(result.is_err());
}

/// A dummy editor (`.cmd`) writes its argument to a file: the editor must
/// receive the full path of `config.toml`, created from the template when
/// missing and left untouched when present.
#[cfg(windows)]
#[test]
fn editor_receives_the_config_path() {
    let directory = directory("dummy");
    let editor = directory.join("editor.cmd");
    let received = directory.join("received.txt");
    std::fs::write(
        &editor,
        format!("@echo %~1> \"{}\"\r\n", received.display()),
    )
    .unwrap();
    let config = directory.join("user").join("config.toml");
    let wait = |config: &Path| {
        let _ = std::fs::remove_file(&received);
        open_config(Some(&editor), config).unwrap();
        for _ in 0..100 {
            if let Ok(text) = std::fs::read_to_string(&received) {
                if text.ends_with('\n') {
                    return text.trim().to_string();
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("the dummy editor did not run");
    };
    let first = wait(&config);
    let template = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, "[window]\nwidth = 900\n").unwrap();
    let second = wait(&config);
    let kept = std::fs::read_to_string(&config).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(first, config.display().to_string());
    assert_eq!(second, first);
    assert_eq!(template, crate::config::TEMPLATE);
    assert_eq!(kept, "[window]\nwidth = 900\n");
}
