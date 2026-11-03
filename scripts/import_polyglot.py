#!/usr/bin/env python3
"""
EnginEdge Polyglot Benchmark Dataset Importer
Converts exercises from Aider-AI/polyglot-benchmark into EnginEdge BenchTask format.
"""

import os
import sys
import json
import glob
from pathlib import Path

def parse_exercise(lang: str, ex_dir: Path) -> dict:
    ex_name = ex_dir.name
    
    # Prompt is composed of .docs/instructions.md and optional .docs/instructions.append.md
    instructions_file = ex_dir / ".docs" / "instructions.md"
    append_file = ex_dir / ".docs" / "instructions.append.md"
    
    prompt = ""
    if instructions_file.exists():
        prompt = instructions_file.read_text(encoding="utf-8")
    if append_file.exists():
        prompt += "\n\n" + append_file.read_text(encoding="utf-8")
        
    if not prompt.strip():
        prompt = f"Implement the {ex_name} exercise in {lang}. Pass all unit tests."

    files = {}
    ignored_dirs = {".git", ".docs", ".meta", "node_modules", "target", ".pytest_cache"}
    
    # Allowed file extensions per language
    allowed_exts = {
        "rust": (".rs", ".toml"),
        "go": (".go", ".mod"),
        "python": (".py",),
        "javascript": (".js", ".json", ".mjs", ".cjs"),
        "cpp": (".cpp", ".h", ".hpp", "CMakeLists.txt"),
        "java": (".java", ".gradle", ".properties"),
    }
    target_exts = allowed_exts.get(lang, (".txt",))
    
    for root, dirs, filenames in os.walk(ex_dir):
        dirs[:] = [d for d in dirs if d not in ignored_dirs and not d.startswith(".")]
        for fn in filenames:
            if any(fn.endswith(ext) for ext in target_exts) and not fn.startswith("."):
                fpath = Path(root) / fn
                rel_path = str(fpath.relative_to(ex_dir))
                try:
                    files[rel_path] = fpath.read_text(encoding="utf-8")
                except Exception:
                    pass

    verify_command = ""
    if lang == "rust":
        verify_command = "cargo test -- --include-ignored"
    elif lang == "go":
        verify_command = "go test -v ."
    elif lang == "python":
        ex_slug = ex_name.replace("-", "_")
        test_file = f"{ex_slug}_test.py"
        verify_command = f"python3 -m unittest discover -s . -p {test_file}"
    elif lang == "javascript":
        verify_command = "jest ./*"
    elif lang == "cpp":
        verify_command = "cmake -B build -DEXERCISM_RUN_ALL_TESTS=ON && cmake --build build"
    elif lang == "java":
        verify_command = "gradle test"

    return {
        "id": f"{lang}_{ex_name}",
        "lang": lang,
        "title": f"{ex_name} ({lang})",
        "prompt": prompt.strip(),
        "files": files,
        "verify_command": verify_command,
    }

def main():
    polyglot_root = Path(sys.argv[1] if len(sys.argv) > 1 else "/root/Projects/polyglot-benchmark")
    out_dir = Path(sys.argv[2] if len(sys.argv) > 2 else "datasets")
    out_dir.mkdir(parents=True, exist_ok=True)
    
    languages = ["rust", "go", "python", "javascript", "cpp", "java"]
    all_tasks = []
    
    for lang in languages:
        practice_dir = polyglot_root / lang / "exercises" / "practice"
        if not practice_dir.exists():
            continue
        lang_tasks = []
        for ex_path in sorted(practice_dir.iterdir()):
            if ex_path.is_dir() and not ex_path.name.startswith("."):
                task = parse_exercise(lang, ex_path)
                if task["files"]:
                    lang_tasks.append(task)
        print(f"Loaded {len(lang_tasks)} tasks for {lang}.")
        all_tasks.extend(lang_tasks)
        
        # Per-language dataset
        lang_file = out_dir / f"polyglot_{lang}_{len(lang_tasks)}.jsonl"
        with open(lang_file, "w", encoding="utf-8") as f:
            for t in lang_tasks:
                f.write(json.dumps(t, ensure_ascii=False) + "\n")
        print(f"  -> Wrote {lang_file}")
                    
    print(f"\nTotal loaded: {len(all_tasks)} exercises across all 6 languages.")
    
    # Save full 225 polyglot dataset (JSONL)
    full_225 = out_dir / "polyglot_225_full.jsonl"
    with open(full_225, "w", encoding="utf-8") as f:
        for t in all_tasks:
            f.write(json.dumps(t, ensure_ascii=False) + "\n")
    print(f"Wrote full 225-task dataset to {full_225}")

    # Save remaining 122 tasks (javascript, cpp, java)
    remaining_122 = [t for t in all_tasks if t["lang"] in ("javascript", "cpp", "java")]
    rem_file = out_dir / "polyglot_remaining_122.jsonl"
    with open(rem_file, "w", encoding="utf-8") as f:
        for t in remaining_122:
            f.write(json.dumps(t, ensure_ascii=False) + "\n")
    print(f"Wrote remaining {len(remaining_122)} tasks (JS, C++, Java) to {rem_file}")

    # Canonical Batch 1 (6 tasks: 2 Rust, 2 Go, 2 Python)
    batch1_ids = {
        "rust_accumulate", "rust_word-count",
        "go_matrix", "go_palindrome-products",
        "python_affine-cipher", "python_beer-song",
    }
    batch1_tasks = [t for t in all_tasks if t["id"] in batch1_ids]
    if len(batch1_tasks) < 6:
        batch1_tasks = []
        for l in ["rust", "go", "python"]:
            batch1_tasks.extend([t for t in all_tasks if t["lang"] == l][:2])
            
    batch1_jsonl = out_dir / "polyglot_batch1.jsonl"
    with open(batch1_jsonl, "w", encoding="utf-8") as f:
        for t in batch1_tasks:
            f.write(json.dumps(t, ensure_ascii=False) + "\n")
    print(f"Wrote Batch 1 ({len(batch1_tasks)} tasks) to {batch1_jsonl}")

if __name__ == "__main__":
    main()
