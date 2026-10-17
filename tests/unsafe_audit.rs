//! `unsafe` is allowed in exactly one place: the platform shims in `src/raw_mode/`.

use std::fs;
use std::path::Path;

fn rust_files(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn unsafe_code_only_appears_in_the_raw_mode_shims() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(!files.is_empty());

    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        let relative = file.strip_prefix(&src).unwrap();
        let allowed = relative.starts_with("raw_mode")
            && matches!(
                relative.file_name().and_then(|n| n.to_str()),
                Some("unix.rs" | "windows.rs")
            );
        // the lint name is spelled out in lib.rs, which is the one other mention
        let mentions = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .any(|l| l.contains("unsafe") && !l.contains("unsafe_code"));
        assert!(
            allowed || !mentions,
            "unexpected `unsafe` in src/{}",
            relative.display()
        );
    }
}

#[test]
fn every_unsafe_block_has_a_safety_comment() {
    let raw_mode = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/raw_mode");
    for name in ["unix.rs", "windows.rs"] {
        let Ok(text) = fs::read_to_string(raw_mode.join(name)) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.contains("unsafe {") {
                let above = lines[..i].iter().rev().take(3);
                assert!(
                    line.contains("// SAFETY:")
                        || above.into_iter().any(|l| l.contains("// SAFETY:")),
                    "{name}:{} has an unsafe block with no SAFETY comment",
                    i + 1
                );
            }
        }
    }
}
