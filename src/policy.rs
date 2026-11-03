use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

pub const DEFAULT_DEV_WEIGHT: u32 = 40;
pub const DEFAULT_EVAL_WEIGHT: u32 = 75;
pub const PERMANENT_BLOCK_WEIGHT: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    Dev,
    Eval,
    Custom,
}

impl PolicyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PolicyMode::Dev => "dev",
            PolicyMode::Eval => "eval",
            PolicyMode::Custom => "custom",
        }
    }

    pub fn default_weight(&self) -> u32 {
        match self {
            PolicyMode::Dev => DEFAULT_DEV_WEIGHT,
            PolicyMode::Eval => DEFAULT_EVAL_WEIGHT,
            PolicyMode::Custom => DEFAULT_DEV_WEIGHT,
        }
    }
}

static CURRENT_MODE: Lazy<RwLock<PolicyMode>> = Lazy::new(|| RwLock::new(PolicyMode::Dev));
static CURRENT_MAX_WEIGHT: AtomicU32 = AtomicU32::new(DEFAULT_DEV_WEIGHT);

pub fn init_policy(mode_str: &str, max_weight_override: Option<u32>) {
    let mode = match mode_str.to_ascii_lowercase().as_str() {
        "eval" | "benchmark" | "bench" => PolicyMode::Eval,
        "dev" | "development" => PolicyMode::Dev,
        _ => PolicyMode::Custom,
    };

    let weight = max_weight_override.unwrap_or_else(|| mode.default_weight()).min(99);

    if let Ok(mut lock) = CURRENT_MODE.write() {
        *lock = mode;
    }
    CURRENT_MAX_WEIGHT.store(weight, Ordering::SeqCst);
}

pub fn get_current_mode() -> PolicyMode {
    CURRENT_MODE.read().map(|l| *l).unwrap_or(PolicyMode::Dev)
}

pub fn get_max_weight() -> u32 {
    CURRENT_MAX_WEIGHT.load(Ordering::SeqCst)
}

pub fn set_max_weight(weight: u32) {
    let clamped = weight.min(99);
    CURRENT_MAX_WEIGHT.store(clamped, Ordering::SeqCst);
    if let Ok(mut lock) = CURRENT_MODE.write() {
        *lock = PolicyMode::Custom;
    }
}

