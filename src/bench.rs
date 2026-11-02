use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::warn;

use crate::bridge::{BridgeEvent, CloudBridge};
use crate::policy;
use crate::tools;

pub const OFFICIAL_DEEPSEEK_V41_FLASH_BASE: f64 = 57.0;
pub const OFFICIAL_DEEPSEEK_V41_FLASH_1017: f64 = 71.6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchTask {
    pub id: String,
    pub lang: String,
    pub title: String,
    pub prompt: String,
    pub files: HashMap<String, String>,
    pub verify_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub lang: String,
    pub passed: bool,
    pub duration_ms: u128,
    pub thought_steps: usize,
    pub total_tool_calls: usize,
    pub ast_tool_calls: usize,
    pub file_tool_calls: usize,
    pub verification_stdout: String,
    pub verification_stderr: String,
    pub error_message: Option<String>,
    pub log_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    pub total_tasks: usize,
    pub passed_tasks: usize,
    pub pass_rate: f64,
    pub official_baseline_score: f64,
    pub score_delta: f64,
    pub total_duration_ms: u128,
    pub lang_stats: HashMap<String, LangStat>,
    pub task_results: Vec<TaskResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LangStat {
    pub total: usize,
    pub passed: usize,
    pub pass_rate: f64,
    pub avg_duration_ms: u128,
    pub total_ast_tools: usize,
    pub total_file_tools: usize,
}

pub struct BenchOptions {
    pub dataset_path: Option<String>,
    pub langs: Vec<String>,
    pub limit: Option<usize>,
    pub output_path: String,
    pub log_dir: String,
    pub hub_url: Option<String>,
    pub profile: Option<String>,
    pub dry_run: bool,
    pub timeout_secs: u64,
}

/// Generates the curated built-in polyglot benchmark suite (Rust, Go, Python, JavaScript)
pub fn get_builtin_tasks() -> Vec<BenchTask> {
    vec![
        // 1. Rust: LRU Cache
        BenchTask {
            id: "rust_lru_cache".to_string(),
            lang: "rust".to_string(),
            title: "Implement LRU Cache in Rust".to_string(),
            prompt: "Implement an LRUCache struct in src/lib.rs with new(capacity: usize), get(&mut self, key: i32) -> Option<i32>, and put(&mut self, key: i32, value: i32). All tests in `cargo test` must pass.".to_string(),
            files: [
                ("Cargo.toml".to_string(), r#"[package]
name = "lru_cache"
version = "0.1.0"
edition = "2021"
"#.to_string()),
                ("src/lib.rs".to_string(), r#"use std::collections::HashMap;

pub struct LRUCache {
    // TODO: Complete implementation
    capacity: usize,
    map: HashMap<i32, i32>,
}

impl LRUCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::new(),
        }
    }

    pub fn get(&mut self, _key: i32) -> Option<i32> {
        None
    }

    pub fn put(&mut self, _key: i32, _value: i32) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_put_get() {
        let mut cache = LRUCache::new(2);
        cache.put(1, 10);
        cache.put(2, 20);
        assert_eq!(cache.get(1), Some(10));
        assert_eq!(cache.get(2), Some(20));
        assert_eq!(cache.get(3), None);
    }

    #[test]
    fn test_eviction() {
        let mut cache = LRUCache::new(2);
        cache.put(1, 10);
        cache.put(2, 20);
        cache.get(1); // 1 accessed, 2 is LRU
        cache.put(3, 30); // should evict 2
        assert_eq!(cache.get(2), None);
        assert_eq!(cache.get(1), Some(10));
        assert_eq!(cache.get(3), Some(30));
    }
}
"#.to_string()),
            ].into_iter().collect(),
            verify_command: "cargo test".to_string(),
        },

        // 2. Go: String Reverse & Palindrome
        BenchTask {
            id: "go_palindrome".to_string(),
            lang: "go".to_string(),
            title: "Implement Clean Palindrome Checker in Go".to_string(),
            prompt: "Implement IsPalindrome(s string) bool in palindrome.go. Ignore non-alphanumeric characters and case. Ensure all tests in `go test` pass.".to_string(),
            files: [
                ("go.mod".to_string(), "module palindrome\n\ngo 1.21\n".to_string()),
                ("palindrome.go".to_string(), r#"package palindrome

func IsPalindrome(s string) bool {
    // TODO: implement
    return false
}
"#.to_string()),
                ("palindrome_test.go".to_string(), r#"package palindrome

import "testing"

func TestIsPalindrome(t *testing.T) {
    cases := []struct {
        input    string
        expected bool
    }{
        {"A man, a plan, a canal: Panama", true},
        {"race a car", false},
        {" ", true},
        {"0P", false},
    }

    for _, c := range cases {
        actual := IsPalindrome(c.input)
        if actual != c.expected {
            t.Errorf("IsPalindrome(%q) == %v, expected %v", c.input, actual, c.expected)
        }
    }
}
"#.to_string()),
            ].into_iter().collect(),
            verify_command: "go test".to_string(),
        },

        // 3. Python: Vector Operations
        BenchTask {
            id: "py_vector_math".to_string(),
            lang: "python".to_string(),
            title: "Implement 2D Vector Operations in Python".to_string(),
            prompt: "Implement Vector2D class in vector.py with dot product, magnitude, and addition. Make sure `python3 -m unittest test_vector.py` passes.".to_string(),
            files: [
                ("vector.py".to_string(), r#"import math

class Vector2D:
    def __init__(self, x: float, y: float):
        self.x = x
        self.y = y

    def add(self, other: "Vector2D") -> "Vector2D":
        # TODO: Implement
        return Vector2D(0, 0)

    def dot(self, other: "Vector2D") -> float:
        # TODO: Implement
        return 0.0

    def magnitude(self) -> float:
        # TODO: Implement
        return 0.0
"#.to_string()),
                ("test_vector.py".to_string(), r#"import unittest
from vector import Vector2D

class TestVector2D(unittest.TestCase):
    def test_add(self):
        v1 = Vector2D(1, 2)
        v2 = Vector2D(3, 4)
        v3 = v1.add(v2)
        self.assertEqual(v3.x, 4)
        self.assertEqual(v3.y, 6)

    def test_dot(self):
        v1 = Vector2D(2, 3)
        v2 = Vector2D(4, 5)
        self.assertEqual(v1.dot(v2), 23)

    def test_magnitude(self):
        v = Vector2D(3, 4)
        self.assertAlmostEqual(v.magnitude(), 5.0)

if __name__ == '__main__':
    unittest.main()
"#.to_string()),
            ].into_iter().collect(),
            verify_command: "python3 -m unittest test_vector.py".to_string(),
        },

        // 4. JavaScript: Event Emitter
        BenchTask {
            id: "js_event_emitter".to_string(),
            lang: "javascript".to_string(),
            title: "Implement EventEmitter in JavaScript".to_string(),
            prompt: "Implement EventEmitter class in emitter.js with on(event, listener), emit(event, ...args), and off(event, listener). Make sure node test runner passes.".to_string(),
            files: [
                ("emitter.js".to_string(), r#"class EventEmitter {
  constructor() {
    this.events = {};
  }

  on(event, listener) {
    // TODO: implement
  }

  emit(event, ...args) {
    // TODO: implement
  }

  off(event, listener) {
    // TODO: implement
  }
}

module.exports = { EventEmitter };
"#.to_string()),
                ("test.js".to_string(), r#"const assert = require('assert');
const test = require('node:test');
const { EventEmitter } = require('./emitter');

test('EventEmitter basic emit and listen', () => {
  const em = new EventEmitter();
  let val = 0;
  em.on('inc', (n) => { val += n; });
  em.emit('inc', 5);
  assert.strictEqual(val, 5);
});

test('EventEmitter off listener', () => {
  const em = new EventEmitter();
  let val = 0;
  const fn = (n) => { val += n; };
  em.on('inc', fn);
  em.emit('inc', 2);
  em.off('inc', fn);
  em.emit('inc', 2);
  assert.strictEqual(val, 2);
});
"#.to_string()),
            ].into_iter().collect(),
            verify_command: "node test.js".to_string(),
        },
    ]
}

/// Loads tasks from an external dataset path or returns builtins
pub fn load_tasks(dataset_path: Option<&str>, allowed_langs: &[String], limit: Option<usize>) -> Vec<BenchTask> {
    let mut tasks = if let Some(path_str) = dataset_path {
        let p = Path::new(path_str);
        if p.is_dir() {
            load_tasks_from_dir(p)
        } else if p.is_file() {
            load_tasks_from_file(p)
        } else {
            warn!("Dataset path '{}' not found, falling back to built-in tasks", path_str);
            get_builtin_tasks()
        }
    } else {
        get_builtin_tasks()
    };

    if !allowed_langs.is_empty() {
        tasks.retain(|t| allowed_langs.iter().any(|l| l.eq_ignore_ascii_case(&t.lang)));
    }

    if let Some(lim) = limit {
        tasks.truncate(lim);
    }

    tasks
}

fn load_tasks_from_dir(dir: &Path) -> Vec<BenchTask> {
    let mut tasks = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let task_dir = entry.path();
            if task_dir.is_dir() {
                let task_json_file = task_dir.join("task.json");
                if task_json_file.exists() {
                    if let Ok(content) = fs::read_to_string(&task_json_file) {
                        if let Ok(task) = serde_json::from_str::<BenchTask>(&content) {
                            tasks.push(task);
                            continue;
                        }
                    }
                }
            }
        }
    }
    if tasks.is_empty() {
        get_builtin_tasks()
    } else {
        tasks
    }
}

fn load_tasks_from_file(file: &Path) -> Vec<BenchTask> {
    let mut tasks = Vec::new();
    if let Ok(content) = fs::read_to_string(file) {
        if let Ok(task_vec) = serde_json::from_str::<Vec<BenchTask>>(&content) {
            tasks = task_vec;
        } else {
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    if let Ok(task) = serde_json::from_str::<BenchTask>(line) {
                        tasks.push(task);
                    }
                }
            }
        }
    }
    if tasks.is_empty() {
        get_builtin_tasks()
    } else {
        tasks
    }
}

pub async fn run_benchmark(options: BenchOptions) -> anyhow::Result<BenchReport> {
    // Switch to Eval mode for benchmark execution
    policy::init_policy("eval", None);

    let allowed_langs: Vec<String> = options.langs;
    let tasks = load_tasks(options.dataset_path.as_deref(), &allowed_langs, options.limit);
    let total_tasks = tasks.len();

    eprintln!("🎯 [EnginEdge Bench] Starting benchmark evaluation on {} tasks", total_tasks);
    eprintln!("📋 [EnginEdge Bench] Languages: {:?}", allowed_langs);
    eprintln!("🛡️ [EnginEdge Bench] Active Policy: {}\n", policy::get_policy_summary());

    let mut task_results = Vec::new();
    let overall_start = Instant::now();

    let original_cwd = env::current_dir()?;

    let abs_log_dir = if Path::new(&options.log_dir).is_absolute() {
        PathBuf::from(&options.log_dir)
    } else {
        original_cwd.join(&options.log_dir)
    };
    let _ = fs::create_dir_all(&abs_log_dir);

    let abs_output_path = if Path::new(&options.output_path).is_absolute() {
        PathBuf::from(&options.output_path)
    } else {
        original_cwd.join(&options.output_path)
    };

    for (idx, task) in tasks.iter().enumerate() {
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("▶ [{}/{}] Evaluating: {} ({})", idx + 1, total_tasks, task.id, task.lang);
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let task_start = Instant::now();
        let sandbox_dir = env::temp_dir().join(format!("enginedge_bench_{}_{}", task.id, task_start.elapsed().as_millis()));
        fs::create_dir_all(&sandbox_dir)?;

        // Write initial files into sandbox
        for (rel_path, content) in &task.files {
            let full_p = sandbox_dir.join(rel_path);
            if let Some(parent) = full_p.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(full_p, content)?;
        }

        if !options.dry_run {
            // Change current directory to sandbox during active agent runs
            env::set_current_dir(&sandbox_dir)?;
        }

        let mut thought_steps = 0;
        let mut total_tool_calls = 0;
        let mut ast_tool_calls = 0;
        let mut file_tool_calls = 0;
        let mut error_msg = None;

        let mut trace_log = String::new();
        trace_log.push_str("=======================================================\n");
        trace_log.push_str(&format!("TASK ID: {} [{}]\n", task.id, task.lang));
        trace_log.push_str(&format!("TITLE: {}\n", task.title));
        trace_log.push_str(&format!("PROMPT:\n{}\n", task.prompt));
        trace_log.push_str("=======================================================\n\n");

        if !options.dry_run {
            let b = CloudBridge::new(options.hub_url.clone());
            b.start();
            if b.wait_for_connection(Duration::from_secs(15)).await {
                let mut event_rx = b.subscribe_events();

                    let init_payload = json!({
                        "type": "init",
                        "task_name": format!("bench_{}", task.id),
                        "prompt": task.prompt,
                        "system_prompt": "You are EnginEdge Autonomous Agent evaluating a coding benchmark. Inspect the codebase, solve the problem by writing or replacing symbols, verify with tests, and complete the task.",
                        "llm_profile": options.profile.clone().unwrap_or_else(|| "default".to_string()),
                        "tools": tools::all_local_tool_schemas(),
                    });

                    if let Err(e) = b.send_raw(init_payload.to_string()).await {
                        let err_str = format!("Failed to dispatch task to Cloud Bridge: {}", e);
                        trace_log.push_str(&format!("[FATAL ERROR] {}\n", err_str));
                        error_msg = Some(err_str);
                    } else {
                        let timeout = Duration::from_secs(options.timeout_secs);
                        let timeout_instant = Instant::now();

                        while timeout_instant.elapsed() < timeout {
                            match tokio::time::timeout(Duration::from_millis(500), event_rx.recv()).await {
                                Ok(Ok(event)) => {
                                    match event {
                                        BridgeEvent::StreamEvent { event, data } => {
                                            if event == "thought" {
                                                thought_steps += 1;
                                                let t_text = data.as_str().unwrap_or("");
                                                trace_log.push_str(&format!("[THOUGHT #{}] {}\n", thought_steps, t_text));
                                                eprintln!("💭 [Thought] {}", t_text);
                                            } else if event == "tool_call_start" {
                                                total_tool_calls += 1;
                                                let tool_name = data.get("tool").and_then(|t| t.as_str()).unwrap_or("");
                                                if matches!(tool_name, "get_outline" | "read_symbol" | "replace_symbol") {
                                                    ast_tool_calls += 1;
                                                } else if matches!(tool_name, "read_file" | "write_file" | "grep_search" | "list_files") {
                                                    file_tool_calls += 1;
                                                }
                                                trace_log.push_str(&format!("[TOOL_CALL #{}] {}\n", total_tool_calls, data));
                                                eprintln!("🛠️ [Tool Call] {}", data);
                                            } else if event == "tool_call_end" {
                                                trace_log.push_str(&format!("[TOOL_RESULT] {}\n", data));
                                                eprintln!("✔️ [Tool Result] {}", data);
                                            }
                                        }
                                        BridgeEvent::Finished { result, status } => {
                                            trace_log.push_str(&format!("[FINISHED] Status: {}, Result: {}\n", status, result));
                                            eprintln!("🏁 [Finished] Status: {}", status);
                                            break;
                                        }
                                        BridgeEvent::Error { message } => {
                                            trace_log.push_str(&format!("[AGENT ERROR] {}\n", message));
                                            eprintln!("❌ [Error] {}", message);
                                            error_msg = Some(message);
                                            break;
                                        }
                                    }
                                }
                                Ok(Err(_)) => break,
                                Err(_) => continue,
                            }
                        }
                    }
                } else {
                    let err_str = "Cloud bridge not connected".to_string();
                    trace_log.push_str(&format!("[FATAL ERROR] {}\n", err_str));
                    eprintln!("❌ [Bridge] Failed to connect to Cloud Bridge within 15s");
                    error_msg = Some(err_str);
                }
        } else {
            trace_log.push_str("[DRY RUN] Cloud LLM inference skipped.\n");
        }

        // Grader: Run verification command directly in sandbox_dir
        let (passed, stdout, stderr) = grade_task(&task.verify_command, &sandbox_dir);

        let duration_ms = task_start.elapsed().as_millis();

        trace_log.push_str("\n-------------------------------------------------------\n");
        trace_log.push_str(&format!("VERIFY COMMAND: {}\n", task.verify_command));
        trace_log.push_str(&format!("PASSED: {}\n", passed));
        trace_log.push_str(&format!("DURATION: {}ms\n", duration_ms));
        trace_log.push_str("STDOUT:\n");
        trace_log.push_str(&stdout);
        trace_log.push_str("\nSTDERR:\n");
        trace_log.push_str(&stderr);
        trace_log.push_str("\n-------------------------------------------------------\n");

        let log_file_path = abs_log_dir.join(format!("{}.log", task.id));
        let _ = fs::write(&log_file_path, &trace_log);
        let log_file_str = log_file_path.to_string_lossy().to_string();

        let icon = if passed { "✅ PASS" } else { "❌ FAIL" };
        eprintln!("Result: {} (Duration: {}ms, Thoughts: {}, Tools: {} [AST: {}, File: {}], Log: {})\n",
            icon, duration_ms, thought_steps, total_tool_calls, ast_tool_calls, file_tool_calls, log_file_str);

        task_results.push(TaskResult {
            task_id: task.id.clone(),
            lang: task.lang.clone(),
            passed,
            duration_ms,
            thought_steps,
            total_tool_calls,
            ast_tool_calls,
            file_tool_calls,
            verification_stdout: stdout,
            verification_stderr: stderr,
            error_message: error_msg,
            log_file: Some(log_file_str),
        });

        // Restore original directory if changed & cleanup sandbox
        if !options.dry_run {
            let _ = env::set_current_dir(&original_cwd);
        }
        let _ = fs::remove_dir_all(&sandbox_dir);
    }

    let report = compile_report(task_results, overall_start.elapsed().as_millis());
    write_report(&report, &abs_output_path)?;

    Ok(report)
}

fn grade_task(verify_command: &str, working_dir: &Path) -> (bool, String, String) {
    let tokens: Vec<&str> = verify_command.split_whitespace().collect();
    if tokens.is_empty() {
        return (false, String::new(), "Empty verify command".to_string());
    }

    let program = tokens[0];
    let cleaned_args: Vec<String> = tokens[1..]
        .iter()
        .map(|t| t.trim_matches('\'').trim_matches('"').to_string())
        .collect();

    match std::process::Command::new(program).current_dir(working_dir).args(&cleaned_args).output() {
        Ok(output) => {
            let passed = output.status.success();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            (passed, stdout, stderr)
        }
        Err(e) => (false, String::new(), format!("Failed to spawn verify command '{}': {}", verify_command, e)),
    }
}

fn compile_report(results: Vec<TaskResult>, total_duration_ms: u128) -> BenchReport {
    let total_tasks = results.len();
    let passed_tasks = results.iter().filter(|r| r.passed).count();
    let pass_rate = if total_tasks > 0 {
        (passed_tasks as f64 / total_tasks as f64) * 100.0
    } else {
        0.0
    };

    let mut lang_stats: HashMap<String, LangStat> = HashMap::new();

    for r in &results {
        let entry = lang_stats.entry(r.lang.clone()).or_default();
        entry.total += 1;
        if r.passed {
            entry.passed += 1;
        }
        entry.avg_duration_ms += r.duration_ms;
        entry.total_ast_tools += r.ast_tool_calls;
        entry.total_file_tools += r.file_tool_calls;
    }

    for stat in lang_stats.values_mut() {
        if stat.total > 0 {
            stat.pass_rate = (stat.passed as f64 / stat.total as f64) * 100.0;
            stat.avg_duration_ms /= stat.total as u128;
        }
    }

    let official_baseline_score = OFFICIAL_DEEPSEEK_V41_FLASH_1017;
    let score_delta = pass_rate - official_baseline_score;

    BenchReport {
        total_tasks,
        passed_tasks,
        pass_rate,
        official_baseline_score,
        score_delta,
        total_duration_ms,
        lang_stats,
        task_results: results,
    }
}

fn write_report(report: &BenchReport, output_path: &Path) -> anyhow::Result<()> {
    let mut md = String::new();

    md.push_str("# 🚀 EnginEdge Benchmark Evaluation Scorecard\n\n");
    md.push_str("- **Target Model**: DeepSeek-V4.1-Flash (Cloud)\n");
    md.push_str("- **Harness & Framework**: EnginEdge Edge Agent (AST Engine + Safety Policy)\n");
    md.push_str(&format!("- **Official DeepSeek Baseline (Aider Polyglot -1017)**: `{:.1}%`\n", report.official_baseline_score));
    md.push_str(&format!("- **Official DeepSeek Baseline (V4.1-Flash Initial)**: `{:.1}%`\n", OFFICIAL_DEEPSEEK_V41_FLASH_BASE));
    md.push_str(&format!("- **Total Duration**: `{:.2}s`\n\n", report.total_duration_ms as f64 / 1000.0));

    md.push_str("## 📊 Summary Scorecard\n\n");
    md.push_str("| Total Tasks | Passed | Failed | EnginEdge Pass@1 | Official Baseline | Score Delta |\n");
    md.push_str("| :---: | :---: | :---: | :---: | :---: | :---: |\n");

    let delta_str = if report.score_delta >= 0.0 {
        format!("🟢 +{:.1}%", report.score_delta)
    } else {
        format!("🔴 {:.1}%", report.score_delta)
    };

    md.push_str(&format!(
        "| **{}** | **{}** | **{}** | **{:.1}%** | **{:.1}%** | **{}** |\n\n",
        report.total_tasks,
        report.passed_tasks,
        report.total_tasks.saturating_sub(report.passed_tasks),
        report.pass_rate,
        report.official_baseline_score,
        delta_str
    ));

    md.push_str("## 🌐 Language Breakdown\n\n");
    md.push_str("| Language | Tasks | Passed | Pass Rate | Avg Time | AST Tool Calls | File Tool Calls |\n");
    md.push_str("| :--- | :---: | :---: | :---: | :---: | :---: | :---: |\n");

    let mut sorted_langs: Vec<_> = report.lang_stats.iter().collect();
    sorted_langs.sort_by_key(|(k, _)| (*k).clone());

    for (lang, stat) in sorted_langs {
        md.push_str(&format!(
            "| **{}** | {} | {} | {:.1}% | {:.2}s | {} | {} |\n",
            lang,
            stat.total,
            stat.passed,
            stat.pass_rate,
            stat.avg_duration_ms as f64 / 1000.0,
            stat.total_ast_tools,
            stat.total_file_tools
        ));
    }
    md.push('\n');

    md.push_str("## 📝 Task Execution Details\n\n");
    md.push_str("| Task ID | Language | Status | Duration | AST / File Tools | Details |\n");
    md.push_str("| :--- | :--- | :---: | :---: | :---: | :--- |\n");

    for r in &report.task_results {
        let status_badge = if r.passed { "✅ PASS" } else { "❌ FAIL" };
        let note = r.error_message.as_deref().unwrap_or("Completed");
        md.push_str(&format!(
            "| `{}` | {} | {} | {:.2}s | {} / {} | {} |\n",
            r.task_id,
            r.lang,
            status_badge,
            r.duration_ms as f64 / 1000.0,
            r.ast_tool_calls,
            r.file_tool_calls,
            note
        ));
    }
    md.push('\n');

    md.push_str("## 🔍 Failure Diagnostics & Detailed Error Logs\n\n");

    let failed_tasks: Vec<_> = report.task_results.iter().filter(|r| !r.passed).collect();
    if failed_tasks.is_empty() {
        md.push_str("🎉 **All evaluated tasks passed successfully! No failure diagnostics needed.**\n\n");
    } else {
        md.push_str("> 💡 **排障提示**：点击下方折叠卡片可直接查看未通过任务的编译器报错、单元测试断言失败堆栈或执行异常。完整思考与工具交互轨迹已持久化至日志目录。\n\n");
        for r in failed_tasks {
            md.push_str(&format!(
                "<details>\n<summary>❌ <b>{}</b> (<code>{}</code>) — 点击展开失败报错堆栈</summary>\n\n",
                r.task_id, r.lang
            ));
            if let Some(ref err) = r.error_message {
                md.push_str(&format!("- **通信/调度异常**: `{}`\n", err));
            }
            if let Some(ref log_f) = r.log_file {
                md.push_str(&format!("- **完整轨迹日志**: `{}`\n", log_f));
            }
            md.push_str(&format!("- **执行耗时**: `{:.2}s`\n", r.duration_ms as f64 / 1000.0));
            md.push_str(&format!("- **交互遥测**: 思考 {} 步，调用工具 {} 次 (AST: {}, File: {})\n\n",
                r.thought_steps, r.total_tool_calls, r.ast_tool_calls, r.file_tool_calls));

            let output = if !r.verification_stderr.trim().is_empty() {
                &r.verification_stderr
            } else if !r.verification_stdout.trim().is_empty() {
                &r.verification_stdout
            } else {
                "No verification output captured (exit code non-zero)"
            };

            md.push_str("```text\n");
            md.push_str(output.trim());
            md.push_str("\n```\n\n</details>\n\n");
        }
    }

    if let Some(parent) = output_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(output_path, &md)?;

    let json_path = output_path.with_extension("json");
    fs::write(json_path, serde_json::to_string_pretty(report)?)?;

    eprintln!("📄 [EnginEdge Bench] Report successfully written to: {}", output_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_tasks_loaded() {
        let tasks = get_builtin_tasks();
        assert_eq!(tasks.len(), 4);

        let langs: Vec<&str> = tasks.iter().map(|t| t.lang.as_str()).collect();
        assert!(langs.contains(&"rust"));
        assert!(langs.contains(&"go"));
        assert!(langs.contains(&"python"));
        assert!(langs.contains(&"javascript"));
    }

    #[test]
    fn test_load_tasks_filter_and_limit() {
        let allowed = vec!["rust".to_string(), "go".to_string()];
        let filtered = load_tasks(None, &allowed, Some(1));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].lang, "rust");
    }

    #[test]
    fn test_compile_report() {
        let results = vec![
            TaskResult {
                task_id: "task1".to_string(),
                lang: "rust".to_string(),
                passed: true,
                duration_ms: 1000,
                thought_steps: 3,
                total_tool_calls: 5,
                ast_tool_calls: 3,
                file_tool_calls: 2,
                verification_stdout: "".to_string(),
                verification_stderr: "".to_string(),
                error_message: None,
                log_file: Some("bench_logs/task1.log".to_string()),
            },
            TaskResult {
                task_id: "task2".to_string(),
                lang: "rust".to_string(),
                passed: false,
                duration_ms: 2000,
                thought_steps: 2,
                total_tool_calls: 4,
                ast_tool_calls: 1,
                file_tool_calls: 3,
                verification_stdout: "".to_string(),
                verification_stderr: "".to_string(),
                error_message: None,
                log_file: Some("bench_logs/task2.log".to_string()),
            },
        ];

        let report = compile_report(results, 3000);
        assert_eq!(report.total_tasks, 2);
        assert_eq!(report.passed_tasks, 1);
        assert_eq!(report.pass_rate, 50.0);
        assert!(report.score_delta < 0.0);

        let rust_stat = report.lang_stats.get("rust").unwrap();
        assert_eq!(rust_stat.total, 2);
        assert_eq!(rust_stat.passed, 1);
        assert_eq!(rust_stat.pass_rate, 50.0);
        assert_eq!(rust_stat.total_ast_tools, 4);
    }

    #[tokio::test]
    async fn test_bench_dry_run() {
        let options = BenchOptions {
            dataset_path: None,
            langs: vec!["rust".to_string()],
            limit: Some(1),
            output_path: "target/test_dry_run_report.md".to_string(),
            log_dir: "target/test_bench_logs".to_string(),
            hub_url: None,
            profile: None,
            dry_run: true,
            timeout_secs: 5,
        };

        let report = run_benchmark(options).await.unwrap();
        assert_eq!(report.total_tasks, 1);
        // In dry run without code implementation, the test should fail
        assert_eq!(report.passed_tasks, 0);

        assert!(Path::new("target/test_dry_run_report.md").exists());
        let report_content = fs::read_to_string("target/test_dry_run_report.md").unwrap();
        assert!(report_content.contains("Failure Diagnostics"));

        assert!(Path::new("target/test_bench_logs/rust_lru_cache.log").exists());
        let log_content = fs::read_to_string("target/test_bench_logs/rust_lru_cache.log").unwrap();
        assert!(log_content.contains("rust_lru_cache"));
        assert!(log_content.contains("VERIFY COMMAND"));

        let _ = fs::remove_file("target/test_dry_run_report.md");
        let _ = fs::remove_file("target/test_dry_run_report.json");
        let _ = fs::remove_dir_all("target/test_bench_logs");
    }
}
