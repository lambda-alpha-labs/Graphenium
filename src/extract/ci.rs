//! CI extraction: parses CI configuration files and adds test-to-source mappings
//! to the graph. Supports GitHub Actions, Makefile, and common config patterns.
//!
//! Injects nodes for CI jobs, test targets, and build artifacts so the graph
//! reflects the full verification surface of the repository.

use crate::model::{Edge, ExtractionResult, FileType, Node};

/// Extraction modes for CI artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiFormat {
    GithubActions,
    Makefile,
    CargoToml,
    PackageJson,
    Unknown,
}

/// A CI job or test target extracted from configuration.
#[derive(Debug, Clone)]
pub struct CiTarget {
    pub name: String,
    pub kind: CiFormat,
    pub commands: Vec<String>,
    pub dependencies: Vec<String>,
    /// Source files this job tests (e.g., `src/foo.rs` if `cargo test`).
    pub tested_files: Vec<String>,
    /// Source files this job builds (e.g., `src/*.rs` if `cargo build`).
    pub built_files: Vec<String>,
}

/// Detect the CI format from a file path.
pub fn detect_format(path: &str) -> CiFormat {
    if path.contains(".github/workflows/") || path.ends_with(".github/workflows/ci.yml") {
        CiFormat::GithubActions
    } else if path.ends_with("Makefile") || path.ends_with("makefile") {
        CiFormat::Makefile
    } else if path.ends_with("Cargo.toml") && path == "Cargo.toml" {
        CiFormat::CargoToml
    } else if path.ends_with("package.json") {
        CiFormat::PackageJson
    } else {
        CiFormat::Unknown
    }
}

/// Parse a CI config file (or relevant project file) into CI targets.
/// For Cargo.toml: extracts test targets from [[test]] sections and package name.
/// For package.json: extracts test/build scripts.
/// For GitHub Actions: extracts job names and steps.
/// For Makefile: extracts test/build targets.
pub fn parse_ci_config(path: &str, content: &str) -> Vec<CiTarget> {
    let fmt = detect_format(path);
    match fmt {
        CiFormat::CargoToml => parse_cargo_toml(content),
        CiFormat::PackageJson => parse_package_json(content),
        CiFormat::GithubActions => parse_github_actions(content),
        CiFormat::Makefile => parse_makefile(content),
        CiFormat::Unknown => Vec::new(),
    }
}

/// Convert CI targets into extraction results for graph injection.
pub fn ci_targets_to_extraction(targets: &[CiTarget], source_file: &str) -> ExtractionResult {
    let mut result = ExtractionResult::new();

    for target in targets {
        let node_id = format!(
            "ci_{}",
            target.name.replace(|c: char| !c.is_alphanumeric(), "_")
        );
        result.nodes.push(Node::new(
            &node_id,
            &target.name,
            FileType::Document, // CI config = document-level artifact
            source_file,
        ));

        // Link build/test targets to CI jobs via runs_in
        for built in &target.built_files {
            let built_id = format!("build_{}", built.replace('/', "_").replace('.', "_"));
            result
                .nodes
                .push(Node::new(&built_id, built, FileType::Code, source_file));
            result
                .edges
                .push(Edge::extracted(&built_id, &node_id, "runs_in", source_file));
        }
        for tested in &target.tested_files {
            let tested_id = format!("tested_{}", tested.replace('/', "_").replace('.', "_"));
            result
                .nodes
                .push(Node::new(&tested_id, tested, FileType::Code, source_file));
            result.edges.push(Edge::extracted(
                &tested_id,
                &node_id,
                "runs_in",
                source_file,
            ));
        }

        // Link to tested files as "tests" edges
        for tested in &target.tested_files {
            let tested_id = format!("tested_{}", tested.replace('/', "_").replace('.', "_"));
            result
                .nodes
                .push(Node::new(&tested_id, tested, FileType::Code, source_file));
            result
                .edges
                .push(Edge::extracted(&node_id, &tested_id, "tests", source_file));
        }

        // Link dependencies as "depends_on" edges
        for dep in &target.dependencies {
            let dep_id = format!("dep_{}", dep.replace(|c: char| !c.is_alphanumeric(), "_"));
            result
                .nodes
                .push(Node::new(&dep_id, dep, FileType::Document, source_file));
            result.edges.push(Edge::extracted(
                &node_id,
                &dep_id,
                "depends_on",
                source_file,
            ));
        }
    }

    result
}

