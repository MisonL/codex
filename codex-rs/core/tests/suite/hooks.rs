use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use core_test_support::fs_wait;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_toml_hooks_loaded_into_session() -> Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let marker_path = home.path().join("session_start.marker");
    let marker_path = marker_path.to_string_lossy();

    let config_toml = if cfg!(windows) {
        format!(
            "[hooks]\n\n[[hooks.session_start]]\ncommand = ['cmd', '/C', 'echo loaded>>\"{marker_path}\"']\n"
        )
    } else {
        format!(
            "[hooks]\n\n[[hooks.session_start]]\ncommand = ['sh', '-c', 'echo loaded >> \"{marker_path}\"']\n"
        )
    };
    std::fs::write(home.path().join("config.toml"), config_toml)?;

    let mut builder = test_codex().with_home(Arc::clone(&home));
    builder.build(&server).await?;

    fs_wait::wait_for_path_exists(
        home.path().join("session_start.marker"),
        Duration::from_secs(2),
    )
    .await?;
    let contents = std::fs::read_to_string(home.path().join("session_start.marker"))?;
    assert!(contents.contains("loaded"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn large_session_start_hook_context_spills_to_file() -> Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let hook_stdout_path = home.path().join("session_start_hook_stdout.json");
    let large_context = vec!["hook output"; 1_200].join(" ");
    std::fs::write(
        &hook_stdout_path,
        serde_json::json!({ "additionalContext": large_context }).to_string(),
    )?;
    write_session_start_cat_hook(home.path(), &hook_stdout_path)?;

    let mut builder = test_codex().with_home(Arc::clone(&home));
    let test = builder.build(&server).await?;
    wait_for_spill_file(test.codex_home_path()).await?;
    let mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    test.submit_turn("hello").await?;

    let developer_texts = mock.single_request().message_input_texts("developer");
    let spilled = developer_texts
        .iter()
        .find(|text| text.contains("Full hook output saved to: "))
        .context("spilled hook context developer message")?;
    assert!(spilled.contains("tokens truncated"));
    let spilled_path = spilled
        .lines()
        .find_map(|line| line.strip_prefix("Full hook output saved to: "))
        .context("spill path")?;
    let spilled_path = Path::new(spilled_path);
    assert!(spilled_path.starts_with(test.codex_home_path().join("hook_outputs")));
    assert_eq!(
        tokio::fs::read_to_string(spilled_path).await?,
        large_context
    );
    Ok(())
}

fn write_session_start_cat_hook(home: &Path, hook_stdout_path: &Path) -> Result<()> {
    let command = if cfg!(windows) {
        vec![
            "cmd".to_string(),
            "/C".to_string(),
            format!("type \"{}\"", hook_stdout_path.display()),
        ]
    } else {
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!("cat {}", shell_quote(hook_stdout_path)),
        ]
    };
    let command = command
        .iter()
        .map(|arg| format!("\"{}\"", toml_escape(arg)))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        home.join("config.toml"),
        format!("[hooks]\n\n[[hooks.session_start]]\ncommand = [{command}]\n"),
    )?;
    Ok(())
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

async fn wait_for_spill_file(codex_home: &Path) -> Result<()> {
    let root = codex_home.join("hook_outputs");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if root.exists() {
            for thread_dir in std::fs::read_dir(&root)? {
                let thread_dir = thread_dir?;
                if !thread_dir.file_type()?.is_dir() {
                    continue;
                }
                for spill_file in std::fs::read_dir(thread_dir.path())? {
                    let spill_file = spill_file?;
                    if spill_file.file_type()?.is_file() {
                        return Ok(());
                    }
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for hook output spill file");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
