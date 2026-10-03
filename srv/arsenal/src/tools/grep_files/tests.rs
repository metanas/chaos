use super::*;

use std::os::unix::fs::symlink;

#[test]
fn search_returns_matching_files() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let dir = temp.path();
    std::fs::write(dir.join("match_one.txt"), "alpha beta gamma").unwrap();
    std::fs::write(dir.join("match_two.txt"), "alpha delta").unwrap();
    std::fs::write(dir.join("other.txt"), "omega").unwrap();

    let results = run_grep_search("alpha", None, dir, 10, None).expect("search failed");
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|p| p.ends_with("match_one.txt")));
    assert!(results.iter().any(|p| p.ends_with("match_two.txt")));
}

#[test]
fn search_with_glob_filter() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let dir = temp.path();
    std::fs::write(dir.join("match_one.rs"), "alpha beta gamma").unwrap();
    std::fs::write(dir.join("match_two.txt"), "alpha delta").unwrap();

    let results = run_grep_search("alpha", Some("*.rs"), dir, 10, None).expect("search failed");
    assert_eq!(results.len(), 1);
    assert!(results[0].ends_with("match_one.rs"));
}

#[test]
fn search_respects_limit() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let dir = temp.path();
    std::fs::write(dir.join("one.txt"), "alpha one").unwrap();
    std::fs::write(dir.join("two.txt"), "alpha two").unwrap();
    std::fs::write(dir.join("three.txt"), "alpha three").unwrap();

    let results = run_grep_search("alpha", None, dir, 2, None).expect("search failed");
    assert_eq!(results.len(), 2);
}

#[test]
fn search_handles_no_matches() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let dir = temp.path();
    std::fs::write(dir.join("one.txt"), "omega").unwrap();

    let results = run_grep_search("alpha", None, dir, 5, None).expect("search failed");
    assert!(results.is_empty());
}

#[test]
fn search_rejects_invalid_regex() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let err = run_grep_search("[invalid", None, temp.path(), 10, None).unwrap_err();
    assert!(err.contains("invalid regex"));
}

#[test]
fn search_matches_non_utf8_files() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let dir = temp.path();
    std::fs::write(
        dir.join("latin1.txt"),
        [0xff, b'a', b'l', b'p', b'h', b'a', 0xfe],
    )
    .unwrap();

    let results = run_grep_search("alpha", None, dir, 10, None).expect("search failed");
    assert_eq!(results.len(), 1);
    assert!(results[0].ends_with("latin1.txt"));
}

#[test]
fn search_only_reads_regular_files() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let dir = temp.path();
    let real = dir.join("match.txt");
    let alias = dir.join("match-link.txt");
    std::fs::write(&real, "alpha beta gamma").unwrap();
    symlink(&real, &alias).expect("create symlink");

    let results = run_grep_search("alpha", None, dir, 10, None).expect("search failed");
    assert_eq!(results.len(), 1);
    assert!(results[0].ends_with("match.txt"));
}

#[test]
fn search_accepts_a_single_file_and_returns_an_absolute_path() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("one.rs");
    std::fs::write(&file, "alpha\n").unwrap();
    assert_eq!(
        run_grep_search("alpha", Some("*.rs"), &file, 10, None).unwrap(),
        vec![file.to_string_lossy().into_owned()]
    );

    let results = run_grep_search("chaos-arsenal", None, Path::new("Cargo.toml"), 10, None).unwrap();
    assert_eq!(results.len(), 1);
    assert!(Path::new(&results[0]).is_absolute());
}

#[test]
fn search_preserves_raw_regexes_and_multiline_matches() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("one.txt");
    std::fs::write(&file, "prefix\nalpha != beta\ngamma\n").unwrap();
    for pattern in [
        "^alpha != beta$",
        r"alpha != beta\ngamma",
        "(?s)alpha.*gamma",
        r"\w+ != \w+",
    ] {
        let results = run_grep_search(pattern, None, temp.path(), 10, Some(true)).unwrap();
        assert_eq!(results.len(), 1, "{pattern}");
    }
}

#[test]
fn search_selects_newest_matches_before_limiting_and_breaks_ties_by_path() {
    let temp = tempfile::tempdir().unwrap();
    let base_time = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    for (name, seconds) in [("old.txt", 0), ("new-b.txt", 100), ("new-a.txt", 100)] {
        let path = temp.path().join(name);
        std::fs::write(&path, "alpha").unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(base_time + std::time::Duration::from_secs(seconds)),
            )
            .unwrap();
    }
    let results = run_grep_search("alpha", None, temp.path(), 1, None).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].ends_with("new-a.txt"));
}

#[tokio::test]
async fn search_case_mode_is_optional_and_can_override_smart_case() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let dir = temp.path();
    std::fs::write(dir.join("lower.txt"), "alpha").unwrap();
    std::fs::write(dir.join("upper.txt"), "ALPHA").unwrap();

    for (pattern, case_sensitive, expected) in [
        ("alpha", None, vec!["lower.txt", "upper.txt"]),
        ("ALPHA", None, vec!["upper.txt"]),
        ("alpha", Some(true), vec!["lower.txt"]),
        ("ALPHA", Some(false), vec!["lower.txt", "upper.txt"]),
        ("^ALPHA$", Some(false), vec!["lower.txt", "upper.txt"]),
        ("(?i)^ALPHA$", Some(true), vec!["lower.txt", "upper.txt"]),
        ("(?-i)^alpha$", Some(false), vec!["lower.txt"]),
    ] {
        let mut arguments = serde_json::json!({
            "pattern": pattern,
            "path": dir,
        });
        if let Some(case_sensitive) = case_sensitive {
            arguments["case_sensitive"] = serde_json::json!(case_sensitive);
        }
        let result = execute_structured(&arguments).await.expect("search failed");
        let mut names = result["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|path| {
                Path::new(path.as_str().unwrap())
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        names.sort_unstable();
        assert_eq!(names, expected, "{arguments}");
    }
}

#[tokio::test]
async fn rejects_unknown_arguments() {
    let result = execute(&serde_json::json!({
        "pattern": "alpha",
        "pathh": "."
    }))
    .await;

    let err = result.expect_err("unknown field should fail");
    assert!(err.contains("unknown field `pathh`"));
}