fn parse_cargo_toml(content: &str) -> Vec<CiTarget> {
    let mut targets = Vec::new();
    let mut package_name = "project".to_string();
    let mut has_test_targets = false;
    let mut test_names = Vec::new();
    let mut in_test_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[package]") {
            in_test_section = false;
        } else if trimmed.starts_with("name") && trimmed.contains("=") {
            if let Some(name_part) = trimmed.split('=').nth(1) {
                let name = name_part.trim().trim_matches('"').to_string();
                if !name.is_empty() {
                    package_name = name;
                }
            }
        } else if trimmed.starts_with("[[test]]") {
            in_test_section = true;
            has_test_targets = true;
        } else if trimmed.starts_with('[') {
            in_test_section = false;
        } else if in_test_section && trimmed.starts_with("name") {
            if let Some(n) = trimmed.split('=').nth(1) {
                test_names.push(n.trim().trim_matches('"').to_string());
            }
        }
    }

    // Build target
    targets.push(CiTarget {
        name: format!("cargo_build_{}", package_name),
        kind: CiFormat::CargoToml,
        commands: vec!["cargo build".to_string()],
        dependencies: vec![],
        tested_files: vec![],
        built_files: vec!["src/**/*.rs".to_string()],
    });

    // Test target(s)
    if has_test_targets && !test_names.is_empty() {
        for t in &test_names {
            targets.push(CiTarget {
                name: format!("test_{}", t),
                kind: CiFormat::CargoToml,
                commands: vec![format!("cargo test {}", t)],
                dependencies: vec!["cargo_build".to_string()],
                tested_files: vec!["src/**/*.rs".to_string()],
                built_files: vec![],
            });
        }
    } else {
        targets.push(CiTarget {
            name: format!("test_{}", package_name),
            kind: CiFormat::CargoToml,
            commands: vec!["cargo test".to_string()],
            dependencies: vec!["cargo_build".to_string()],
            tested_files: vec!["src/**/*.rs".to_string()],
            built_files: vec![],
        });
    }

    targets
}

fn parse_package_json(content: &str) -> Vec<CiTarget> {
    let mut targets = Vec::new();
    let mut package_name = "project".to_string();

    // Try to extract package name
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"name\"") {
            if let Some(n) = trimmed.split(':').nth(1) {
                package_name = n.trim().trim_matches(',').trim_matches('"').to_string();
            }
            break;
        }
    }

    targets.push(CiTarget {
        name: format!("build_{}", package_name),
        kind: CiFormat::PackageJson,
        commands: vec!["npm run build".to_string()],
        dependencies: vec![],
        tested_files: vec![],
        built_files: vec!["src/**".to_string()],
    });

    targets.push(CiTarget {
        name: format!("test_{}", package_name),
        kind: CiFormat::PackageJson,
        commands: vec!["npm test".to_string()],
        dependencies: vec!["build".to_string()],
        tested_files: vec!["src/**/*.{js,ts,jsx,tsx}".to_string()],
        built_files: vec![],
    });

    targets
}

fn parse_github_actions(content: &str) -> Vec<CiTarget> {
    let mut targets = Vec::new();
    let mut current_job = String::new();
    let mut current_commands = Vec::new();
    let mut in_job = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("  ") && !trimmed.starts_with("    ") && trimmed.contains(':') {
            // Could be a job name
            let name = trimmed.split(':').next().unwrap_or("").trim().to_string();
            if !name.is_empty()
                && !name.starts_with("runs-on")
                && !name.starts_with("steps")
                && !name.starts_with("with")
                && !name.starts_with("env")
                && !name.starts_with("on")
                && !name.starts_with("name")
            {
                if in_job && !current_job.is_empty() {
                    targets.push(CiTarget {
                        name: current_job.clone(),
                        kind: CiFormat::GithubActions,
                        commands: current_commands.clone(),
                        dependencies: vec![],
                        tested_files: vec![],
                        built_files: vec![],
                    });
                    current_commands.clear();
                }
                current_job = name;
                in_job = true;
            }
        } else if in_job && trimmed.starts_with("- run:") {
            if let Some(cmd) = trimmed.split(':').nth(1) {
                current_commands.push(cmd.trim().to_string());
            }
        }
    }

    // Push the last job
    if in_job && !current_job.is_empty() {
        targets.push(CiTarget {
            name: current_job,
            kind: CiFormat::GithubActions,
            commands: current_commands,
            dependencies: vec![],
            tested_files: vec![],
            built_files: vec![],
        });
    }

    targets
}

