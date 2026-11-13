//! End-to-end Lean 4 trust-boundary auditing.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn gm() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin("gm"))
}

fn setup(root: &Path) -> std::path::PathBuf {
    let lean_dir = root.join("MUB6");
    std::fs::create_dir_all(&lean_dir).unwrap();
    std::fs::write(
        lean_dir.join("Proof.lean"),
        br#"
namespace MUB6
theorem no_four_mubs_dim6 : 1 = 1 := by
  exact rfl
theorem clean : 1 = 1 := by exact rfl
end MUB6
"#,
    )
    .unwrap();

    let report = root.join("axioms.json");
    std::fs::write(
        &report,
        br#"{
  "schema_version": "lean-axioms.v1",
  "source_hash": "deadbeef",
  "declarations": [
    { "name": "MUB6.no_four_mubs_dim6", "axioms": ["propext", "Quot.sound", "sorryAx", "Lean.ofReduceBool", "EdgeAtlasCertificate"] },
    { "name": "MUB6.clean", "axioms": ["propext", "Quot.sound"] }
  ]
}"#,
    )
    .unwrap();
    report
}

fn run_with_axioms(root: &Path) {
    let output = gm()
        .arg("run")
        .arg(root)
        .args(["--no-semantic", "--no-viz", "--lean-axioms", "axioms.json"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn baked_axioms_are_auditable_by_gm_trust() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup(root);
    run_with_axioms(root);

    // `--forbid-native-decide` must reject the tainted theorem.
    let output = gm()
        .args([
            "trust",
            "--graph",
            "graphenium-out/graph.json",
            "--theorem",
            "MUB6.no_four_mubs_dim6",
            "--forbid-native-decide",
            "--json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(!output.status.success(), "tainted theorem must fail");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], "lean-trust.v1");
    assert_eq!(report["passed"], false);
    let axioms = report["theorems"][0]["axioms"].to_string();
    assert!(axioms.contains("EdgeAtlasCertificate"), "{axioms}");
    assert!(axioms.contains("Lean.ofReduceBool"), "{axioms}");
    assert!(axioms.contains("sorryAx"), "{axioms}");
    assert_eq!(report["theorems"][0]["has_native_decide"], true);
    assert_eq!(report["theorems"][0]["has_sorry"], true);

    // A kernel-only theorem passes even with native_decide forbidden.
    let output = gm()
        .args([
            "trust",
            "--graph",
            "graphenium-out/graph.json",
            "--theorem",
            "MUB6.clean",
            "--forbid-native-decide",
            "--json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "clean theorem should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["passed"], true);
}

#[test]
fn allowlisted_external_axiom_passes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup(root);
    run_with_axioms(root);

    // Allow the external axiom and omit --forbid-native-decide; only sorryAx
    // remains disallowed.
    let output = gm()
        .args([
            "trust",
            "--graph",
            "graphenium-out/graph.json",
            "--theorem",
            "MUB6.no_four_mubs_dim6",
            "--allow-axiom",
            "EdgeAtlasCertificate",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "sorryAx must still fail despite allowlisting the external axiom"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("sorryAx"),
        "report should surface sorryAx"
    );
}
