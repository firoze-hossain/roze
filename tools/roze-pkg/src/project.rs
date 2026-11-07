// tools/roze-pkg/src/project.rs
//
// Proprietary 'bsr.mfz' (Mohammad Firoze) Manifest Engine.
// Full parser, validator, serializer, and feature/target resolution engine
// with 100% backwards compatibility for legacy flat manifests (roze.mfz, roze.toml).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::semver::{Version, VersionReq};

pub const MANIFEST_NAME: &str = "bsr.mfz";
pub const ALTERNATIVE_MANIFEST_NAME: &str = "roze.mfz";
pub const LEGACY_MANIFEST_NAME: &str = "roze.toml";
pub const MANIFEST_HEADER: &str = "# bsr.mfz - Basir Project Manifest (Mohammad Firoze)\n\n";

pub const RESERVED_KEYWORDS: &[&str] = &[
    "as", "assert", "async", "await", "break", "case", "catch", "class", "const",
    "continue", "crate", "default", "defer", "dyn", "else", "enum", "export",
    "extern", "false", "final", "fn", "for", "from", "func", "if", "impl", "import",
    "in", "interface", "let", "loop", "match", "mod", "mut", "null", "package",
    "pub", "return", "self", "static", "struct", "super", "switch", "trait",
    "true", "try", "type", "typeof", "use", "var", "while", "yield",
];

pub fn validate_package_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("Package name cannot be empty"));
    }

    let parts: Vec<&str> = name.split("::").collect();
    for part in parts {
        if part.is_empty() {
            return Err(anyhow!(
                "Invalid package name '{}': contains empty namespace segment",
                name
            ));
        }
        let mut chars = part.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_alphabetic() && first != '_' {
            return Err(anyhow!(
                "Invalid package name '{}': must start with an ASCII letter or underscore",
                name
            ));
        }

        for c in chars {
            if !c.is_ascii_alphanumeric() && c != '_' && c != '-' {
                return Err(anyhow!(
                    "Invalid character '{}' in package name '{}': names may only contain ASCII alphanumerics, hyphens, and underscores",
                    c,
                    name
                ));
            }
        }

        if RESERVED_KEYWORDS.contains(&part.to_lowercase().as_str()) {
            return Err(anyhow!(
                "The name '{}' cannot be used as a package name, it is a Roze keyword",
                part
            ));
        }
    }

    Ok(())
}

pub fn validate_feature_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("Feature name cannot be empty"));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(anyhow!(
            "Feature name '{}' must start with an ASCII letter or underscore",
            name
        ));
    }
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '+' {
            return Err(anyhow!("Invalid character '{}' in feature name '{}'", c, name));
        }
    }
    Ok(())
}

/// Detailed dependency specification supporting version, path, git,
/// optional status, features, and default-features flags.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct DetailedDependency {
    pub version: Option<String>,
    pub path: Option<String>,
    pub git: Option<String>,
    pub tag: Option<String>,
    pub branch: Option<String>,
    pub rev: Option<String>,
    #[serde(default)]
    pub optional: Option<bool>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub default_features: Option<bool>,
    #[serde(default)]
    pub package: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum DependencyValue {
    Simple(String),
    Detailed(DetailedDependency),
}

impl DependencyValue {
    pub fn version_req(&self) -> Option<&str> {
        match self {
            DependencyValue::Simple(s) => Some(s.as_str()),
            DependencyValue::Detailed(d) => d.version.as_deref(),
        }
    }

    pub fn is_optional(&self) -> bool {
        match self {
            DependencyValue::Simple(_) => false,
            DependencyValue::Detailed(d) => d.optional.unwrap_or(false),
        }
    }

    pub fn features(&self) -> &[String] {
        match self {
            DependencyValue::Simple(_) => &[],
            DependencyValue::Detailed(d) => &d.features,
        }
    }
}

/// Package metadata table `[package]` in `bsr.mfz`
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub edition: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default)]
    pub main: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
}

/// Target-specific configuration `[target.<target-name>]`
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct TargetConfig {
    #[serde(default)]
    pub dependencies: HashMap<String, DependencyValue>,
    #[serde(default, rename = "dev-dependencies")]
    pub dev_dependencies: HashMap<String, DependencyValue>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub opt_level: Option<u32>,
    #[serde(default)]
    pub no_std: Option<bool>,
}

/// Compilation profile `[profile.<profile-name>]` (dev, release, bench, test)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct ProfileConfig {
    #[serde(default)]
    pub opt_level: Option<u32>,
    #[serde(default)]
    pub debug: Option<bool>,
    #[serde(default)]
    pub lto: Option<bool>,
    #[serde(default)]
    pub panic: Option<String>,
}