fn parse_makefile(content: &str) -> Vec<CiTarget> {
    let mut targets = Vec::new();
    let mut commands = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with(':') && !trimmed.starts_with('.') && !trimmed.starts_with('#') {
            let name = trimmed.trim_end_matches(':').to_string();
            targets.push(CiTarget {
                name,
                kind: CiFormat::Makefile,
                commands: commands.clone(),
                dependencies: vec![],
                tested_files: vec!["src/**".to_string()],
                built_files: vec![],
            });
            commands.clear();
        } else if trimmed.starts_with("\t") && !trimmed.starts_with("\t#") {
            commands.push(trimmed.to_string());
        }
    }

    targets
}

// ── C# Project integration ───────────────────────────────────────────────────

/// Inject C# project boundary nodes and dependency edges from a parsed workspace.
pub fn csproj_to_extraction(
    workspace: &crate::extract::csharp_project::CSharpWorkspace,
) -> ExtractionResult {
    let mut result = ExtractionResult::new();

    for (_path, project) in &workspace.projects {
        let proj_id = format!("csproj_{}", project.name);
        let node = Node {
            id: proj_id.clone(),
            label: project.name.clone(),
            file_type: FileType::Code,
            source_file: project.source_root.to_string_lossy().to_string(),
            source_location: String::new(),
            qualified_label: Some(project.root_namespace.clone()),
            plan_id: None,
            community: None,
            extractor: Some("csproj-parser".to_string()),
            resolution_status: Some("resolved".to_string()),
        };
        result.nodes.push(node);

        for reference in &project.project_references {
            if let Some(ref_name) = reference.file_stem() {
                let ref_id = format!("csproj_{}", ref_name.to_string_lossy());
                let edge = Edge {
                    source: proj_id.clone(),
                    target: ref_id,
                    relation: "depends_on".to_string(),
                    confidence: crate::model::Confidence::Extracted,
                    confidence_score: 1.0,
                    source_file: project.source_root.to_string_lossy().to_string(),
                    source_location: None,
                    extractor: Some("csproj-parser".to_string()),
                    resolution_status: Some("resolved".to_string()),
                    weight: 1.0,
                    src_original: String::new(),
                    tgt_original: String::new(),
                    plan_id: None,
                };
                result.edges.push(edge);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cargo_toml() {
        assert_eq!(detect_format("Cargo.toml"), CiFormat::CargoToml);
    }

    #[test]
    fn detects_github_actions() {
        assert_eq!(
            detect_format(".github/workflows/ci.yml"),
            CiFormat::GithubActions
        );
    }

    #[test]
    fn parse_cargo_toml_build_and_test() {
        let content = r#"[package]
name = "myapp"

[[test]]
name = "integration"
"#;
        let targets = parse_cargo_toml(content);
        assert!(targets.len() >= 2);
        assert!(targets.iter().any(|t| t.commands[0] == "cargo build"));
        assert!(targets.iter().any(|t| t.name.contains("test")));
    }

    #[test]
    fn parse_package_json_creates_build_and_test() {
        let content = r#"{
  "name": "myapp",
  "scripts": {
    "test": "jest"
  }
}"#;
        let targets = parse_package_json(content);
        assert_eq!(targets.len(), 2);
        assert!(targets[0].commands[0].contains("build"));
        assert!(targets[1].commands[0].contains("test"));
    }

    #[test]
    fn ci_targets_to_extraction_creates_nodes() {
        let targets = vec![CiTarget {
            name: "test_core".to_string(),
            kind: CiFormat::CargoToml,
            commands: vec!["cargo test".to_string()],
            dependencies: vec!["build".to_string()],
            tested_files: vec!["src/core.rs".to_string()],
            built_files: vec![],
        }];
        let result = ci_targets_to_extraction(&targets, "Cargo.toml");
        assert!(result.nodes.len() >= 3); // ci node + tested + dep node
        assert!(result.edges.iter().any(|e| e.relation == "tests"));
        assert!(result.edges.iter().any(|e| e.relation == "depends_on"));
    }
}
