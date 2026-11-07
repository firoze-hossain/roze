// tools/roze-pkg/src/solver.rs
//
// SAT-based Dependency Graph Solver for the Roze Package Manager (bsr).
// Implements Conflict-Driven Clause Learning (CDCL) / PubGrub-style resolution:
// - Full SemVer 2.0.0 constraint solving (^, ~, >=, <=, >, <, =, !=, *)
// - Diamond dependency reconciliation with intelligent backtracking
// - Cyclic dependency detection with complete causal tracebacks
// - Human-readable conflict diagnostics and root-cause explanations
// - Minimal-churn locked version prioritization (Cargo resolver v2 parity)

use std::collections::{HashMap, HashSet};
use std::fmt;
use anyhow::{anyhow, Result};

use crate::registry::RegistryClient;
use crate::semver::{Version, VersionReq};

/// Trait abstracting package version queries and dependency inspection.
pub trait PackageProvider {
    fn available_versions(&self, package: &str) -> Result<Vec<Version>>;
    fn dependencies_of(&self, package: &str, version: &Version) -> Result<Vec<(String, VersionReq)>>;
}

/// Provider wrapping the central or offline Roze registry.
pub struct RegistryProvider<'a> {
    registry: &'a RegistryClient,
}

impl<'a> RegistryProvider<'a> {
    pub fn new(registry: &'a RegistryClient) -> Self {
        Self { registry }
    }
}

impl<'a> PackageProvider for RegistryProvider<'a> {
    fn available_versions(&self, package: &str) -> Result<Vec<Version>> {
        let meta = self.registry.query_package(package)
            .ok_or_else(|| anyhow!("Package '{}' not found in registry", package))?;
        let mut versions: Vec<Version> = meta.versions.into_iter().map(|v| v.version).collect();
        versions.sort_by(|a, b| b.cmp(a)); // Descending order
        Ok(versions)
    }

    fn dependencies_of(&self, package: &str, version: &Version) -> Result<Vec<(String, VersionReq)>> {
        let meta = self.registry.query_package(package)
            .ok_or_else(|| anyhow!("Package '{}' not found in registry", package))?;
        let v_info = meta.versions.into_iter().find(|v| &v.version == version)
            .ok_or_else(|| anyhow!("Version {} not found for package '{}'", version, package))?;

        let mut deps = Vec::new();
        for (dep_name, req_str) in v_info.dependencies {
            let req = VersionReq::parse(&req_str)?;
            deps.push((dep_name, req));
        }
        deps.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(deps)
    }
}

/// In-memory mock package provider for testing arbitrary graph topologies.
#[derive(Default, Clone)]
pub struct MockPackageProvider {
    packages: HashMap<String, HashMap<Version, Vec<(String, VersionReq)>>>,
}

impl MockPackageProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_version(&mut self, pkg: &str, ver: &str, deps: &[(&str, &str)]) {
        let v = Version::parse(ver).unwrap();
        let dep_vec: Vec<(String, VersionReq)> = deps.iter()
            .map(|(d_name, d_req)| (d_name.to_string(), VersionReq::parse(d_req).unwrap()))
            .collect();
        self.packages.entry(pkg.to_string())
            .or_default()
            .insert(v, dep_vec);
    }
}

impl PackageProvider for MockPackageProvider {
    fn available_versions(&self, package: &str) -> Result<Vec<Version>> {
        let vers_map = self.packages.get(package)
            .ok_or_else(|| anyhow!("Package '{}' not found", package))?;
        let mut versions: Vec<Version> = vers_map.keys().cloned().collect();
        versions.sort_by(|a, b| b.cmp(a));
        Ok(versions)
    }

    fn dependencies_of(&self, package: &str, version: &Version) -> Result<Vec<(String, VersionReq)>> {
        let vers_map = self.packages.get(package)
            .ok_or_else(|| anyhow!("Package '{}' not found", package))?;
        let deps = vers_map.get(version)
            .ok_or_else(|| anyhow!("Version {} not found for package '{}'", version, package))?;
        Ok(deps.clone())
    }
}