/// Workspace configuration `[workspace]`
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Complete in-memory representation of a Roze project manifest (`bsr.mfz`, `roze.mfz`, `roze.toml`).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    // Core direct fields for backwards compatibility:
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    pub main: String,
    #[serde(default)]
    pub edition: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub dependencies: HashMap<String, DependencyValue>,
    #[serde(default)]
    pub dev_dependencies: HashMap<String, DependencyValue>,
    #[serde(default)]
    pub target: Option<String>,

    // Extended M5-T02 Manifest capabilities:
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default)]
    pub build_dependencies: HashMap<String, DependencyValue>,
    #[serde(default)]
    pub features: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub targets: HashMap<String, TargetConfig>,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
    #[serde(default)]
    pub workspace: Option<WorkspaceConfig>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "my_project".to_string(),
            version: "0.1.0".to_string(),
            description: Some("A Roze project".to_string()),
            authors: vec!["Mohammad Firoze".to_string()],
            main: "src/main.roze".to_string(),
            edition: Some("2026".to_string()),
            license: Some("MIT".to_string()),
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
            target: Some("jvm".to_string()),
            homepage: None,
            repository: None,
            keywords: Vec::new(),
            readme: Some("README.md".to_string()),
            build_dependencies: HashMap::new(),
            features: HashMap::new(),
            targets: HashMap::new(),
            profiles: HashMap::new(),
            workspace: None,
        }
    }
}

