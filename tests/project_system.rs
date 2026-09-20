//! End-to-end project/solution and multi-file acceptance tests.

use crate::tests::common::{exec, rubyc_path};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new(label: &str) -> Self {
        let sequence = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rubyc_project_{label}_{}_{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn project(workspace: &Workspace, sources: &str) -> PathBuf {
    workspace.write(
        "app.rcp",
        &format!(
            r#"[meta]
name = "MultiFile"
version = "1.0.0"

[target]
platform = "linux_x86_64"
output = "exe"

[build]
sources = [{sources}]
entry-point = "main"
"#
        ),
    )
}

fn write_cross_file_program(workspace: &Workspace) {
    workspace.write(
        "src/a_program.rc",
        r#"namespace multifile {
            partial class Program {
                int main() { printf("MULTI|value={0}", helper()); return 0; }
            }
        }"#,
    );
    workspace.write(
        "src/z_helper.rc",
        r#"namespace multifile {
            partial class Program { int helper() { return 42; } }
        }"#,
    );
}

fn run_binary(path: &Path) -> String {
    let outcome = exec(&mut Command::new(path));
    assert_eq!(
        outcome.code,
        0,
        "execution failed:\n{}",
        outcome.stderr_str()
    );
    outcome.stdout_str()
}

#[test]
fn project_jit_merges_sources_before_resolving_cross_file_calls() {
    let workspace = Workspace::new("jit");
    write_cross_file_program(&workspace);
    let project = project(&workspace, r#"src/z_helper.rc", "src/a_program.rc""#);

    let outcome = exec(Command::new(rubyc_path()).arg("run").arg(project));
    assert_eq!(
        outcome.code,
        0,
        "project JIT failed:\n{}",
        outcome.stderr_str()
    );
    assert_eq!(outcome.stdout_str(), "MULTI|value=42");
}

#[test]
fn project_native_build_expands_glob_and_emits_one_executable() {
    let workspace = Workspace::new("native");
    write_cross_file_program(&workspace);
    let project = project(&workspace, r#"src/*.rc""#);
    let binary = workspace.0.join("multifile");

    let outcome = exec(
        Command::new(rubyc_path())
            .arg("build")
            .arg(project)
            .arg("-o")
            .arg(&binary),
    );
    assert_eq!(
        outcome.code,
        0,
        "project build failed:\n{}",
        outcome.stderr_str()
    );
    assert_eq!(&std::fs::read(&binary).unwrap()[..4], b"\x7fELF");
    assert_eq!(run_binary(&binary), "MULTI|value=42");
}

#[test]
fn solution_resolves_project_relative_to_solution_location() {
    let workspace = Workspace::new("solution");
    write_cross_file_program(&workspace);
    project(&workspace, r#"src/*.rc""#);
    let solution = workspace.write(
        "workspace.rcs",
        r#"solution "Workspace"
[[project]]
path = "app.rcp"
"#,
    );
    let binary = workspace.0.join("solution-app");

    let outcome = exec(
        Command::new(rubyc_path())
            .arg("build")
            .arg(solution)
            .arg("-o")
            .arg(&binary),
    );
    assert_eq!(
        outcome.code,
        0,
        "solution build failed:\n{}",
        outcome.stderr_str()
    );
    assert_eq!(run_binary(&binary), "MULTI|value=42");
}

#[test]
fn solution_rejects_missing_project_with_resolved_path() {
    let workspace = Workspace::new("missing");
    let solution = workspace.write(
        "workspace.rcs",
        r#"solution "Workspace"
[[project]]
path = "missing/app.rcp"
"#,
    );

    let outcome = exec(Command::new(rubyc_path()).arg("build").arg(solution));
    assert_ne!(outcome.code, 0);
    assert!(outcome.stderr_str().contains("missing/app.rcp"));
}

#[test]
fn solution_builds_multiple_projects_to_distinct_default_outputs() {
    let workspace = Workspace::new("multi_solution");
    for (directory, value) in [("first", 11), ("second", 22)] {
        workspace.write(
            &format!("{directory}/main.rc"),
            &format!(
                "namespace {directory} {{ class Program {{ int main() {{ printf(\"{directory}|value={{0}}\", {value}); return 0; }} }} }}"
            ),
        );
        workspace.write(
            &format!("{directory}/app.rcp"),
            &format!(
                r#"[meta]
name = "{directory}"
version = "1.0.0"
[build]
sources = ["main.rc"]
entry-point = "main"
"#
            ),
        );
    }
    let solution = workspace.write(
        "workspace.rcs",
        r#"solution "Workspace"
[[project]]
path = "first/app.rcp"
[[project]]
path = "second/app.rcp"
"#,
    );

    let outcome = exec(Command::new(rubyc_path()).arg("build").arg(solution));
    assert_eq!(
        outcome.code,
        0,
        "solution build failed:\n{}",
        outcome.stderr_str()
    );
    assert_eq!(run_binary(&workspace.0.join("first/app")), "first|value=11");
    assert_eq!(
        run_binary(&workspace.0.join("second/app")),
        "second|value=22"
    );
}

#[test]
fn solution_parallel_rci_only_emits_sidecars_without_images() {
    // Backfill shape: two shared libraries built with `--jobs 2
    // --rci-only` must emit only `out/<name>.rci` (no codegen, no
    // `.so`), proving the fragment path skips the backend entirely.
    let workspace = Workspace::new("scope_rci");
    for directory in ["scope_a", "scope_b"] {
        workspace.write(
            &format!("{directory}/lib.rc"),
            &format!("namespace {directory} {{ class Lib {{ public int M() {{ return 1; }} }} }}"),
        );
        workspace.write(
            &format!("{directory}/app.rcp"),
            &format!(
                r#"[meta]
name = "{directory}"
version = "1.0.0"
[target]
platform = "linux_x86_64"
language = "rubyc"
output = "shared"
[build]
sources = ["lib.rc"]
entry-point = ""
"#
            ),
        );
    }
    let solution = workspace.write(
        "workspace.rcs",
        r#"solution "Scope"
[[project]]
path = "scope_a/app.rcp"
[[project]]
path = "scope_b/app.rcp"
"#,
    );

    let outcome = exec(
        Command::new(rubyc_path())
            .arg("build")
            .arg(&solution)
            .arg("--jobs")
            .arg("2")
            .arg("--rci-only")
            .arg("-P")
            .arg("-Q"),
    );
    assert_eq!(
        outcome.code,
        0,
        "parallel rci-only build failed:\n{}",
        outcome.stderr_str()
    );
    for directory in ["scope_a", "scope_b"] {
        let out = workspace.0.join(format!("{directory}/out"));
        assert!(
            out.join(format!("{directory}.rci")).is_file(),
            "missing sidecar for {directory}"
        );
        assert!(
            !out.join(format!("{directory}.so")).exists(),
            "rci-only must not emit an image for {directory}"
        );
    }
}