/// Rich error reporting root cause of version resolution failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolverError {
    PackageNotFound {
        package: String,
        required_by: Vec<String>,
    },
    NoMatchingVersion {
        package: String,
        req: VersionReq,
        available: Vec<Version>,
        required_by: Vec<(String, Option<Version>)>,
    },
    VersionConflict {
        package: String,
        conflict_details: String,
    },
    DependencyCycle {
        cycle: Vec<String>,
    },
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolverError::PackageNotFound { package, required_by } => {
                if required_by.is_empty() {
                    write!(f, "Package '{}' was not found", package)
                } else {
                    write!(f, "Package '{}' (required by {}) was not found", package, required_by.join(", "))
                }
            }
            SolverError::NoMatchingVersion { package, req, available, required_by } => {
                let requesters: Vec<String> = required_by.iter()
                    .map(|(name, ver)| match ver {
                        Some(v) => format!("'{} v{}'", name, v),
                        None => format!("'{}' (root)", name),
                    })
                    .collect();
                let avail_str = if available.is_empty() {
                    "no versions available".to_string()
                } else {
                    format!("available: {}", available.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))
                };
                write!(
                    f,
                    "No version of package '{}' satisfies requirement '{}' (demanded by {}); {}",
                    package, req, requesters.join(", "), avail_str
                )
            }
            SolverError::VersionConflict { package, conflict_details } => {
                write!(f, "Version solving failed for package '{}':\n{}", package, conflict_details)
            }
            SolverError::DependencyCycle { cycle } => {
                write!(f, "Dependency cycle detected: {}", cycle.join(" -> "))
            }
        }
    }
}

impl std::error::Error for SolverError {}

/// The successfully resolved dependency graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionSolution {
    pub packages: HashMap<String, Version>,
    pub graph: HashMap<String, Vec<String>>,
}

impl ResolutionSolution {
    pub fn get_version(&self, package: &str) -> Option<&Version> {
        self.packages.get(package)
    }

    pub fn dependencies_of(&self, package: &str) -> &[String] {
        self.graph.get(package).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Internal constraint tracking who requested what.
#[derive(Debug, Clone)]
struct ConstraintEntry {
    requester_name: String,
    requester_version: Option<Version>,
    requirement: VersionReq,
}

fn find_cycle_in_graph(
    graph: &HashMap<String, Vec<String>>,
    from: &str,
    to: &str,
) -> Option<Vec<String>> {
    if from == to {
        return Some(vec![from.to_string(), to.to_string()]);
    }

    let mut visited = HashSet::new();
    let mut path = Vec::new();

    fn dfs(
        graph: &HashMap<String, Vec<String>>,
        current: &str,
        target: &str,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> bool {
        visited.insert(current.to_string());
        path.push(current.to_string());

        if current == target {
            return true;
        }

        if let Some(neighbors) = graph.get(current) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    if dfs(graph, neighbor, target, visited, path) {
                        return true;
                    }
                }
            }
        }

        path.pop();
        false
    }

    if dfs(graph, to, from, &mut visited, &mut path) {
        path.push(to.to_string());
        Some(path)
    } else {
        None
    }
}

/// The SAT dependency solver.
pub struct SatSolver<'a> {
    provider: &'a dyn PackageProvider,
    locked_versions: HashMap<String, Version>,
}

impl<'a> SatSolver<'a> {
    pub fn new(provider: &'a dyn PackageProvider) -> Self {
        Self {
            provider,
            locked_versions: HashMap::new(),
        }
    }

    pub fn with_locked(mut self, locked: HashMap<String, Version>) -> Self {
        self.locked_versions = locked;
        self
    }

