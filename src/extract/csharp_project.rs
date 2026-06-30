//! C# solution and project configuration parsing.
//!
//! Parses `.sln` (solution) and `.csproj` (project) files to build
//! a workspace-level map of project dependencies.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct CSharpProject {
    pub name: String,
    pub assembly_name: String,
    pub root_namespace: String,
    pub project_references: Vec<PathBuf>,
    pub source_root: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct CSharpWorkspace {
    pub projects: HashMap<PathBuf, CSharpProject>,
}

impl CSharpWorkspace {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a Visual Studio solution file and return all discovered projects.
    pub fn parse_solution(sln_path: &Path) -> Self {
        let mut ws = Self::default();
        let content = match std::fs::read_to_string(sln_path) {
            Ok(c) => c,
            Err(_) => return ws,
        };
        let sln_dir = sln_path.parent().unwrap_or(Path::new("."));

        for line in content.lines() {
            let trimmed = line.trim();
            // Match: Project("{GUID}") = "ProjectName", "Path\To\Project.csproj", "{GUID}"
            if !trimmed.starts_with("Project(") {
                continue;
            }
            let parts: Vec<&str> = trimmed.split(',').collect();
            if parts.len() < 2 {
                continue;
            }
            let csproj_rel = parts[1].trim().trim_matches('"');
            let csproj_path = sln_dir.join(csproj_rel);
            let proj = Self::parse_csproj(&csproj_path);
            ws.projects.insert(csproj_path, proj);
        }
        ws
    }

    /// Parse a single .csproj file and return the project metadata.
    pub fn parse_csproj(csproj_path: &Path) -> CSharpProject {
        let content = match std::fs::read_to_string(csproj_path) {
            Ok(c) => c,
            Err(_) => {
                return CSharpProject {
                    name: csproj_path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    ..Default::default()
                }
            }
        };

        let name = csproj_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let source_root = csproj_path.parent().unwrap_or(Path::new(".")).to_path_buf();

        let mut assembly_name = name.clone();
        let mut root_namespace = name.clone();
        let mut project_references = Vec::new();
        let csproj_dir = csproj_path.parent().unwrap_or(Path::new("."));

        // Simple XML-ish parsing (not a full XML parser, just tag extraction)
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("<AssemblyName>") {
                let value = trimmed
                    .trim_start_matches("<AssemblyName>")
                    .trim_end_matches("</AssemblyName>");
                if !value.is_empty() {
                    assembly_name = value.to_string();
                }
            } else if trimmed.starts_with("<RootNamespace>") {
                let value = trimmed
                    .trim_start_matches("<RootNamespace>")
                    .trim_end_matches("</RootNamespace>");
                if !value.is_empty() {
                    root_namespace = value.to_string();
                }
            } else if trimmed.contains("<ProjectReference Include=") {
                // Extract the path from the Include attribute
                if let Some(start) = trimmed.find("Include=\"") {
                    let rest = &trimmed[start + 9..];
                    if let Some(end) = rest.find('"') {
                        let rel_path = &rest[..end];
                        // Normalize backslashes to forward slashes for Unix paths
                        let normalized = rel_path.replace('\\', "/");
                        let abs = csproj_dir.join(&normalized);
                        project_references.push(abs);
                    }
                }
            }
        }

        CSharpProject {
            name,
            assembly_name,
            root_namespace,
            project_references,
            source_root,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn parse_csproj_extracts_assembly_name() {
        let xml = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <AssemblyName>MyApp</AssemblyName>
    <RootNamespace>MyApp.Core</RootNamespace>
  </PropertyGroup>
</Project>"#;
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("MyApp.csproj");
        std::fs::write(&path, xml).unwrap();
        let proj = CSharpWorkspace::parse_csproj(&path);
        assert_eq!(proj.assembly_name, "MyApp");
        assert_eq!(proj.root_namespace, "MyApp.Core");
    }

    #[test]
    fn parse_csproj_detects_project_references() {
        let xml = r#"<Project Sdk="Microsoft.NET.Sdk">
  <ItemGroup>
    <ProjectReference Include="..\Core\Core.csproj" />
  </ItemGroup>
</Project>"#;
        let tmp = TempDir::new().unwrap();
        let core_dir = tmp.path().join("Core");
        std::fs::create_dir_all(&core_dir).unwrap();
        std::fs::write(core_dir.join("Core.csproj"), "<Project />").unwrap();
        let path = tmp.path().join("App.csproj");
        std::fs::write(&path, xml).unwrap();
        let proj = CSharpWorkspace::parse_csproj(&path);
        assert!(
            !proj.project_references.is_empty(),
            "Expected project references"
        );
        assert!(proj.project_references[0].ends_with("Core.csproj"));
    }

    #[test]
    fn missing_file_returns_default_name() {
        let proj = CSharpWorkspace::parse_csproj(Path::new("/nonexistent/project.csproj"));
        assert_eq!(proj.name, "project");
    }
}
