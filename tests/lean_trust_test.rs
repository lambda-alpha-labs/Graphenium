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

fn setup_hypotheses(root: &Path) -> std::path::PathBuf {
    let lean_dir = root.join("NoFourMUBs").join("EdgeXFinal");
    std::fs::create_dir_all(&lean_dir).unwrap();
    std::fs::write(
        lean_dir.join("Proof.lean"),
        br#"
namespace NoFourMUBs.EdgeXFinal

structure EdgeAtlasCertificate where
  witness : Nat

theorem no_four_mubs_seed (h : EdgeAtlasCertificate) : 1 = 1 := by
  exact rfl

theorem downstream : 1 = 1 := by
  exact no_four_mubs_seed { witness := 0 }

end NoFourMUBs.EdgeXFinal
"#,
    )
    .unwrap();

    let report = root.join("axioms-hypotheses.json");
    std::fs::write(
        &report,
        br#"{
  "schema_version": "lean-axioms.v1",
  "source_hash": "feedface",
  "declarations": [
    { "name": "NoFourMUBs.EdgeXFinal.no_four_mubs_seed", "axioms": ["propext"] },
    { "name": "NoFourMUBs.EdgeXFinal.downstream", "axioms": ["propext"] }
  ]
}"#,
    )
    .unwrap();
    report
}

/// Mirrors the real NoFourMUBs layout: the certificate lives in another file,
/// the theorem uses a section `variable`, and a Mathlib-style generic theorem
/// uses `[CommRing R]` / `(n : Nat)` parameters.
fn setup_cross_file_hypotheses(root: &Path) -> std::path::PathBuf {
    let seed_dir = root.join("NoFourMUBs").join("SeedAtlas");
    std::fs::create_dir_all(&seed_dir).unwrap();
    std::fs::write(
        seed_dir.join("EdgeXAtlas.lean"),
        br#"
namespace NoFourMUBs.SeedAtlas.EdgeX

structure EdgeAtlasCertificate where
  witness : Nat

end NoFourMUBs.SeedAtlas.EdgeX
"#,
    )
    .unwrap();
    std::fs::write(
        seed_dir.join("EdgeXFinal.lean"),
        br#"
import NoFourMUBs.SeedAtlas.EdgeXAtlas

namespace NoFourMUBs

open SeedAtlas EdgeX

section

variable (C : EdgeAtlasCertificate)

theorem EdgeXFinal.no_four_mubs_seed :
    1 = 1 := by
  exact rfl

theorem multiline_seed
    (h :
      EdgeAtlasCertificate) :
    1 = 1 := by
  exact rfl

end

theorem generic {R : Type} [CommRing R] (n : Nat) : n = n := rfl

end NoFourMUBs
"#,
    )
    .unwrap();

    let report = root.join("axioms-cross-file.json");
    std::fs::write(
        &report,
        br#"{
  "schema_version": "lean-axioms.v1",
  "source_hash": "cafebabe",
  "declarations": [
    { "name": "NoFourMUBs.EdgeXFinal.no_four_mubs_seed", "axioms": ["propext"] },
    { "name": "NoFourMUBs.multiline_seed", "axioms": ["propext"] },
    { "name": "NoFourMUBs.generic", "axioms": ["propext", "Classical.choice", "Quot.sound"] }
  ]
}"#,
    )
    .unwrap();
    report
}