impl ProjectConfig {
    pub fn find_manifest(dir: &Path) -> Option<PathBuf> {
        let bsr = dir.join(MANIFEST_NAME);
        if bsr.exists() {
            return Some(bsr);
        }
        let rz_mfz = dir.join(ALTERNATIVE_MANIFEST_NAME);
        if rz_mfz.exists() {
            return Some(rz_mfz);
        }
        let legacy = dir.join(LEGACY_MANIFEST_NAME);
        if legacy.exists() {
            return Some(legacy);
        }
        None
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self> {
        let root: toml::Value = toml::from_str(content)
            .map_err(|e| anyhow!("Failed to parse manifest TOML/MFZ syntax: {}", e))?;

        let table = root
            .as_table()
            .ok_or_else(|| anyhow!("Manifest root must be a TOML table"))?;

        let pkg_table = table.get("package").and_then(|v| v.as_table());

        let get_str = |key: &str| -> Option<String> {
            pkg_table
                .and_then(|p| p.get(key))
                .or_else(|| table.get(key))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };

        let get_string_list = |key: &str| -> Vec<String> {
            pkg_table
                .and_then(|p| p.get(key))
                .or_else(|| table.get(key))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        };

        let name = get_str("name")
            .ok_or_else(|| anyhow!("Manifest missing required 'name' field in [package]"))?;
        let version = get_str("version")
            .ok_or_else(|| anyhow!("Manifest missing required 'version' field in [package]"))?;

        let edition = get_str("edition").or_else(|| Some("2026".to_string()));
        let authors = get_string_list("authors");
        let license = get_str("license");
        let description = get_str("description");
        let homepage = get_str("homepage");
        let repository = get_str("repository");
        let keywords = get_string_list("keywords");
        let readme = get_str("readme");
        let main = get_str("main").unwrap_or_else(|| "src/main.roze".to_string());

        let default_target = pkg_table
            .and_then(|p| p.get("target"))
            .and_then(|v| v.as_str())
            .or_else(|| table.get("target").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .or_else(|| Some("jvm".to_string()));

        let parse_dep_val = |v: &toml::Value| -> Result<DependencyValue> {
            match v {
                toml::Value::String(s) => Ok(DependencyValue::Simple(s.clone())),
                toml::Value::Table(_) => {
                    let d: DetailedDependency = v.clone().try_into()
                        .map_err(|e| anyhow!("Invalid detailed dependency: {}", e))?;
                    Ok(DependencyValue::Detailed(d))
                }
                other => Err(anyhow!(
                    "Invalid dependency format: expected string or table, got {}",
                    other.type_str()
                )),
            }
        };

        let parse_deps_table =
            |val: Option<&toml::Value>| -> Result<HashMap<String, DependencyValue>> {
                let mut map = HashMap::new();
                if let Some(toml::Value::Table(tbl)) = val {
                    for (k, v) in tbl {
                        map.insert(k.clone(), parse_dep_val(v)?);
                    }
                }
                Ok(map)
            };

        let dependencies = parse_deps_table(table.get("dependencies"))?;
        let mut dev_dependencies = parse_deps_table(table.get("dev-dependencies"))?;
        if dev_dependencies.is_empty() {
            dev_dependencies = parse_deps_table(table.get("dev_dependencies"))?;
        }
        let mut build_dependencies = parse_deps_table(table.get("build-dependencies"))?;
        if build_dependencies.is_empty() {
            build_dependencies = parse_deps_table(table.get("build_dependencies"))?;
        }

        let mut features = HashMap::new();
        if let Some(toml::Value::Table(ftbl)) = table.get("features") {
            for (fk, fv) in ftbl {
                let items = match fv {
                    toml::Value::Array(arr) => {
                        let mut list = Vec::new();
                        for item in arr {
                            if let Some(s) = item.as_str() {
                                list.push(s.to_string());
                            } else {
                                return Err(anyhow!("Feature '{}' items must be strings", fk));
                            }
                        }
                        list
                    }
                    _ => return Err(anyhow!("Feature '{}' must be an array of feature names", fk)),
                };
                features.insert(fk.clone(), items);
            }
        }

        let mut targets = HashMap::new();
        if let Some(toml::Value::Table(ttbl)) = table.get("target") {
            for (tname, tval) in ttbl {
                if let toml::Value::Table(entry_tbl) = tval {
                    let mut tcfg = TargetConfig::default();
                    if let Some(flags_val) = entry_tbl.get("flags") {
                        if let Some(arr) = flags_val.as_array() {
                            tcfg.flags = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect();
                        }
                    }
                    if let Some(opt_val) = entry_tbl.get("opt_level").or_else(|| entry_tbl.get("opt-level")) {
                        tcfg.opt_level = opt_val.as_integer().map(|i| i as u32);
                    }
                    if let Some(ns_val) = entry_tbl.get("no_std").or_else(|| entry_tbl.get("no-std")) {
                        tcfg.no_std = ns_val.as_bool();
                    }
                    tcfg.dependencies = parse_deps_table(entry_tbl.get("dependencies"))?;
                    tcfg.dev_dependencies = parse_deps_table(
                        entry_tbl.get("dev-dependencies").or_else(|| entry_tbl.get("dev_dependencies")),
                    )?;
                    targets.insert(tname.clone(), tcfg);
                }
            }
        }

        let mut profiles = HashMap::new();
        if let Some(toml::Value::Table(ptbl)) = table.get("profile") {
            for (pname, pval) in ptbl {
                if let toml::Value::Table(entry_tbl) = pval {
                    let mut pcfg = ProfileConfig::default();
                    if let Some(opt_val) = entry_tbl.get("opt_level").or_else(|| entry_tbl.get("opt-level")) {
                        pcfg.opt_level = opt_val.as_integer().map(|i| i as u32);
                    }
                    if let Some(dbg_val) = entry_tbl.get("debug") {
                        pcfg.debug = dbg_val.as_bool();
                    }
                    if let Some(lto_val) = entry_tbl.get("lto") {
                        pcfg.lto = lto_val.as_bool();
                    }
                    if let Some(pan_val) = entry_tbl.get("panic") {
                        pcfg.panic = pan_val.as_str().map(|s| s.to_string());
                    }
                    profiles.insert(pname.clone(), pcfg);
                }
            }
        }

        let workspace = if let Some(toml::Value::Table(wtbl)) = table.get("workspace") {
            let mut wcfg = WorkspaceConfig::default();
            if let Some(m_val) = wtbl.get("members") {
                if let Some(arr) = m_val.as_array() {
                    wcfg.members = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                }
            }
            if let Some(e_val) = wtbl.get("exclude") {
                if let Some(arr) = e_val.as_array() {
                    wcfg.exclude = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                }
            }
            Some(wcfg)
        } else {
            None
        };

        let config = ProjectConfig {
            name,
            version,
            description,
            authors,
            main,
            edition,
            license,
            dependencies,
            dev_dependencies,
            target: default_target,
            homepage,
            repository,
            keywords,
            readme,
            build_dependencies,
            features,
            targets,
            profiles,
            workspace,
        };

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        validate_package_name(&self.name)
            .map_err(|e| anyhow!("Invalid package name in manifest: {}", e))?;

        Version::parse(&self.version)
            .map_err(|e| anyhow!("Invalid package version '{}' in manifest: {}", self.version, e))?;

        if let Some(ref ed) = self.edition {
            if ed.len() != 4 || !ed.chars().all(|c| c.is_ascii_digit()) {
                return Err(anyhow!(
                    "Invalid edition '{}' in manifest: expected 4-digit year (e.g. '2026')",
                    ed
                ));
            }
        }

        for (name, dep) in &self.dependencies {
            validate_package_name(name)
                .map_err(|e| anyhow!("Invalid dependency name '{}': {}", name, e))?;
            if let Some(req_str) = dep.version_req() {
                VersionReq::parse(req_str).map_err(|e| {
                    anyhow!(
                        "Invalid version requirement '{}' for dependency '{}': {}",
                        req_str,
                        name,
                        e
                    )
                })?;
            }
            if let DependencyValue::Detailed(d) = dep {
                if d.version.is_none() && d.path.is_none() && d.git.is_none() {
                    return Err(anyhow!(
                        "Dependency '{}' must specify at least one of 'version', 'path', or 'git'",
                        name
                    ));
                }
            }
        }

        for (name, dep) in &self.dev_dependencies {
            validate_package_name(name)
                .map_err(|e| anyhow!("Invalid dev-dependency name '{}': {}", name, e))?;
            if let Some(req_str) = dep.version_req() {
                VersionReq::parse(req_str).map_err(|e| {
                    anyhow!(
                        "Invalid version requirement '{}' for dev-dependency '{}': {}",
                        req_str,
                        name,
                        e
                    )
                })?;
            }
        }

        for (name, dep) in &self.build_dependencies {
            validate_package_name(name)
                .map_err(|e| anyhow!("Invalid build-dependency name '{}': {}", name, e))?;
            if let Some(req_str) = dep.version_req() {
                VersionReq::parse(req_str).map_err(|e| {
                    anyhow!(
                        "Invalid version requirement '{}' for build-dependency '{}': {}",
                        req_str,
                        name,
                        e
                    )
                })?;
            }
        }

        for (feat_name, feat_deps) in &self.features {
            validate_feature_name(feat_name)
                .map_err(|e| anyhow!("Invalid feature name '{}': {}", feat_name, e))?;
            for dep in feat_deps {
                if let Some(pkg) = dep.strip_prefix("dep:") {
                    if !self.dependencies.contains_key(pkg) {
                        return Err(anyhow!(
                            "Feature '{}' references 'dep:{}', but '{}' is not declared in [dependencies]",
                            feat_name,
                            pkg,
                            pkg
                        ));
                    }
                } else if let Some((pkg, _subfeat)) = dep.split_once('/') {
                    if !self.dependencies.contains_key(pkg) {
                        return Err(anyhow!(
                            "Feature '{}' references '{}/...', but '{}' is not declared in [dependencies]",
                            feat_name,
                            pkg,
                            pkg
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn serialize_mfz(&self) -> Result<String> {
        self.validate()?;
        let mut out = String::new();
        out.push_str(MANIFEST_HEADER);

        out.push_str("[package]\n");
        out.push_str(&format!("name = \"{}\"\n", self.name));
        out.push_str(&format!("version = \"{}\"\n", self.version));
        if let Some(ref ed) = self.edition {
            out.push_str(&format!("edition = \"{}\"\n", ed));
        }
        if !self.authors.is_empty() {
            let authors_str = self
                .authors
                .iter()
                .map(|a| format!("\"{}\"", a))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("authors = [{}]\n", authors_str));
        }
        if let Some(ref lic) = self.license {
            out.push_str(&format!("license = \"{}\"\n", lic));
        }
        if let Some(ref desc) = self.description {
            out.push_str(&format!("description = \"{}\"\n", desc));
        }
        if let Some(ref hp) = self.homepage {
            out.push_str(&format!("homepage = \"{}\"\n", hp));
        }
        if let Some(ref repo) = self.repository {
            out.push_str(&format!("repository = \"{}\"\n", repo));
        }
        if !self.keywords.is_empty() {
            let kw_str = self
                .keywords
                .iter()
                .map(|k| format!("\"{}\"", k))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("keywords = [{}]\n", kw_str));
        }
        if let Some(ref rm) = self.readme {
            out.push_str(&format!("readme = \"{}\"\n", rm));
        }
        out.push_str(&format!("main = \"{}\"\n", self.main));
        if let Some(ref tgt) = self.target {
            out.push_str(&format!("target = \"{}\"\n", tgt));
        }

        if !self.features.is_empty() {
            out.push_str("\n[features]\n");
            let mut sorted_features: Vec<_> = self.features.iter().collect();
            sorted_features.sort_by_key(|(k, _)| *k);
            for (feat_name, feat_deps) in sorted_features {
                let items = feat_deps
                    .iter()
                    .map(|d| format!("\"{}\"", d))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("{} = [{}]\n", feat_name, items));
            }
        }

        fn write_deps(out: &mut String, header: &str, deps: &HashMap<String, DependencyValue>) {
            if deps.is_empty() && header != "[dependencies]" {
                return;
            }
            out.push_str(&format!("\n{}\n", header));
            let mut sorted: Vec<_> = deps.iter().collect();
            sorted.sort_by_key(|(k, _)| *k);
            for (name, val) in sorted {
                match val {
                    DependencyValue::Simple(ver) => {
                        out.push_str(&format!("{} = \"{}\"\n", name, ver));
                    }
                    DependencyValue::Detailed(d) => {
                        let mut parts = Vec::new();
                        if let Some(ref v) = d.version {
                            parts.push(format!("version = \"{}\"", v));
                        }
                        if let Some(ref p) = d.path {
                            parts.push(format!("path = \"{}\"", p));
                        }
                        if let Some(ref g) = d.git {
                            parts.push(format!("git = \"{}\"", g));
                        }
                        if let Some(ref t) = d.tag {
                            parts.push(format!("tag = \"{}\"", t));
                        }
                        if let Some(ref b) = d.branch {
                            parts.push(format!("branch = \"{}\"", b));
                        }
                        if let Some(ref r) = d.rev {
                            parts.push(format!("rev = \"{}\"", r));
                        }
                        if let Some(opt) = d.optional {
                            if opt {
                                parts.push("optional = true".to_string());
                            }
                        }
                        if !d.features.is_empty() {
                            let feats = d
                                .features
                                .iter()
                                .map(|f| format!("\"{}\"", f))
                                .collect::<Vec<_>>()
                                .join(", ");
                            parts.push(format!("features = [{}]", feats));
                        }
                        if let Some(df) = d.default_features {
                            if !df {
                                parts.push("default_features = false".to_string());
                            }
                        }
                        if let Some(ref pkg) = d.package {
                            parts.push(format!("package = \"{}\"", pkg));
                        }
                        out.push_str(&format!("{} = {{ {} }}\n", name, parts.join(", ")));
                    }
                }
            }
        }

        write_deps(&mut out, "[dependencies]", &self.dependencies);
        write_deps(&mut out, "[dev-dependencies]", &self.dev_dependencies);
        write_deps(&mut out, "[build-dependencies]", &self.build_dependencies);

        if !self.targets.is_empty() {
            let mut sorted_targets: Vec<_> = self.targets.iter().collect();
            sorted_targets.sort_by_key(|(k, _)| *k);
            for (tname, tcfg) in sorted_targets {
                out.push_str(&format!("\n[target.{}]\n", tname));
                if !tcfg.flags.is_empty() {
                    let flags_str = tcfg
                        .flags
                        .iter()
                        .map(|f| format!("\"{}\"", f))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(&format!("flags = [{}]\n", flags_str));
                }
                if let Some(opt) = tcfg.opt_level {
                    out.push_str(&format!("opt_level = {}\n", opt));
                }
                if let Some(ns) = tcfg.no_std {
                    if ns {
                        out.push_str("no_std = true\n");
                    }
                }
                if !tcfg.dependencies.is_empty() {
                    let subheader = format!("[target.{}.dependencies]", tname);
                    write_deps(&mut out, &subheader, &tcfg.dependencies);
                }
            }
        }

        if !self.profiles.is_empty() {
            let mut sorted_profiles: Vec<_> = self.profiles.iter().collect();
            sorted_profiles.sort_by_key(|(k, _)| *k);
            for (pname, pcfg) in sorted_profiles {
                out.push_str(&format!("\n[profile.{}]\n", pname));
                if let Some(opt) = pcfg.opt_level {
                    out.push_str(&format!("opt_level = {}\n", opt));
                }
                if let Some(dbg) = pcfg.debug {
                    out.push_str(&format!("debug = {}\n", dbg));
                }
                if let Some(lto) = pcfg.lto {
                    out.push_str(&format!("lto = {}\n", lto));
                }
                if let Some(ref pan) = pcfg.panic {
                    out.push_str(&format!("panic = \"{}\"\n", pan));
                }
            }
        }

        if let Some(ref ws) = self.workspace {
            out.push_str("\n[workspace]\n");
            if !ws.members.is_empty() {
                let m_str = ws
                    .members
                    .iter()
                    .map(|m| format!("\"{}\"", m))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("members = [{}]\n", m_str));
            }
            if !ws.exclude.is_empty() {
                let e_str = ws
                    .exclude
                    .iter()
                    .map(|e| format!("\"{}\"", e))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("exclude = [{}]\n", e_str));
            }
        }

        Ok(out)
    }

    pub fn serialize_toml(&self) -> Result<String> {
        let mfz = self.serialize_mfz()?;
        Ok(mfz.trim_start_matches(MANIFEST_HEADER).to_string())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let is_mfz = path.file_name().and_then(|s| s.to_str()) != Some(LEGACY_MANIFEST_NAME);
        let content = if is_mfz {
            self.serialize_mfz()?
        } else {
            self.serialize_toml()?
        };
        fs::write(path, content)?;
        Ok(())
    }

    pub fn resolve_features(&self, requested: &[&str]) -> Result<HashSet<String>> {
        let mut enabled = HashSet::new();
        let mut to_process = Vec::new();

        if requested.is_empty() {
            if self.features.contains_key("default") {
                to_process.push("default".to_string());
            }
        } else {
            for req in requested {
                to_process.push(req.to_string());
            }
        }

        let mut visited = HashSet::new();

        while let Some(current) = to_process.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }

            enabled.insert(current.clone());

            if let Some(sub_features) = self.features.get(&current) {
                for sub in sub_features {
                    to_process.push(sub.clone());
                }
            } else if let Some(dep) = self.dependencies.get(&current) {
                if !dep.is_optional() {
                    return Err(anyhow!(
                        "Package '{}' does not have feature '{}'",
                        self.name,
                        current
                    ));
                }
            } else if current.starts_with("dep:") || current.contains('/') {
                // Dependency feature activation specifiers
            } else {
                return Err(anyhow!(
                    "Package '{}' does not have feature '{}'",
                    self.name,
                    current
                ));
            }
        }

        Ok(enabled)
    }

    pub fn activated_dependencies(
        &self,
        active_features: &HashSet<String>,
    ) -> HashMap<String, DependencyValue> {
        let mut result = HashMap::new();

        for (name, dep) in &self.dependencies {
            if !dep.is_optional() {
                result.insert(name.clone(), dep.clone());
            } else {
                let dep_flag = format!("dep:{}", name);
                if active_features.contains(name) || active_features.contains(&dep_flag) {
                    result.insert(name.clone(), dep.clone());
                }
            }
        }

        result
    }

    pub fn effective_dependencies(&self, target_name: &str) -> HashMap<String, DependencyValue> {
        let mut result = self.dependencies.clone();
        if let Some(target_cfg) = self.targets.get(target_name) {
            for (k, v) in &target_cfg.dependencies {
                result.insert(k.clone(), v.clone());
            }
        }
        result
    }

    pub fn effective_dev_dependencies(&self, target_name: &str) -> HashMap<String, DependencyValue> {
        let mut result = self.dev_dependencies.clone();
        if let Some(target_cfg) = self.targets.get(target_name) {
            for (k, v) in &target_cfg.dev_dependencies {
                result.insert(k.clone(), v.clone());
            }
        }
        result
    }

    pub fn target_flags(&self, target_name: &str) -> Vec<String> {
        self.targets
            .get(target_name)
            .map(|t| t.flags.clone())
            .unwrap_or_default()
    }

    pub fn effective_profile(&self, profile_name: &str) -> ProfileConfig {
        let mut base = match profile_name {
            "release" => ProfileConfig {
                opt_level: Some(3),
                debug: Some(false),
                lto: Some(true),
                panic: Some("abort".to_string()),
            },
            "bench" => ProfileConfig {
                opt_level: Some(3),
                debug: Some(false),
                lto: Some(true),
                panic: Some("abort".to_string()),
            },
            _ => ProfileConfig {
                opt_level: Some(0),
                debug: Some(true),
                lto: Some(false),
                panic: Some("unwind".to_string()),
            },
        };

        if let Some(custom) = self.profiles.get(profile_name) {
            if custom.opt_level.is_some() {
                base.opt_level = custom.opt_level;
            }
            if custom.debug.is_some() {
                base.debug = custom.debug;
            }
            if custom.lto.is_some() {
                base.lto = custom.lto;
            }
            if custom.panic.is_some() {
                base.panic = custom.panic.clone();
            }
        }

        base
    }

    pub fn to_package_metadata(&self) -> PackageMetadata {
        PackageMetadata {
            name: self.name.clone(),
            version: self.version.clone(),
            edition: self.edition.clone(),
            authors: self.authors.clone(),
            license: self.license.clone(),
            description: self.description.clone(),
            homepage: self.homepage.clone(),
            repository: self.repository.clone(),
            keywords: self.keywords.clone(),
            readme: self.readme.clone(),
            main: Some(self.main.clone()),
            target: self.target.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_modern_bsr_mfz() {
        let content = r#"# bsr.mfz - Basir Project Manifest (Mohammad Firoze)

[package]
name = "roze_server"
version = "2.1.0"
edition = "2026"
authors = ["Mohammad Firoze <firoze@roze.org>"]
license = "Apache-2.0"
description = "High-concurrency async web server written in Roze"
homepage = "https://roze.dev/server"
repository = "https://github.com/roze-lang/roze-server"
keywords = ["http", "async", "loom", "fast"]
readme = "README.md"
main = "src/main.roze"
target = "jvm"

[features]
default = ["std", "compression"]
std = []
compression = ["dep:zstd"]
tls = ["dep:rustls"]
full = ["compression", "tls"]

[dependencies]
http = "0.5.0"
serde = { version = "1.0.0", features = ["derive"] }
zstd = { version = "0.12.0", optional = true }
rustls = { git = "https://github.com/rustls/rustls", tag = "v0.21.0", optional = true }
local_util = { path = "../local_util" }

[dev-dependencies]
test_harness = "0.1.0"

[build-dependencies]
proto_gen = "0.3.0"

[target.jvm]
dependencies = { jvm_loom = "0.1.0" }
flags = ["-Xmx4G", "--enable-preview"]

[target.native]
dependencies = { libc = "0.2.0" }
opt_level = 3

[profile.release]
opt_level = 3
lto = true
debug = false
"#;

        let config = ProjectConfig::parse(content).expect("parse failed");
        assert_eq!(config.name, "roze_server");
        assert_eq!(config.version, "2.1.0");
        assert_eq!(config.edition, Some("2026".to_string()));
        assert_eq!(config.license, Some("Apache-2.0".to_string()));
        assert_eq!(
            config.description,
            Some("High-concurrency async web server written in Roze".to_string())
        );
        assert_eq!(config.homepage, Some("https://roze.dev/server".to_string()));
        assert_eq!(
            config.repository,
            Some("https://github.com/roze-lang/roze-server".to_string())
        );
        assert_eq!(config.keywords, vec!["http", "async", "loom", "fast"]);
        assert_eq!(config.readme, Some("README.md".to_string()));
        assert_eq!(config.main, "src/main.roze");
        assert_eq!(config.target, Some("jvm".to_string()));

        assert_eq!(config.dependencies.len(), 5);
        assert_eq!(config.dev_dependencies.len(), 1);
        assert_eq!(config.build_dependencies.len(), 1);
        assert_eq!(config.features.len(), 5);
        assert_eq!(config.targets.len(), 2);
        assert_eq!(config.profiles.len(), 1);
    }

    #[test]
    fn test_parse_flat_legacy_manifest() {
        let content = r#"name = "legacy_pkg"
version = "0.2.1"
main = "src/main.roze"
description = "A legacy flat manifest"
authors = ["Firoze"]

[dependencies]
math = "^1.0.0"
"#;
        let config = ProjectConfig::parse(content).expect("parse legacy failed");
        assert_eq!(config.name, "legacy_pkg");
        assert_eq!(config.version, "0.2.1");
        assert_eq!(config.main, "src/main.roze");
        assert_eq!(config.edition, Some("2026".to_string()));
        assert_eq!(config.dependencies.len(), 1);
        assert_eq!(
            config.dependencies.get("math").unwrap().version_req(),
            Some("^1.0.0")
        );
    }

    #[test]
    fn test_feature_resolution_and_dependency_activation() {
        let content = r#"
[package]
name = "feature_pkg"
version = "1.0.0"

[features]
default = ["std", "compression"]
std = []
compression = ["dep:zstd"]
tls = ["dep:rustls"]
full = ["compression", "tls"]

[dependencies]
core = "1.0.0"
zstd = { version = "0.12.0", optional = true }
rustls = { version = "0.21.0", optional = true }
"#;
        let config = ProjectConfig::parse(content).expect("parse failed");

        // Resolve default features
        let default_feats = config.resolve_features(&[]).expect("resolve default");
        assert!(default_feats.contains("default"));
        assert!(default_feats.contains("std"));
        assert!(default_feats.contains("compression"));
        assert!(default_feats.contains("dep:zstd"));
        assert!(!default_feats.contains("tls"));

        let activated = config.activated_dependencies(&default_feats);
        assert!(activated.contains_key("core")); // non-optional
        assert!(activated.contains_key("zstd")); // activated by compression
        assert!(!activated.contains_key("rustls")); // inactive

        // Resolve explicit "full" feature
        let full_feats = config.resolve_features(&["full"]).expect("resolve full");
        assert!(full_feats.contains("full"));
        assert!(full_feats.contains("compression"));
        assert!(full_feats.contains("dep:zstd"));
        assert!(full_feats.contains("tls"));
        assert!(full_feats.contains("dep:rustls"));

        let activated_full = config.activated_dependencies(&full_feats);
        assert!(activated_full.contains_key("core"));
        assert!(activated_full.contains_key("zstd"));
        assert!(activated_full.contains_key("rustls"));
    }

    #[test]
    fn test_feature_cycle_detection() {
        let content = r#"
[package]
name = "cycle_pkg"
version = "1.0.0"

[features]
a = ["b"]
b = ["a"]
"#;
        let config = ProjectConfig::parse(content).expect("parse failed");
        let feats = config.resolve_features(&["a"]).expect("resolve cycle");
        assert!(feats.contains("a"));
        assert!(feats.contains("b"));
    }

    #[test]
    fn test_feature_unknown_error() {
        let content = r#"
[package]
name = "unknown_feat_pkg"
version = "1.0.0"

[features]
foo = []
"#;
        let config = ProjectConfig::parse(content).expect("parse failed");
        let res = config.resolve_features(&["nonexistent"]);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("does not have feature 'nonexistent'"));
    }

    #[test]
    fn test_target_configurations_and_effective_dependencies() {
        let content = r#"
[package]
name = "target_pkg"
version = "1.0.0"

[dependencies]
base_lib = "1.0.0"

[target.jvm]
dependencies = { jvm_runtime = "0.5.0" }
flags = ["-Xmx2G"]

[target.native]
dependencies = { libc = "0.2.0" }
flags = ["-O3"]
opt_level = 3
"#;
        let config = ProjectConfig::parse(content).expect("parse failed");

        let jvm_deps = config.effective_dependencies("jvm");
        assert!(jvm_deps.contains_key("base_lib"));
        assert!(jvm_deps.contains_key("jvm_runtime"));
        assert!(!jvm_deps.contains_key("libc"));
        assert_eq!(config.target_flags("jvm"), vec!["-Xmx2G"]);

        let native_deps = config.effective_dependencies("native");
        assert!(native_deps.contains_key("base_lib"));
        assert!(native_deps.contains_key("libc"));
        assert!(!native_deps.contains_key("jvm_runtime"));
        assert_eq!(config.target_flags("native"), vec!["-O3"]);
    }

    #[test]
    fn test_profile_resolution() {
        let content = r#"
[package]
name = "profile_pkg"
version = "1.0.0"

[profile.release]
opt_level = 3
lto = true
panic = "abort"
"#;
        let config = ProjectConfig::parse(content).expect("parse failed");
        let rel_prof = config.effective_profile("release");
        assert_eq!(rel_prof.opt_level, Some(3));
        assert_eq!(rel_prof.lto, Some(true));
        assert_eq!(rel_prof.panic, Some("abort".to_string()));

        let dev_prof = config.effective_profile("dev");
        assert_eq!(dev_prof.opt_level, Some(0));
        assert_eq!(dev_prof.debug, Some(true));
    }

    #[test]
    fn test_manifest_roundtrip_serialization() {
        let orig = ProjectConfig {
            name: "roundtrip_app".to_string(),
            version: "1.2.3".to_string(),
            description: Some("Roundtrip test app".to_string()),
            authors: vec!["Mohammad Firoze".to_string()],
            main: "src/main.roze".to_string(),
            edition: Some("2026".to_string()),
            license: Some("MIT".to_string()),
            dependencies: {
                let mut d = HashMap::new();
                d.insert("serde".to_string(), DependencyValue::Simple("^1.0.0".to_string()));
                d.insert(
                    "tokio".to_string(),
                    DependencyValue::Detailed(DetailedDependency {
                        version: Some("1.28.0".to_string()),
                        optional: Some(true),
                        features: vec!["sync".to_string(), "rt".to_string()],
                        ..Default::default()
                    }),
                );
                d
            },
            dev_dependencies: HashMap::new(),
            target: Some("native".to_string()),
            homepage: Some("https://roze.dev".to_string()),
            repository: Some("https://github.com/roze/roundtrip".to_string()),
            keywords: vec!["roundtrip".to_string()],
            readme: Some("README.md".to_string()),
            build_dependencies: HashMap::new(),
            features: {
                let mut f = HashMap::new();
                f.insert("default".to_string(), vec!["serde".to_string()]);
                f.insert("async".to_string(), vec!["dep:tokio".to_string()]);
                f
            },
            targets: {
                let mut t = HashMap::new();
                t.insert(
                    "native".to_string(),
                    TargetConfig {
                        flags: vec!["-O3".to_string()],
                        opt_level: Some(3),
                        ..Default::default()
                    },
                );
                t
            },
            profiles: {
                let mut p = HashMap::new();
                p.insert(
                    "release".to_string(),
                    ProfileConfig {
                        opt_level: Some(3),
                        lto: Some(true),
                        ..Default::default()
                    },
                );
                p
            },
            workspace: None,
        };

        let serialized = orig.serialize_mfz().expect("serialize failed");
        assert!(serialized.contains("# bsr.mfz - Basir Project Manifest (Mohammad Firoze)"));
        assert!(serialized.contains("[package]"));
        assert!(serialized.contains("name = \"roundtrip_app\""));
        assert!(serialized.contains("[features]"));
        assert!(serialized.contains("[dependencies]"));
        assert!(serialized.contains("[target.native]"));
        assert!(serialized.contains("[profile.release]"));

        let roundtrip = ProjectConfig::parse(&serialized).expect("parse roundtrip failed");
        assert_eq!(roundtrip.name, orig.name);
        assert_eq!(roundtrip.version, orig.version);
        assert_eq!(roundtrip.edition, orig.edition);
        assert_eq!(roundtrip.dependencies.len(), orig.dependencies.len());
        assert_eq!(roundtrip.features.len(), orig.features.len());
        assert_eq!(roundtrip.targets.len(), orig.targets.len());
    }

    #[test]
    fn test_validation_errors() {
        // Missing name
        assert!(ProjectConfig::parse("version = '1.0.0'").is_err());

        // Missing version
        assert!(ProjectConfig::parse("name = 'foo'").is_err());

        // Invalid semver
        assert!(ProjectConfig::parse("name = 'foo'\nversion = 'invalid'").is_err());

        // Invalid package name with spaces
        assert!(ProjectConfig::parse("name = 'foo bar'\nversion = '1.0.0'").is_err());

        // Reserved keyword
        assert!(ProjectConfig::parse("name = 'func'\nversion = '1.0.0'").is_err());

        // Invalid dependency semver requirement
        let bad_dep = r#"
name = "foo"
version = "1.0.0"
[dependencies]
bar = "invalid-comparator"
"#;
        assert!(ProjectConfig::parse(bad_dep).is_err());

        // Feature references undeclared dependency
        let bad_feat = r#"
name = "foo"
version = "1.0.0"
[features]
custom = ["dep:non_existent"]
"#;
        assert!(ProjectConfig::parse(bad_feat).is_err());
    }
}