pub fn get_policy_summary() -> String {
    let mode = get_current_mode();
    let weight = get_max_weight();
    match mode {
        PolicyMode::Dev => format!("Dev Mode (Max Weight: {}) - Safe development, unit tests & linting allowed", weight),
        PolicyMode::Eval => format!("Eval Mode (Max Weight: {}) - Benchmark extended, multi-language builds, git restore & chains allowed", weight),
        PolicyMode::Custom => format!("Custom Policy (Max Weight: {})", weight),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PolicyCheckResult {
    Allowed {
        weight: u32,
        sub_commands: Vec<String>,
    },
    Denied {
        weight: u32,
        reason: String,
    },
}

/// Evaluates command string against fixed, deterministic security rules (0-100 scale) using current max weight.
pub fn evaluate_command(raw_cmd: &str) -> PolicyCheckResult {
    evaluate_command_with_weight(raw_cmd, get_max_weight())
}

/// Evaluates command string against fixed, deterministic security rules with an explicit max weight threshold.
pub fn evaluate_command_with_weight(raw_cmd: &str, max_allowed: u32) -> PolicyCheckResult {
    let raw_cmd = raw_cmd.trim();
    if raw_cmd.is_empty() {
        return PolicyCheckResult::Denied {
            weight: PERMANENT_BLOCK_WEIGHT,
            reason: "Empty verification command".to_string(),
        };
    }

    // 1. Red Line Check: Absolutely forbidden metacharacters
    for forbidden in ['|', ';', '>', '<', '`', '$', '\n'] {
        if raw_cmd.contains(forbidden) {
            return PolicyCheckResult::Denied {
                weight: PERMANENT_BLOCK_WEIGHT,
                reason: format!("Forbidden shell metacharacter '{}' strictly blocked by policy", forbidden),
            };
        }
    }

    // 2. Security Jail Path Escape Check
    if raw_cmd.contains("..") {
        return PolicyCheckResult::Denied {
            weight: PERMANENT_BLOCK_WEIGHT,
            reason: "Directory traversal '..' strictly blocked by policy".to_string(),
        };
    }

    // 3. Sensitive file access check
    let lower_cmd = raw_cmd.to_ascii_lowercase();
    for sensitive in [".env", "id_rsa", "id_ed25519", ".pem", ".key", ".aws", ".ssh", "credentials"] {
        if lower_cmd.contains(sensitive) {
            return PolicyCheckResult::Denied {
                weight: PERMANENT_BLOCK_WEIGHT,
                reason: format!("Access to sensitive file or pattern '{}' blocked by policy", sensitive),
            };
        }
    }

    // 4. Split by controlled chaining operator '&&'
    let parts: Vec<&str> = raw_cmd.split("&&").map(|s| s.trim()).collect();
    if parts.iter().any(|p| p.is_empty()) {
        return PolicyCheckResult::Denied {
            weight: PERMANENT_BLOCK_WEIGHT,
            reason: "Empty sub-command in '&&' chain".to_string(),
        };
    }

    let is_chained = parts.len() > 1;
    let chain_penalty = if is_chained { 10 } else { 0 };

    let mut highest_sub_weight = 0;
    let mut parsed_commands = Vec::new();

    for part in parts {
        let tokens: Vec<&str> = part.split_whitespace().collect();
        if tokens.is_empty() {
            return PolicyCheckResult::Denied {
                weight: PERMANENT_BLOCK_WEIGHT,
                reason: "Empty command fragment".to_string(),
            };
        }

        let sub_weight = match evaluate_single_subcommand(&tokens) {
            Ok(w) => w,
            Err(reason) => {
                return PolicyCheckResult::Denied {
                    weight: PERMANENT_BLOCK_WEIGHT,
                    reason,
                };
            }
        };

        if sub_weight > highest_sub_weight {
            highest_sub_weight = sub_weight;
        }
        parsed_commands.push(part.to_string());
    }

    let final_weight = (highest_sub_weight + chain_penalty).min(PERMANENT_BLOCK_WEIGHT);

    if final_weight > max_allowed {
        PolicyCheckResult::Denied {
            weight: final_weight,
            reason: format!(
                "Command weight {} exceeds current limit {} ({}). Required weight: {}",
                final_weight,
                max_allowed,
                get_current_mode().as_str(),
                final_weight
            ),
        }
    } else {
        PolicyCheckResult::Allowed {
            weight: final_weight,
            sub_commands: parsed_commands,
        }
    }
}

/// Evaluates a single tokenized sub-command against the deterministic rule table.
fn evaluate_single_subcommand(tokens: &[&str]) -> Result<u32, String> {
    let program = tokens[0];

    // Explicit Blocklist
    match program {
        "rm" | "mv" | "sudo" | "curl" | "wget" | "bash" | "sh" | "zsh" | "ssh" | "scp" |
        "nc" | "ncat" | "netcat" | "dd" | "chmod" | "chown" | "kill" | "pkill" | "killall" |
        "reboot" | "shutdown" | "su" | "eval" | "exec" | "alias" => {
            return Err(format!("Command '{}' is permanently forbidden by policy", program));
        }
        _ => {}
    }

    match program {
        // === Level 1: Read-Only & Inspection (10 - 20) ===
        "git" => {
            let sub = tokens.get(1).copied().unwrap_or("");
            match sub {
                "status" | "diff" | "log" | "branch" => Ok(10),
                // Level 3: Controlled single-file restore/checkout for evaluation error-recovery
                "checkout" => {
                    // Only allow: git checkout -- <file>
                    if tokens.len() == 4 && tokens[2] == "--" {
                        Ok(70)
                    } else {
                        Err("Arbitrary git checkout is forbidden. Only 'git checkout -- <file>' is permitted in eval mode".to_string())
                    }
                }
                "restore" => {
                    // Only allow: git restore <file> or git restore --staged <file>
                    if tokens.len() >= 3 && !tokens.iter().any(|&t| t.starts_with('-') && t != "--staged") {
                        Ok(70)
                    } else {
                        Err("Arbitrary git restore flags are forbidden. Only 'git restore <file>' is permitted in eval mode".to_string())
                    }
                }
                _ => Err(format!("Git subcommand 'git {}' is not permitted by policy", sub)),
            }
        }

        // === Level 1-3: Rust Ecosystem ===
        "cargo" => {
            let sub = tokens.get(1).copied().unwrap_or("");
            match sub {
                "check" | "clippy" => Ok(15),
                "test" => Ok(30),
                "build" => Ok(50),
                _ => Err(format!("Cargo subcommand 'cargo {}' is not permitted", sub)),
            }
        }
        "rustc" => Ok(50),

        // === Level 1-3: Go Ecosystem ===
        "go" => {
            let sub = tokens.get(1).copied().unwrap_or("");
            match sub {
                "vet" => Ok(15),
                "test" => Ok(30),
                "build" => Ok(50),
                _ => Err(format!("Go subcommand 'go {}' is not permitted", sub)),
            }
        }

        // === Level 2-3: JavaScript / TypeScript Ecosystem ===
        "jest" => Ok(30),
        "npm" | "pnpm" | "yarn" | "bun" => {
            let sub = tokens.get(1).copied().unwrap_or("");
            match sub {
                "test" => Ok(30),
                "run" => {
                    let task = tokens.get(2).copied().unwrap_or("");
                    match task {
                        "test" | "check" | "lint" => Ok(30),
                        "build" => Ok(50),
                        _ => Err(format!("Script '{} {} {}' is not permitted by policy", program, sub, task)),
                    }
                }
                "build" => Ok(50),
                _ => Err(format!("Subcommand '{} {}' is not permitted by policy", program, sub)),
            }
        }

        // === Level 2: Python Testing ===
        "pytest" => Ok(30),
        "python" | "python3" => {
            if tokens.len() >= 3 && tokens[1] == "-m" {
                match tokens[2] {
                    "unittest" | "pytest" => Ok(30),
                    _ => Err(format!("Python module execution 'python -m {}' is not in verification whitelist", tokens[2])),
                }
            } else {
                Err("Direct Python interpreter execution forbidden. Only 'python -m unittest/pytest' allowed".to_string())
            }
        }

        // === Level 2: Ruby Testing ===
        "bundle" => {
            if tokens.len() >= 3 && tokens[1] == "exec" {
                match tokens[2] {
                    "rspec" | "rake" => Ok(30),
                    _ => Err(format!("Bundle exec target '{}' is not permitted", tokens[2])),
                }
            } else {
                Err("Only 'bundle exec rspec/rake' is permitted for ruby".to_string())
            }
        }

        // === Level 3: C++ & Native Build Ecosystem ===
        "gcc" | "g++" | "clang" | "clang++" => Ok(55),
        "cmake" => Ok(60),
        "ctest" => Ok(60),
        "make" => {
            let sub = tokens.get(1).copied().unwrap_or("");
            match sub {
                "test" | "check" => Ok(35),
                "" | "build" | "all" => Ok(60),
                _ => Ok(60), // standard make targets
            }
        }

        // === Level 3: Java & JVM Ecosystem ===
        "javac" => Ok(50),
        "mvn" => {
            let sub = tokens.get(1).copied().unwrap_or("");
            match sub {
                "test" => Ok(30),
                "compile" | "package" | "verify" => Ok(60),
                _ => Ok(60),
            }
        }
        "gradle" | "./gradlew" => {
            let sub = tokens.get(1).copied().unwrap_or("");
            match sub {
                "test" | "check" => Ok(30),
                "build" | "assemble" => Ok(60),
                _ => Ok(60),
            }
        }

        _ => Err(format!("Program '{}' is not recognized in policy rules (Weight 100 Denied)", program)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_only_commands_under_dev_mode() {
        init_policy("dev", None);
        assert_eq!(get_max_weight(), 40);

        let res = evaluate_command("git status");
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 10, .. }));

        let res = evaluate_command("cargo check");
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 15, .. }));

        let res = evaluate_command("go vet");
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 15, .. }));
    }

    #[test]
    fn test_test_commands_under_dev_mode() {
        init_policy("dev", None);

        let res = evaluate_command("cargo test");
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 30, .. }));

        let res = evaluate_command("pytest");
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 30, .. }));

        let res = evaluate_command("python3 -m unittest");
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 30, .. }));

        let res = evaluate_command("npm test");
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 30, .. }));

        let res = evaluate_command("jest ./*");
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 30, .. }));
    }

    #[test]
    fn test_eval_mode_unlocks_build_and_cmake() {
        // Under dev threshold (weight 40), cmake and go build are denied
        let res = evaluate_command_with_weight("cmake -B build", 40);
        assert!(matches!(res, PolicyCheckResult::Denied { weight: 60, .. }));

        let res = evaluate_command_with_weight("go build", 40);
        assert!(matches!(res, PolicyCheckResult::Denied { weight: 50, .. }));

        // Under eval threshold (weight 75), both are allowed
        let res = evaluate_command_with_weight("cmake -B build", 75);
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 60, .. }));

        let res = evaluate_command_with_weight("go build", 75);
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 50, .. }));

        let res = evaluate_command_with_weight("git restore src/main.rs", 75);
        assert!(matches!(res, PolicyCheckResult::Allowed { weight: 70, .. }));
    }

    #[test]
    fn test_controlled_chaining_with_and() {
        // cmake (60) && ctest (60) + chained penalty (10) = 70 <= 75 -> Allowed
        let res = evaluate_command_with_weight("cmake -B build && ctest --test-dir build", 75);
        match res {
            PolicyCheckResult::Allowed { weight, sub_commands } => {
                assert_eq!(weight, 70);
                assert_eq!(sub_commands.len(), 2);
            }
            _ => panic!("Expected allowed chain"),
        }

        // Under dev threshold (40), it should be denied
        let res = evaluate_command_with_weight("cmake -B build && ctest --test-dir build", 40);
        assert!(matches!(res, PolicyCheckResult::Denied { weight: 70, .. }));
    }

    #[test]
    fn test_permanent_blocks() {
        init_policy("eval", Some(99));

        // Metacharacters
        assert!(matches!(evaluate_command("cargo test | grep ok"), PolicyCheckResult::Denied { weight: 100, .. }));
        assert!(matches!(evaluate_command("cargo test; rm -rf /"), PolicyCheckResult::Denied { weight: 100, .. }));
        assert!(matches!(evaluate_command("cargo test > out.txt"), PolicyCheckResult::Denied { weight: 100, .. }));
        assert!(matches!(evaluate_command("cargo test `whoami`"), PolicyCheckResult::Denied { weight: 100, .. }));
        assert!(matches!(evaluate_command("cargo test $(whoami)"), PolicyCheckResult::Denied { weight: 100, .. }));

        // Path traversal
        assert!(matches!(evaluate_command("cargo test ../outside"), PolicyCheckResult::Denied { weight: 100, .. }));

        // Dangerous programs
        assert!(matches!(evaluate_command("rm -rf target"), PolicyCheckResult::Denied { weight: 100, .. }));
        assert!(matches!(evaluate_command("curl https://example.com"), PolicyCheckResult::Denied { weight: 100, .. }));
        assert!(matches!(evaluate_command("bash -c 'echo hello'"), PolicyCheckResult::Denied { weight: 100, .. }));

        // Sensitive files
        assert!(matches!(evaluate_command("cat .env"), PolicyCheckResult::Denied { weight: 100, .. }));
    }
}