fn run_with_hypotheses(root: &Path) {
    let output = gm()
        .arg("run")
        .arg(root)
        .args([
            "--no-semantic",
            "--no-viz",
            "--lean-axioms",
            "axioms-hypotheses.json",
        ])
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
fn conditional_hypotheses_are_detected_allowlisted_and_transitive() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_hypotheses(root);
    run_with_hypotheses(root);

    // Direct hypothesis: kernel-clean (`propext`) but CONDITIONAL.
    let output = gm()
        .args([
            "trust",
            "--graph",
            "graphenium-out/graph.json",
            "--theorem",
            "NoFourMUBs.EdgeXFinal.no_four_mubs_seed",
            "--json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "conditional theorem must fail without --allow-hypothesis"
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let theorem = &report["theorems"][0];
    assert_eq!(theorem["status"], "CONDITIONAL");
    assert_eq!(theorem["passed"], false);
    let assumptions = theorem["assumptions"].as_array().unwrap();
    assert_eq!(assumptions.len(), 1, "{assumptions:?}");
    assert_eq!(assumptions[0]["name"], "EdgeAtlasCertificate");
    assert_eq!(assumptions[0]["kind"], "hypothesis");
    assert_eq!(assumptions[0]["direct_or_transitive"], "direct");
    assert!(theorem["kernel_axioms"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a == "propext"));

    // Explicit allowance makes the gate pass while preserving the status.
    let output = gm()
        .args([
            "trust",
            "--graph",
            "graphenium-out/graph.json",
            "--theorem",
            "NoFourMUBs.EdgeXFinal.no_four_mubs_seed",
            "--allow-hypothesis",
            "EdgeAtlasCertificate",
            "--json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "allowlisted hypothesis should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["theorems"][0]["status"], "CONDITIONAL");
    assert_eq!(report["theorems"][0]["passed"], true);

    // A downstream theorem inherits the hypothesis transitively.
    let output = gm()
        .args([
            "trust",
            "--graph",
            "graphenium-out/graph.json",
            "--theorem",
            "NoFourMUBs.EdgeXFinal.downstream",
            "--json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let theorem = &report["theorems"][0];
    assert_eq!(theorem["status"], "CONDITIONAL");
    let assumptions = theorem["assumptions"].as_array().unwrap();
    assert!(
        assumptions
            .iter()
            .any(|a| a["name"] == "EdgeAtlasCertificate"
                && a["direct_or_transitive"] == "transitive"
                && a["introduced_by"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("no_four_mubs_seed")),
        "{assumptions:?}"
    );
}

#[test]
fn cross_file_and_multiline_hypotheses_are_detected_without_flags() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_cross_file_hypotheses(root);
    let output = gm()
        .arg("run")
        .arg(root)
        .args([
            "--no-semantic",
            "--no-viz",
            "--lean-axioms",
            "axioms-cross-file.json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // No --hypothesis, no --allow-hypothesis: the certificate binder must be
    // discovered from the multi-line/section-variable signature and resolved
    // across files. JSON output stays valid and stderr stays clean.
    let output = gm()
        .args([
            "trust",
            "--graph",
            "graphenium-out/graph.json",
            "--theorem",
            "NoFourMUBs.EdgeXFinal.no_four_mubs_seed",
            "--warn-structure-params",
            "--forbid-native-decide",
            "--json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(!output.status.success(), "conditional theorem must fail");
    assert!(
        output.stderr.is_empty(),
        "--json failures must not emit stderr noise: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let theorem = &report["theorems"][0];
    assert_eq!(theorem["status"], "CONDITIONAL");
    assert_eq!(theorem["passed"], false);
    assert!(
        theorem["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|assumption| assumption["name"] == "EdgeAtlasCertificate"),
        "{theorem}"
    );
    assert!(!report["failure_reasons"].as_array().unwrap().is_empty());

    // Multi-line explicit binder is also detected.
    let output = gm()
        .args([
            "trust",
            "--graph",
            "graphenium-out/graph.json",
            "--theorem",
            "NoFourMUBs.multiline_seed",
            "--json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["theorems"][0]["status"], "CONDITIONAL");

    // Guardrail: typeclass constraints and primitive data remain UNCONDITIONAL.
    let output = gm()
        .args([
            "trust",
            "--graph",
            "graphenium-out/graph.json",
            "--theorem",
            "NoFourMUBs.generic",
            "--json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generic theorem should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let theorem = &report["theorems"][0];
    assert_eq!(theorem["status"], "UNCONDITIONAL");
    assert_eq!(theorem["passed"], true);
    assert_eq!(theorem["assumptions"], serde_json::json!([]));
}

#[test]
fn policy_file_can_allow_hypotheses() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_hypotheses(root);
    run_with_hypotheses(root);

    std::fs::create_dir_all(root.join(".graphenium")).unwrap();
    std::fs::write(
        root.join(".graphenium/policy.json"),
        br#"{
  "rules": [],
  "trust": { "allow_hypotheses": ["EdgeAtlasCertificate"] }
}"#,
    )
    .unwrap();

    let output = gm()
        .args([
            "trust",
            "--graph",
            "graphenium-out/graph.json",
            "--theorem",
            "NoFourMUBs.EdgeXFinal.no_four_mubs_seed",
            "--json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "policy allow_hypotheses should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