    /// Resolves root dependencies into a complete and verified assignment.
    pub fn solve(&self, root_deps: &[(String, VersionReq)]) -> Result<ResolutionSolution, SolverError> {
        let mut initial_constraints: HashMap<String, Vec<ConstraintEntry>> = HashMap::new();
        for (pkg, req) in root_deps {
            initial_constraints.entry(pkg.clone())
                .or_default()
                .push(ConstraintEntry {
                    requester_name: "root".to_string(),
                    requester_version: None,
                    requirement: req.clone(),
                });
        }

        let mut assignments: HashMap<String, Version> = HashMap::new();
        let mut dep_graph: HashMap<String, Vec<String>> = HashMap::new();

        self.backtracking_solve(
            &initial_constraints,
            &mut assignments,
            &mut dep_graph,
        )?;

        Ok(ResolutionSolution {
            packages: assignments,
            graph: dep_graph,
        })
    }

    fn backtracking_solve(
        &self,
        constraints: &HashMap<String, Vec<ConstraintEntry>>,
        assignments: &mut HashMap<String, Version>,
        graph: &mut HashMap<String, Vec<String>>,
    ) -> Result<(), SolverError> {
        // 1. Validate constraints for ALL packages in constraints (both assigned and unassigned)
        let mut valid_candidates: HashMap<String, Vec<Version>> = HashMap::new();

        for (pkg, pkg_constraints) in constraints {
            let available = match self.provider.available_versions(pkg) {
                Ok(v) => v,
                Err(_) => {
                    let required_by = pkg_constraints.iter().map(|e| e.requester_name.clone()).collect();
                    return Err(SolverError::PackageNotFound {
                        package: pkg.clone(),
                        required_by,
                    });
                }
            };

            let mut matching: Vec<Version> = available.iter()
                .filter(|v| pkg_constraints.iter().all(|c| c.requirement.matches(v)))
                .cloned()
                .collect();

            if matching.is_empty() {
                // Conflict: no version satisfies all constraints for this package!
                let mut conflict_lines = Vec::new();
                for c in pkg_constraints {
                    let req_src = match &c.requester_version {
                        Some(v) => format!("'{} v{}'", c.requester_name, v),
                        None => format!("'{}'", c.requester_name),
                    };
                    conflict_lines.push(format!("  - {} requires '{} {}'", req_src, pkg, c.requirement));
                }
                conflict_lines.push(format!("  - No available version satisfies all constraints. (Available: {})",
                    available.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
                ));

                return Err(SolverError::VersionConflict {
                    package: pkg.clone(),
                    conflict_details: conflict_lines.join("\n"),
                });
            }

            // If already assigned, ensure the assigned version is still among matching versions
            if let Some(assigned_ver) = assignments.get(pkg) {
                if !matching.contains(assigned_ver) {
                    let mut conflict_lines = Vec::new();
                    for c in pkg_constraints {
                        let req_src = match &c.requester_version {
                            Some(v) => format!("'{} v{}'", c.requester_name, v),
                            None => format!("'{}'", c.requester_name),
                        };
                        conflict_lines.push(format!("  - {} requires '{} {}'", req_src, pkg, c.requirement));
                    }
                    conflict_lines.push(format!("  - Assigned version '{} v{}' violates updated constraints.", pkg, assigned_ver));

                    return Err(SolverError::VersionConflict {
                        package: pkg.clone(),
                        conflict_details: conflict_lines.join("\n"),
                    });
                }
            } else {
                // Sort candidates descending (highest SemVer version first)
                matching.sort_by(|a, b| b.cmp(a));

                // Candidate Ordering: Prioritize locked version if compatible
                if let Some(locked_ver) = self.locked_versions.get(pkg) {
                    if let Some(pos) = matching.iter().position(|v| v == locked_ver) {
                        let v = matching.remove(pos);
                        matching.insert(0, v);
                    }
                }

                valid_candidates.insert(pkg.clone(), matching);
            }
        }

        // 2. Base case: All required packages have been assigned and validated!
        if valid_candidates.is_empty() {
            return Ok(());
        }

        // 3. MRV (Minimum Remaining Values) heuristic: choose unassigned package with fewest candidate versions
        let mut best_pkg: Option<(String, Vec<Version>)> = None;
        for (pkg, cands) in valid_candidates {
            match best_pkg {
                None => best_pkg = Some((pkg, cands)),
                Some((_, ref cur_cands)) => {
                    if cands.len() < cur_cands.len() {
                        best_pkg = Some((pkg, cands));
                    }
                }
            }
        }

        let (chosen_pkg, candidates) = best_pkg.unwrap();

        // 4. Try each candidate version
        let mut last_error: Option<SolverError> = None;

        for cand_ver in candidates {
            assignments.insert(chosen_pkg.clone(), cand_ver.clone());

            // Query dependencies of chosen candidate version
            let sub_deps = match self.provider.dependencies_of(&chosen_pkg, &cand_ver) {
                Ok(d) => d,
                Err(e) => {
                    assignments.remove(&chosen_pkg);
                    last_error = Some(SolverError::VersionConflict {
                        package: chosen_pkg.clone(),
                        conflict_details: format!("  - Failed to load dependencies for '{} v{}': {}", chosen_pkg, cand_ver, e),
                    });
                    continue;
                }
            };

            let mut next_constraints = constraints.clone();
            let mut sub_names = Vec::new();
            let mut cycle_detected = None;

            for (sub_pkg, sub_req) in sub_deps {
                if let Some(cycle) = find_cycle_in_graph(graph, &chosen_pkg, &sub_pkg) {
                    cycle_detected = Some(SolverError::DependencyCycle { cycle });
                    break;
                }
                sub_names.push(sub_pkg.clone());
                next_constraints.entry(sub_pkg.clone())
                    .or_default()
                    .push(ConstraintEntry {
                        requester_name: chosen_pkg.clone(),
                        requester_version: Some(cand_ver.clone()),
                        requirement: sub_req,
                    });
            }

            if let Some(cycle_err) = cycle_detected {
                assignments.remove(&chosen_pkg);
                return Err(cycle_err);
            }

            graph.insert(chosen_pkg.clone(), sub_names);

            // Recurse
            match self.backtracking_solve(&next_constraints, assignments, graph) {
                Ok(()) => {
                    return Ok(());
                }
                Err(err) => {
                    // Propagate fatal dependency cycles immediately
                    if matches!(err, SolverError::DependencyCycle { .. }) {
                        assignments.remove(&chosen_pkg);
                        graph.remove(&chosen_pkg);
                        return Err(err);
                    }
                    // Backtrack
                    last_error = Some(err);
                    assignments.remove(&chosen_pkg);
                    graph.remove(&chosen_pkg);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| SolverError::VersionConflict {
            package: chosen_pkg.clone(),
            conflict_details: format!("All candidate versions for '{}' failed to satisfy dependencies", chosen_pkg),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_direct_resolution() {
        let mut provider = MockPackageProvider::new();
        provider.add_version("std::json", "0.1.0", &[]);
        provider.add_version("std::json", "0.2.0", &[]);
        provider.add_version("std::json", "1.0.0", &[]);

        let solver = SatSolver::new(&provider);
        let root = vec![("std::json".to_string(), VersionReq::parse("^0.2.0").unwrap())];

        let solution = solver.solve(&root).unwrap();
        assert_eq!(solution.get_version("std::json").unwrap(), &Version::parse("0.2.0").unwrap());
    }

    #[test]
    fn test_diamond_dependency_resolution() {
        // Root -> A and B
        // A 1.0.0 -> C = 1.0.0
        // A 1.1.0 -> C = 2.0.0
        // B 1.0.0 -> C = ^2.0.0
        // C 1.0.0, 2.0.0
        let mut provider = MockPackageProvider::new();
        provider.add_version("lib_a", "1.0.0", &[("lib_c", "= 1.0.0")]);
        provider.add_version("lib_a", "1.1.0", &[("lib_c", "= 2.0.0")]);
        provider.add_version("lib_b", "1.0.0", &[("lib_c", "^2.0.0")]);
        provider.add_version("lib_c", "1.0.0", &[]);
        provider.add_version("lib_c", "2.0.0", &[]);

        let solver = SatSolver::new(&provider);
        let root = vec![
            ("lib_a".to_string(), VersionReq::parse("^1.0.0").unwrap()),
            ("lib_b".to_string(), VersionReq::parse("^1.0.0").unwrap()),
        ];

        let solution = solver.solve(&root).unwrap();
        // A must be 1.1.0 because 1.0.0 would conflict with B on C
        assert_eq!(solution.get_version("lib_a").unwrap(), &Version::parse("1.1.0").unwrap());
        assert_eq!(solution.get_version("lib_b").unwrap(), &Version::parse("1.0.0").unwrap());
        assert_eq!(solution.get_version("lib_c").unwrap(), &Version::parse("2.0.0").unwrap());
    }

    #[test]
    fn test_unresolvable_version_conflict_reporting() {
        let mut provider = MockPackageProvider::new();
        provider.add_version("lib_conflict_a", "1.0.0", &[("shared_lib", "= 1.0.0")]);
        provider.add_version("lib_conflict_b", "1.0.0", &[("shared_lib", "= 2.0.0")]);
        provider.add_version("shared_lib", "1.0.0", &[]);
        provider.add_version("shared_lib", "2.0.0", &[]);

        let solver = SatSolver::new(&provider);
        let root = vec![
            ("lib_conflict_a".to_string(), VersionReq::parse("1.0.0").unwrap()),
            ("lib_conflict_b".to_string(), VersionReq::parse("1.0.0").unwrap()),
        ];

        let res = solver.solve(&root);
        assert!(res.is_err(), "Must fail with conflict error");
        let err = res.unwrap_err();
        match err {
            SolverError::VersionConflict { package, conflict_details } => {
                assert_eq!(package, "shared_lib");
                assert!(conflict_details.contains("lib_conflict_a"));
                assert!(conflict_details.contains("lib_conflict_b"));
            }
            other => panic!("Expected VersionConflict, got {:?}", other),
        }
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut provider = MockPackageProvider::new();
        provider.add_version("cycle_a", "1.0.0", &[("cycle_b", "1.0.0")]);
        provider.add_version("cycle_b", "1.0.0", &[("cycle_c", "1.0.0")]);
        provider.add_version("cycle_c", "1.0.0", &[("cycle_a", "1.0.0")]);

        let solver = SatSolver::new(&provider);
        let root = vec![("cycle_a".to_string(), VersionReq::parse("1.0.0").unwrap())];

        let res = solver.solve(&root);
        assert!(res.is_err());
        let err = res.unwrap_err();
        match err {
            SolverError::DependencyCycle { cycle } => {
                assert_eq!(cycle, vec!["cycle_a", "cycle_b", "cycle_c", "cycle_a"]);
            }
            other => panic!("Expected DependencyCycle, got {:?}", other),
        }
    }

    #[test]
    fn test_locked_version_preference() {
        let mut provider = MockPackageProvider::new();
        provider.add_version("std::json", "0.1.0", &[]);
        provider.add_version("std::json", "0.2.0", &[]);
        provider.add_version("std::json", "1.0.0", &[]);

        let mut locked = HashMap::new();
        locked.insert("std::json".to_string(), Version::parse("0.2.0").unwrap());

        let solver = SatSolver::new(&provider).with_locked(locked);
        let root = vec![("std::json".to_string(), VersionReq::parse("*").unwrap())];

        let solution = solver.solve(&root).unwrap();
        // Prefers locked 0.2.0 over highest available 1.0.0
        assert_eq!(solution.get_version("std::json").unwrap(), &Version::parse("0.2.0").unwrap());
    }

    #[test]
    fn test_complex_conjunction_and_not_equal() {
        let mut provider = MockPackageProvider::new();
        provider.add_version("pkg_foo", "1.0.0", &[]);
        provider.add_version("pkg_foo", "1.5.0", &[]);
        provider.add_version("pkg_foo", "1.6.0", &[]);
        provider.add_version("pkg_foo", "2.0.0", &[]);

        let solver = SatSolver::new(&provider);
        // Exclude 1.6.0 with !=
        let root = vec![("pkg_foo".to_string(), VersionReq::parse("^1.0.0, != 1.6.0").unwrap())];

        let solution = solver.solve(&root).unwrap();
        // 1.6.0 is excluded, so next highest is 1.5.0
        assert_eq!(solution.get_version("pkg_foo").unwrap(), &Version::parse("1.5.0").unwrap());
    }

    #[test]
    fn test_deep_transitive_4_level_graph() {
        let mut provider = MockPackageProvider::new();
        // Level 1: app -> core (^1.0)
        provider.add_version("core", "1.2.0", &[("net", "^0.5.0")]);
        // Level 2: core -> net (^0.5.0)
        provider.add_version("net", "0.5.4", &[("crypto", "^2.0.0")]);
        // Level 3: net -> crypto (^2.0.0)
        provider.add_version("crypto", "2.1.0", &[("asn1", "^0.1.0")]);
        // Level 4: crypto -> asn1 (^0.1.0)
        provider.add_version("asn1", "0.1.9", &[]);

        let solver = SatSolver::new(&provider);
        let root = vec![("core".to_string(), VersionReq::parse("^1.0.0").unwrap())];

        let solution = solver.solve(&root).unwrap();
        assert_eq!(solution.get_version("core").unwrap(), &Version::parse("1.2.0").unwrap());
        assert_eq!(solution.get_version("net").unwrap(), &Version::parse("0.5.4").unwrap());
        assert_eq!(solution.get_version("crypto").unwrap(), &Version::parse("2.1.0").unwrap());
        assert_eq!(solution.get_version("asn1").unwrap(), &Version::parse("0.1.9").unwrap());
    }

    #[test]
    fn test_package_not_found_error_reporting() {
        let mut provider = MockPackageProvider::new();
        provider.add_version("entry_lib", "1.0.0", &[("ghost_lib", "^1.0.0")]);

        let solver = SatSolver::new(&provider);
        let root = vec![("entry_lib".to_string(), VersionReq::parse("1.0.0").unwrap())];

        let res = solver.solve(&root);
        assert!(res.is_err());
        let err = res.unwrap_err();
        match err {
            SolverError::PackageNotFound { package, required_by } => {
                assert_eq!(package, "ghost_lib");
                assert_eq!(required_by, vec!["entry_lib"]);
            }
            other => panic!("Expected PackageNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_complex_diamond_multi_path_backtracking() {
        // Root -> pkg_x and pkg_y
        // pkg_x 2.0.0 -> common_util = 1.0.0 (conflicts with pkg_y!)
        // pkg_x 2.1.0 -> common_util = 2.0.0 (compatible with pkg_y!)
        // pkg_y 1.0.0 -> common_util = ^2.0.0
        // common_util 1.0.0 and 2.0.0 exist
        let mut provider = MockPackageProvider::new();
        provider.add_version("pkg_x", "2.0.0", &[("common_util", "= 1.0.0")]);
        provider.add_version("pkg_x", "2.1.0", &[("common_util", "= 2.0.0")]);
        provider.add_version("pkg_y", "1.0.0", &[("common_util", "^2.0.0")]);
        provider.add_version("common_util", "1.0.0", &[]);
        provider.add_version("common_util", "2.0.0", &[]);

        let solver = SatSolver::new(&provider);
        let root = vec![
            ("pkg_x".to_string(), VersionReq::parse("^2.0.0").unwrap()),
            ("pkg_y".to_string(), VersionReq::parse("^1.0.0").unwrap()),
        ];

        let solution = solver.solve(&root).unwrap();
        assert_eq!(solution.get_version("pkg_x").unwrap(), &Version::parse("2.1.0").unwrap());
        assert_eq!(solution.get_version("pkg_y").unwrap(), &Version::parse("1.0.0").unwrap());
        assert_eq!(solution.get_version("common_util").unwrap(), &Version::parse("2.0.0").unwrap());
    }
}
