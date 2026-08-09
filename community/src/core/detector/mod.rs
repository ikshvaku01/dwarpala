use crate::config::language_config_path;
use std::{fs, path::Path};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    Node, SpringBoot, Maven, DotNet, Rails, Python, Cpp, Unknown,
}

impl ProjectType {
    pub fn detect(root: &Path) -> Self {
        if root.join("package.json").exists() { return Self::Node; }
        if root.join("pom.xml").exists() {
            let is_spring = fs::read_to_string(root.join("pom.xml"))
                .map(|s| s.contains("spring-boot")).unwrap_or(false);
            return if is_spring { Self::SpringBoot } else { Self::Maven };
        }
        if WalkDir::new(root).max_depth(3).into_iter()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().ends_with(".csproj"))
        { return Self::DotNet; }
        if root.join("Gemfile").exists()          { return Self::Rails;  }
        if root.join("requirements.txt").exists() { return Self::Python; }
        if root.join("CMakeLists.txt").exists()   { return Self::Cpp;    }
        Self::Unknown
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Node       => "node",
            Self::SpringBoot => "springboot",
            Self::Maven      => "maven",
            Self::DotNet     => "dotnet",
            Self::Rails      => "rails",
            Self::Python     => "python",
            Self::Cpp        => "cpp",
            Self::Unknown    => "unknown",
        }
    }

    pub fn language_ignores(&self) -> Vec<String> {
        let path = language_config_path(self.as_str());
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(arr) = json.get("ignore").and_then(|v| v.as_array()) {
                    return arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                }
            }
        }
        match self {
            Self::Node               => vec!["node_modules/".into(), "dist/".into()],
            Self::SpringBoot | Self::Maven => vec!["target/".into()],
            Self::DotNet             => vec!["bin/".into(), "obj/".into()],
            Self::Rails              => vec!["log/".into(), "tmp/".into()],
            Self::Python             => vec!["__pycache__/".into(), "*.pyc".into()],
            Self::Cpp                => vec!["build/".into()],
            Self::Unknown            => vec![],
        }
    }
}
