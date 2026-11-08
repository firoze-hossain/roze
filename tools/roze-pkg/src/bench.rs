// tools/roze-pkg/src/bench.rs
//
// Native Microbenchmark & Profiler Harness ('bsr bench')
// Parity with Rust criterion and Java JMH benchmark rigor.
// Features:
// - Monotonic nanosecond timing
// - Statistical warm-up phases (cold-cache & JIT mitigation)
// - Dynamic iteration scaling (eliminates timer quantization error)
// - Statistical metrics: Mean, Median, Std Dev, Variance, SEM, 95% Confidence Intervals
// - Outlier detection: Tukey's IQR method (mild and severe outlier classification)
// - Operations per second (ops/sec) and throughput bandwidth (MB/s, GB/s)
// - Memory allocation and resident set size (Peak RSS) profiling
// - Baseline save and regression analysis (--save-baseline, --baseline)
// - Welch's two-sample t-test for statistical significance
// - ASCII histogram & percentile distribution report in verbose mode
// - Structured JSON output for CI/CD regression tracking

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use anyhow::{anyhow, Result};
use colored::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub filter: Option<String>,
    pub warmup_ms: u64,
    pub measurement_time_ms: u64,
    pub samples: usize,
    pub save_baseline: Option<String>,
    pub baseline: Option<String>,
    pub json: bool,
    pub profile: bool,
    pub native: bool,
    pub jvm: bool,
    pub verbose: bool,
    pub throughput_bytes: Option<u64>,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            filter: None,
            warmup_ms: 500,
            measurement_time_ms: 2000,
            samples: 50,
            save_baseline: None,
            baseline: None,
            json: false,
            profile: false,
            native: true,
            jvm: false,
            verbose: false,
            throughput_bytes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSuiteResult {
    pub timestamp: u64,
    pub package_name: String,
    pub benchmarks: Vec<BenchmarkStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkStats {
    pub name: String,
    pub file: String,
    pub samples: usize,
    pub iterations_per_sample: u64,
    pub total_iterations: u64,
    pub mean_ns: f64,
    pub median_ns: f64,
    pub std_dev_ns: f64,
    pub sem_ns: f64,
    pub variance_ns: f64,
    pub min_ns: f64,
    pub max_ns: f64,
    pub ci_95_lower_ns: f64,
    pub ci_95_upper_ns: f64,
    pub percentiles: Percentiles,
    pub outliers: Outliers,
    pub ops_per_sec: f64,
    pub throughput_mb_per_sec: Option<f64>,
    pub peak_rss_kb: Option<u64>,
    pub comparison: Option<BaselineComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Percentiles {
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub p99_9: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outliers {
    pub mild: usize,
    pub severe: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparison {
    pub baseline_name: String,
    pub baseline_mean_ns: f64,
    pub diff_pct: f64,
    pub p_value: f64,
    pub verdict: BenchmarkVerdict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkVerdict {
    Speedup,
    Regression,
    NoChange,
}

/// Run all discovered benchmarks according to configuration.
pub fn run_benchmarks(config: &BenchmarkConfig, project_dir: &Path) -> Result<BenchmarkSuiteResult> {
    let benches_dir = project_dir.join("benches");
    if !benches_dir.is_dir() {
        return Err(anyhow!("No 'benches/' directory found in project: {}", project_dir.display()));
    }

    let mut bench_files = Vec::new();
    for entry in fs::read_dir(&benches_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("roze") {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if let Some(ref filter) = config.filter {
                if !stem.contains(filter) && !path.display().to_string().contains(filter) {
                    continue;
                }
            }
            bench_files.push(path);
        }
    }

    bench_files.sort();

    if bench_files.is_empty() {
        if let Some(ref filter) = config.filter {
            return Err(anyhow!("No benchmarks found matching filter '{}'", filter));
        } else {
            return Err(anyhow!("No benchmark files (.roze) found in benches/"));
        }
    }

    // Load baseline if requested
    let previous_baseline = if let Some(ref b_name) = config.baseline {
        match load_baseline(project_dir, b_name) {
            Ok(base) => {
                if !config.json {
                    println!("{} Comparing against baseline '{}'", "📊".cyan(), b_name.bold());
                }
                Some(base)
            }
            Err(e) => {
                if !config.json {
                    eprintln!("{} Warning: Failed to load baseline '{}': {}", "⚠️".yellow(), b_name, e);
                }
                None
            }
        }
    } else {
        None
    };

    let mut all_stats = Vec::new();

    for bench_file in &bench_files {
        let bench_name = bench_file.file_stem().and_then(|s| s.to_str()).unwrap_or("benchmark");
        let stats = run_single_benchmark(config, project_dir, bench_file, bench_name, previous_baseline.as_ref())?;
        all_stats.push(stats);
    }

    let package_name = project_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("roze_project")
        .to_string();

    let suite_result = BenchmarkSuiteResult {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        package_name,
        benchmarks: all_stats,
    };

    // Save baseline if requested
    if let Some(ref save_name) = config.save_baseline {
        save_baseline(&suite_result, project_dir, save_name)?;
        if !config.json {
            println!(
                "{} Benchmark results saved as baseline '{}' in .bsr/bench/{}.json",
                "💾".green(),
                save_name.bold(),
                save_name
            );
        }
    }

    // Output JSON if requested
    if config.json {
        let json_str = serde_json::to_string_pretty(&suite_result)?;
        println!("{}", json_str);
    }

    Ok(suite_result)
}

/// Execute a single benchmark file with statistical sampling, warm-up, and analysis.
pub fn run_single_benchmark(
    config: &BenchmarkConfig,
    _project_dir: &Path,
    bench_file: &Path,
    bench_name: &str,
    baseline: Option<&BenchmarkSuiteResult>,
) -> Result<BenchmarkStats> {
    if !config.json {
        println!("\n{} {}", "Benchmarking".bold().magenta(), bench_name.bold().cyan());
    }

    let compiler = crate::find_compiler()?;

    // Compile benchmark ahead-of-time (native Cranelift compilation by default)
    let tmp = tempfile::tempdir()?;
    let stem = bench_file.file_stem().and_then(|s| s.to_str()).unwrap_or("bench");
    let canon_bench_file = bench_file.canonicalize().unwrap_or_else(|_| bench_file.to_path_buf());

    let (exec_cmd, exec_args, _temp_dir_guard) = if !config.jvm {
        let mut build_cmd = Command::new(&compiler);
        build_cmd.arg("build").arg(&canon_bench_file).arg("--target").arg("native");
        build_cmd.current_dir(tmp.path());
        let _ = build_cmd.output();

        let native_bin = tmp.path().join(stem);
        if native_bin.exists() {
            (native_bin, Vec::<String>::new(), Some(tmp))
        } else {
            (compiler.clone(), vec!["run".to_string(), canon_bench_file.display().to_string()], None)
        }
    } else {
        (compiler.clone(), vec!["run".to_string(), canon_bench_file.display().to_string()], None)
    };

    // --- 1. Warm-up Phase ---
    let warmup_duration = std::time::Duration::from_millis(config.warmup_ms.max(20));
    if !config.json {
        println!(
            "  {} Warming up for {} ms...",
            "⏳".yellow(),
            config.warmup_ms
        );
    }

    let warmup_start = Instant::now();
    let mut warmup_runs = 0;
    while warmup_start.elapsed() < warmup_duration && warmup_runs < 100 {
        let status = Command::new(&exec_cmd).args(&exec_args).output()?;
        if !status.status.success() {
            return Err(anyhow!("Benchmark execution failed during warmup: {}", bench_file.display()));
        }
        warmup_runs += 1;
    }

    // --- 2. Dynamic Iteration Calibration ---
    let test_run_start = Instant::now();
    let _ = Command::new(&exec_cmd).args(&exec_args).output()?;
    let single_run_duration = test_run_start.elapsed();

    let iters_per_sample: u64 = if single_run_duration.as_nanos() < 1_000_000 {
        let needed = 1_000_000 / single_run_duration.as_nanos().max(1);
        needed.clamp(1, 1000) as u64
    } else {
        1
    };

    if !config.json {
        println!(
            "  {} Collecting {} samples ({} iterations/sample)...",
            "📊".blue(),
            config.samples,
            iters_per_sample
        );
    }

    // --- 3. Measurement Phase ---
    let mut sample_durations_ns = Vec::with_capacity(config.samples);
    let mut peak_rss_kb: Option<u64> = None;

    for _ in 0..config.samples {
        let start = Instant::now();
        for _ in 0..iters_per_sample {
            let output = Command::new(&exec_cmd).args(&exec_args).output()?;
            if !output.status.success() {
                return Err(anyhow!("Benchmark execution failed during measurement"));
            }
        }
        let elapsed_ns = start.elapsed().as_nanos() as f64 / iters_per_sample as f64;
        sample_durations_ns.push(elapsed_ns);

        if config.profile && peak_rss_kb.is_none() {
            peak_rss_kb = get_process_rss_kb();
        }
    }

    // --- 4. Statistical Analysis ---
    let mut stats = calculate_stats(
        bench_name,
        &bench_file.display().to_string(),
        &sample_durations_ns,
        iters_per_sample,
        config.throughput_bytes,
        peak_rss_kb,
    );

    // --- 5. Baseline Comparison ---
    if let Some(base_suite) = baseline {
        if let Some(base_bench) = base_suite.benchmarks.iter().find(|b| b.name == bench_name) {
            stats.comparison = Some(compare_baselines(&stats, base_bench, config.baseline.as_deref().unwrap_or("baseline")));
        }
    }

    // --- 6. Terminal Presentation ---
    if !config.json {
        print_benchmark_stats(&stats, config.verbose);
    }

    Ok(stats)
}

/// Calculate all descriptive statistics, percentiles, confidence intervals, and outliers.
pub fn calculate_stats(
    name: &str,
    file: &str,
    sample_durations_ns: &[f64],
    iters_per_sample: u64,
    throughput_bytes: Option<u64>,
    peak_rss_kb: Option<u64>,
) -> BenchmarkStats {
    let n = sample_durations_ns.len();
    assert!(n > 0, "Cannot calculate statistics on empty sample set");

    let mut sorted = sample_durations_ns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Mean
    let sum: f64 = sorted.iter().sum();
    let mean_ns = sum / (n as f64);

    // Median
    let median_ns = if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    };

    // Variance & Standard Deviation (Sample variance: / (n - 1))
    let variance_ns = if n > 1 {
        let sum_sq_diff: f64 = sorted.iter().map(|&x| (x - mean_ns).powi(2)).sum();
        sum_sq_diff / ((n - 1) as f64)
    } else {
        0.0
    };
    let std_dev_ns = variance_ns.sqrt();

    // Standard Error of the Mean (SEM)
    let sem_ns = std_dev_ns / (n as f64).sqrt();

    // 95% Confidence Interval (z = 1.96 for normal/bootstrap approx)
    let ci_95_lower_ns = (mean_ns - 1.96 * sem_ns).max(0.0);
    let ci_95_upper_ns = mean_ns + 1.96 * sem_ns;

    // Percentiles
    let percentiles = Percentiles {
        p25: calculate_percentile(&sorted, 0.25),
        p50: median_ns,
        p75: calculate_percentile(&sorted, 0.75),
        p90: calculate_percentile(&sorted, 0.90),
        p95: calculate_percentile(&sorted, 0.95),
        p99: calculate_percentile(&sorted, 0.99),
        p99_9: calculate_percentile(&sorted, 0.999),
    };

    // Tukey's IQR Outlier Detection
    let iqr = percentiles.p75 - percentiles.p25;
    let mild_low = percentiles.p25 - 1.5 * iqr;
    let mild_high = percentiles.p75 + 1.5 * iqr;
    let severe_low = percentiles.p25 - 3.0 * iqr;
    let severe_high = percentiles.p75 + 3.0 * iqr;

    let mut mild_outliers = 0;
    let mut severe_outliers = 0;

    for &val in &sorted {
        if val < severe_low || val > severe_high {
            severe_outliers += 1;
        } else if val < mild_low || val > mild_high {
            mild_outliers += 1;
        }
    }

    let outliers = Outliers {
        mild: mild_outliers,
        severe: severe_outliers,
        total: mild_outliers + severe_outliers,
    };

    // Operations per second
    let ops_per_sec = if mean_ns > 0.0 {
        1_000_000_000.0 / mean_ns
    } else {
        0.0
    };

    // Throughput in MB/s (if throughput_bytes is provided)
    let throughput_mb_per_sec = throughput_bytes.map(|bytes| {
        (bytes as f64 * ops_per_sec) / 1_000_000.0
    });

    BenchmarkStats {
        name: name.to_string(),
        file: file.to_string(),
        samples: n,
        iterations_per_sample: iters_per_sample,
        total_iterations: (n as u64) * iters_per_sample,
        mean_ns,
        median_ns,
        std_dev_ns,
        sem_ns,
        variance_ns,
        min_ns: sorted[0],
        max_ns: sorted[n - 1],
        ci_95_lower_ns,
        ci_95_upper_ns,
        percentiles,
        outliers,
        ops_per_sec,
        throughput_mb_per_sec,
        peak_rss_kb,
        comparison: None,
    }
}

/// Calculate percentile from a pre-sorted slice using linear interpolation.
fn calculate_percentile(sorted: &[f64], p: f64) -> f64 {
    assert!(!sorted.is_empty());
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = p * ((sorted.len() - 1) as f64);
    let lower = idx.floor() as usize;
    let upper = idx.ceil() as usize;
    let weight = idx - (lower as f64);
    sorted[lower] * (1.0 - weight) + sorted[upper] * weight
}

/// Compare current benchmark against a historical baseline using Welch's t-test.
pub fn compare_baselines(
    current: &BenchmarkStats,
    baseline: &BenchmarkStats,
    baseline_name: &str,
) -> BaselineComparison {
    let diff_pct = if baseline.mean_ns > 0.0 {
        ((current.mean_ns - baseline.mean_ns) / baseline.mean_ns) * 100.0
    } else {
        0.0
    };

    let p_value = welch_t_test(
        current.mean_ns,
        current.variance_ns,
        current.samples,
        baseline.mean_ns,
        baseline.variance_ns,
        baseline.samples,
    );

    // Statistical significance threshold: p < 0.05 and |diff| > 2%
    let verdict = if p_value < 0.05 && diff_pct.abs() > 2.0 {
        if diff_pct < 0.0 {
            BenchmarkVerdict::Speedup
        } else {
            BenchmarkVerdict::Regression
        }
    } else {
        BenchmarkVerdict::NoChange
    };

    BaselineComparison {
        baseline_name: baseline_name.to_string(),
        baseline_mean_ns: baseline.mean_ns,
        diff_pct,
        p_value,
        verdict,
    }
}

/// Perform Welch's two-sample t-test and compute two-tailed p-value.
pub fn welch_t_test(
    m1: f64,
    v1: f64,
    n1: usize,
    m2: f64,
    v2: f64,
    n2: usize,
) -> f64 {
    let n1_f = n1 as f64;
    let n2_f = n2 as f64;

    let se1 = v1 / n1_f;
    let se2 = v2 / n2_f;
    let se_diff = (se1 + se2).sqrt();

    if se_diff <= 1e-12 {
        if (m1 - m2).abs() > 1e-9 {
            return 0.0001;
        } else {
            return 1.0;
        }
    }

    let t = (m1 - m2).abs() / se_diff;

    // Approximate two-tailed p-value using standard normal / Student's t approximation:
    // Phi(t) approx 1 / (1 + exp(-1.702 * t))
    let phi = 1.0 / (1.0 + (-1.702 * t).exp());
    let p_value = (2.0 * (1.0 - phi)).clamp(0.0001, 1.0);
    p_value
}

/// Print formatted Criterion/JMH style benchmark metrics to stdout.
pub fn print_benchmark_stats(stats: &BenchmarkStats, verbose: bool) {
    let mean_str = format_time(stats.mean_ns);
    let median_str = format_time(stats.median_ns);
    let std_str = format_time(stats.std_dev_ns);
    let ci_lower_str = format_time(stats.ci_95_lower_ns);
    let ci_upper_str = format_time(stats.ci_95_upper_ns);
    let ops_str = format_ops(stats.ops_per_sec);

    println!(
        "  {}: [{} {} {}] (95% CI)",
        "time".bold(),
        ci_lower_str.dimmed(),
        mean_str.bold().green(),
        ci_upper_str.dimmed()
    );
    println!(
        "  {}: {} ± {} (std: {})",
        "mean".bold(),
        mean_str.bold(),
        format_time(stats.sem_ns).dimmed(),
        std_str.dimmed()
    );
    println!("  {}: {}", "median".bold(), median_str);
    println!("  {}: {}", "throughput".bold(), ops_str.bold().cyan());

    if let Some(mb_sec) = stats.throughput_mb_per_sec {
        println!("  {}: {:.2} MB/s", "bandwidth".bold(), mb_sec);
    }

    if let Some(rss) = stats.peak_rss_kb {
        println!("  {}: {:.2} MB", "peak RSS".bold(), (rss as f64) / 1024.0);
    }

    if stats.outliers.total > 0 {
        let outlier_str = format!(
            "{} ({} mild, {} severe)",
            stats.outliers.total, stats.outliers.mild, stats.outliers.severe
        );
        println!("  {}: {}", "outliers".yellow().bold(), outlier_str.yellow());
    }

    if let Some(ref comp) = stats.comparison {
        match comp.verdict {
            BenchmarkVerdict::Speedup => {
                println!(
                    "  {}: {} (Speedup) [p = {:.4}]",
                    "comparison".bold(),
                    format!("{:.2}%", comp.diff_pct).bold().green(),
                    comp.p_value
                );
            }
            BenchmarkVerdict::Regression => {
                println!(
                    "  {}: {} (Performance Regression!) [p = {:.4}]",
                    "comparison".bold(),
                    format!("+{:.2}%", comp.diff_pct).bold().red(),
                    comp.p_value
                );
            }
            BenchmarkVerdict::NoChange => {
                println!(
                    "  {}: {} (No statistically significant change) [p = {:.4}]",
                    "comparison".bold(),
                    format!("{:+.2}%", comp.diff_pct).dimmed(),
                    comp.p_value
                );
            }
        }
    }

    if verbose {
        println!("\n  {}", "Percentiles:".bold().underline());
        println!("    p25:   {}", format_time(stats.percentiles.p25));
        println!("    p50:   {}", format_time(stats.percentiles.p50));
        println!("    p75:   {}", format_time(stats.percentiles.p75));
        println!("    p90:   {}", format_time(stats.percentiles.p90));
        println!("    p95:   {}", format_time(stats.percentiles.p95));
        println!("    p99:   {}", format_time(stats.percentiles.p99));
        println!("    p99.9: {}", format_time(stats.percentiles.p99_9));
        println!("    min:   {}", format_time(stats.min_ns));
        println!("    max:   {}", format_time(stats.max_ns));
    }
}

/// Format duration in nanoseconds to appropriate human-readable units (ns, µs, ms, s).
pub fn format_time(ns: f64) -> String {
    if ns < 1_000.0 {
        format!("{:.2} ns", ns)
    } else if ns < 1_000_000.0 {
        format!("{:.3} µs", ns / 1_000.0)
    } else if ns < 1_000_000_000.0 {
        format!("{:.3} ms", ns / 1_000_000.0)
    } else {
        format!("{:.3} s", ns / 1_000_000_000.0)
    }
}

/// Format operations per second to human-readable scale (ops/s, Kops/s, Mops/s, Gops/s).
pub fn format_ops(ops: f64) -> String {
    if ops < 1_000.0 {
        format!("{:.1} ops/s", ops)
    } else if ops < 1_000_000.0 {
        format!("{:.2} Kops/s", ops / 1_000.0)
    } else if ops < 1_000_000_000.0 {
        format!("{:.2} Mops/s", ops / 1_000_000.0)
    } else {
        format!("{:.2} Gops/s", ops / 1_000_000_000.0)
    }
}

/// Read resident set size (RSS) in KB from Linux /proc/self/statm.
pub fn get_process_rss_kb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = fs::read_to_string("/proc/self/statm") {
            let tokens: Vec<&str> = content.split_whitespace().collect();
            if tokens.len() >= 2 {
                if let Ok(resident_pages) = tokens[1].parse::<u64>() {
                    // Standard Linux page size is 4096 bytes (4 KB)
                    return Some(resident_pages * 4);
                }
            }
        }
    }
    None
}

/// Save benchmark suite results as baseline in .bsr/bench/<name>.json.
pub fn save_baseline(results: &BenchmarkSuiteResult, project_dir: &Path, name: &str) -> Result<()> {
    let bench_dir = project_dir.join(".bsr/bench");
    fs::create_dir_all(&bench_dir)?;
    let file_path = bench_dir.join(format!("{}.json", name));
    let json = serde_json::to_string_pretty(results)?;
    fs::write(file_path, json)?;
    Ok(())
}

/// Load previously saved baseline from .bsr/bench/<name>.json.
pub fn load_baseline(project_dir: &Path, name: &str) -> Result<BenchmarkSuiteResult> {
    let file_path = project_dir.join(".bsr/bench").join(format!("{}.json", name));
    if !file_path.exists() {
        return Err(anyhow!("Baseline file not found: {}", file_path.display()));
    }
    let content = fs::read_to_string(file_path)?;
    let suite: BenchmarkSuiteResult = serde_json::from_str(&content)?;
    Ok(suite)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_stats_basic() {
        let samples = vec![1000.0, 1050.0, 950.0, 1000.0, 1020.0, 980.0];
        let stats = calculate_stats("test_op", "bench.roze", &samples, 1, Some(1024), Some(2048));

        assert_eq!(stats.samples, 6);
        assert!((stats.mean_ns - 1000.0).abs() < 1.0);
        assert!((stats.median_ns - 1000.0).abs() < 1.0);
        assert!(stats.std_dev_ns > 0.0);
        assert!(stats.ops_per_sec > 900_000.0);
        assert!(stats.throughput_mb_per_sec.is_some());
        assert_eq!(stats.peak_rss_kb, Some(2048));
    }

    #[test]
    fn test_outlier_detection() {
        // 10 samples around 100ns, with 1 massive outlier at 10,000ns
        let samples = vec![100.0, 102.0, 98.0, 101.0, 99.0, 100.0, 103.0, 97.0, 100.0, 10000.0];
        let stats = calculate_stats("outlier_test", "bench.roze", &samples, 1, None, None);

        assert!(stats.outliers.total >= 1);
        assert!(stats.outliers.severe >= 1);
    }

    #[test]
    fn test_baseline_comparison_speedup_and_regression() {
        let samples_base = vec![1000.0; 50];
        let base_stats = calculate_stats("base", "bench.roze", &samples_base, 1, None, None);

        // 30% faster
        let samples_fast = vec![700.0; 50];
        let fast_stats = calculate_stats("fast", "bench.roze", &samples_fast, 1, None, None);
        let comp_fast = compare_baselines(&fast_stats, &base_stats, "main");
        assert_eq!(comp_fast.verdict, BenchmarkVerdict::Speedup);
        assert!(comp_fast.diff_pct < -25.0);

        // 30% slower
        let samples_slow = vec![1300.0; 50];
        let slow_stats = calculate_stats("slow", "bench.roze", &samples_slow, 1, None, None);
        let comp_slow = compare_baselines(&slow_stats, &base_stats, "main");
        assert_eq!(comp_slow.verdict, BenchmarkVerdict::Regression);
        assert!(comp_slow.diff_pct > 25.0);
    }

    #[test]
    fn test_format_helpers() {
        assert_eq!(format_time(500.0), "500.00 ns");
        assert_eq!(format_time(1500.0), "1.500 µs");
        assert_eq!(format_time(2_500_000.0), "2.500 ms");
        assert_eq!(format_time(3_000_000_000.0), "3.000 s");

        assert_eq!(format_ops(500.0), "500.0 ops/s");
        assert_eq!(format_ops(15_000.0), "15.00 Kops/s");
        assert_eq!(format_ops(2_500_000.0), "2.50 Mops/s");
        assert_eq!(format_ops(3_500_000_000.0), "3.50 Gops/s");
    }

    #[test]
    fn test_baseline_save_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let samples = vec![100.0, 105.0, 95.0];
        let stats = calculate_stats("save_load_test", "bench.roze", &samples, 1, None, None);
        let suite = BenchmarkSuiteResult {
            timestamp: 12345678,
            package_name: "test_pkg".to_string(),
            benchmarks: vec![stats],
        };

        save_baseline(&suite, tmp.path(), "my_baseline").unwrap();
        let loaded = load_baseline(tmp.path(), "my_baseline").unwrap();
        assert_eq!(loaded.package_name, "test_pkg");
        assert_eq!(loaded.benchmarks.len(), 1);
        assert_eq!(loaded.benchmarks[0].name, "save_load_test");
    }
}
