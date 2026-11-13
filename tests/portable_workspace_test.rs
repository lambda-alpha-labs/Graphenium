//! Portable workspace and read-only CI guarantees.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn gm() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin("gm"))
}

fn write_source(root: &Path) {
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("a.py"),
        b"class Foo:\n    def bar(self):\n        return 1\n",
    )
    .unwrap();
    std::fs::write(src.join("b.py"), b"def qux():\n    return 2\n").unwrap();
}

fn run_graph(root: &Path, out: &str, cache: &str) -> std::process::Output {
    gm().arg("run")
        .arg(root)
        .args(["--no-semantic", "--no-viz"])
        .args(["--out-dir", out, "--cache-dir", cache])
        .current_dir(root)
        .output()
        .expect("run gm")
}

#[test]
fn custom_out_and_cache_dirs_are_portable_and_not_indexed() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_source(root);

    let output = run_graph(root, "custom-out", "custom-cache");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(!root.join("graphenium-out").exists());
    assert!(root.join("custom-out/graph.json").exists());
    assert!(root.join("custom-out/manifest.json").exists());
    assert!(root.join("custom-cache/ast").exists());

    // Second run must not index files inside custom-out/custom-cache.
    let output = run_graph(root, "custom-out", "custom-cache");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let root_text = root.to_string_lossy().to_string();
    for artifact_dir in ["custom-out", "custom-cache"] {
        for entry in walk_files(&root.join(artifact_dir)) {
            let text = std::fs::read_to_string(&entry).unwrap_or_default();
            if entry.file_name().is_some_and(|name| name == "graph.json") {
                // metadata.project_root is intentionally absolute; every other
                // persisted path must be repo-relative.
                let graph: serde_json::Value = serde_json::from_str(&text).unwrap();
                let mut without_metadata = graph.clone();
                without_metadata.as_object_mut().unwrap().remove("metadata");
                assert!(
                    !without_metadata.to_string().contains(&root_text),
                    "absolute path leaked into {}",
                    entry.display()
                );
            } else {
                assert!(
                    !text.contains(&root_text),
                    "absolute path leaked into {}",
                    entry.display()
                );
            }
        }
    }

    let graph: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("custom-out/graph.json")).unwrap())
            .unwrap();
    let node_files: Vec<&str> = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["source_file"].as_str())
        .collect();
    assert!(node_files.iter().all(|path| path.starts_with("src/")));
    assert!(!node_files.iter().any(|path| path.contains("custom-out")));
}

fn walk_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_files(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[test]
fn verify_fresh_tolerates_touch_but_detects_content_drift() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_source(root);
    assert!(run_graph(root, "graphenium-out", "graphenium-out/cache")
        .status
        .success());

    let source = root.join("src/a.py");

    // Touch (mtime-only change) must not be reported as drift.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&source)
        .unwrap();
    file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
        .unwrap();
    drop(file);
    let output = gm()
        .args([
            "check",
            "--verify-fresh",
            "--graph",
            "graphenium-out/graph.json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "touch should not drift: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // One-byte content change must fail with the repo-relative path.
    std::fs::write(
        &source,
        b"class Foo:\n    def bar(self):\n        return 9\n",
    )
    .unwrap();
    let output = gm()
        .args([
            "check",
            "--verify-fresh",
            "--graph",
            "graphenium-out/graph.json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(!output.status.success(), "content drift must fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("src/a.py"),
        "drift report should name the repo-relative file: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn read_only_verification_commands_do_not_write() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_source(root);
    assert!(run_graph(root, "graphenium-out", "graphenium-out/cache")
        .status
        .success());

    let out = root.join("graphenium-out");
    let cache = out.join("cache");
    let restore = |dir: &Path, mode: u32| {
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode));
    };
    restore(&out, 0o555);
    restore(&cache, 0o555);

    let commands: Vec<Vec<&str>> = vec![
        vec!["check", "--graph", "graphenium-out/graph.json"],
        vec!["doctor", "--graph", "graphenium-out/graph.json"],
        vec!["query", "Foo", "--graph", "graphenium-out/graph.json"],
        vec!["explain", "Foo", "--graph", "graphenium-out/graph.json"],
        vec!["gate", "--out-dir", "graphenium-out"],
    ];

    for args in commands {
        let output = gm().args(&args).current_dir(root).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        assert!(
            !stderr.contains("permission denied") && !stderr.contains("read-only file system"),
            "{args:?} attempted a write: {stderr}"
        );
    }

    restore(&cache, 0o755);
    restore(&out, 0o755);
}
