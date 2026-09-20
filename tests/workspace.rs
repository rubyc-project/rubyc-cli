//! Workspace restructure: discovery registry, polymorphic target trait,
//! and module separation.

// These verify workspace discovery behavior through the public CLI.

fn listing(args: &[&str]) -> String {
    let argv = std::iter::once("rubyc".to_owned())
        .chain(args.iter().map(|value| (*value).to_owned()))
        .collect::<Vec<_>>();
    ::rubyc::driver::extension_listing(&argv)
        .unwrap()
        .join("\n")
}

// ---------- discovery ----------

#[test]
fn list_targets_includes_linux_x86_64() {
    let stdout = listing(&["--list-targets"]);
    assert!(stdout.contains("linux_x86_64"), "stdout: {stdout}");
}

#[test]
fn list_targets_sorted() {
    let stdout = listing(&["--list-targets"]);
    let names: Vec<&str> = stdout
        .lines()
        .map(|line| line.split_whitespace().next().unwrap_or(""))
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "targets must be sorted by name");
}

// ---------- discovery directories ----------

#[test]
fn discovery_scans_custom_dir() {
    // Create a temp dir with a TOML manifest for a fake target.
    let dir = std::env::temp_dir().join(format!("rubyc_disc_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("test_target.toml"),
        "[target]\nname = \"test_target\"\nauthor = \"tester\"\nversion = \"9.9\"\ntriple = \"x86_64-test-os\"\ndescription = \"test discovery\"",
    )
    .unwrap();

    let stdout = listing(&["--list-targets", "--targets-dir", dir.to_str().unwrap()]);
    assert!(
        stdout.contains("test_target"),
        "discovered target not found:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discovery_empty_dir_still_lists_builtins() {
    let dir = std::env::temp_dir().join(format!("rubyc_disc_empty_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let stdout = listing(&["--list-targets", "--targets-dir", dir.to_str().unwrap()]);
    assert!(stdout.contains("linux_x86_64"), "builtins must always be present");

    let _ = std::fs::remove_dir_all(&dir);
}
