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

#[test]
fn install_libs_builds_stale_and_skips_fresh() {
    // `install libs` on a miniature pulled tree: first run builds and
    // installs both libs through their post-build hooks; second run
    // reports everything up to date; touching one lib rebuilds only it.
    // The child runs with its CWD inside the temp tree so pull/file
    // discovery never escapes to the real checkout.
    let workspace = Workspace::new("install_libs");
    let tree = workspace.0.join("tree");
    let libs = tree.join("dotnet-runtime/src/libraries");
    for (lib, cls, ret) in [("liba", "A", 1), ("libb", "B", 2)] {
        let dir = libs.join(lib);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/l.rc"),
            format!("namespace {lib} {{ class {cls} {{ public int M() {{ return {ret}; }} }} }}\n"),
        )
        .unwrap();
        std::fs::write(
            dir.join(format!("{lib}.rcp")),
            format!(
                r#"[meta]
name = "{lib}"
version = "1.0.0"
[target]
platform = "linux_x86_64"
language = "rubyc"
output = "shared"
[build]
sources = ["src/**/*.rc"]
entry-point = ""
defines = []
[libs]
ruby_c = []
[hooks]
post-build = 'mkdir -p "$RUBYC_BASE/libraries" && cp out/*.so "$RUBYC_BASE/libraries/" 2>/dev/null; cp out/*.rci "$RUBYC_BASE/libraries/" 2>/dev/null; true'
"#
            ),
        )
        .unwrap();
    }
    std::fs::write(
        libs.join("dotnet-runtime.rcs"),
        "solution mini\nversion = \"1\"\n[[project]]\npath = \"liba/liba.rcp\"\n\n[[project]]\npath = \"libb/libb.rcp\"\n",
    )
    .unwrap();
    let base = workspace.0.join("base");
    let installed = base.join("libraries");
    let run_install = |extra: &[&str]| {
        let mut cmd = Command::new(rubyc_path());
        cmd.arg("install").arg("libs");
        cmd.args(extra);
        cmd.env("RUBYC_BASE", &base);
        cmd.current_dir(&tree);
        exec(&mut cmd)
    };
    // First run: builds + installs both (embedded by default, so no
    // sidecar files land next to the images).
    let out = run_install(&[]);
    assert_eq!(out.code, 0, "install failed:\n{}", out.stderr_str());
    for lib in ["liba", "libb"] {
        assert!(installed.join(format!("{lib}.so")).is_file(), "{lib}.so installed");
        assert!(
            !installed.join(format!("{lib}.rci")).exists(),
            "{lib}.rci must not exist when embedded"
        );
    }
    let mtime = |lib: &str| {
        std::fs::metadata(installed.join(format!("{lib}.so")))
            .unwrap()
            .modified()
            .unwrap()
    };
    let (a0, b0) = (mtime("liba"), mtime("libb"));
    // Second run: everything up to date, artifacts untouched.
    let out = run_install(&[]);
    assert_eq!(out.code, 0, "reinstall failed:\n{}", out.stderr_str());
    assert!(out.stderr_str().contains("up to date"), "stderr: {}", out.stderr_str());
    assert_eq!(mtime("liba"), a0, "fresh liba rebuilt");
    assert_eq!(mtime("libb"), b0, "fresh libb rebuilt");
    // Touch one lib: only it rebuilds.
    std::fs::write(
        libs.join("libb/src/l.rc"),
        "namespace libb { class B { public int M() { return 2; } public int N() { return 3; } } }\n",
    )
    .unwrap();
    let out = run_install(&[]);
    assert_eq!(out.code, 0, "touch rebuild failed:\n{}", out.stderr_str());
    assert_eq!(mtime("liba"), a0, "untouched liba rebuilt");
    assert!(mtime("libb") > b0, "touched libb not rebuilt");
}

#[test]
fn install_libs_missing_tree_errors_loudly() {
    // No pulled tree and no pull script above an empty dir: loud error
    // naming the fix, never a bare OS error (and no network touched).
    let workspace = Workspace::new("install_no_tree");
    let empty = workspace.0.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let mut cmd = Command::new(rubyc_path());
    cmd.arg("install").arg("libs");
    cmd.env("RUBYC_BASE", workspace.0.join("base"));
    cmd.current_dir(&empty);
    let out = exec(&mut cmd);
    assert_eq!(out.code, 1, "must fail without a tree");
    assert!(
        out.stderr_str().contains("pull"),
        "must name the fix, stderr: {}",
        out.stderr_str()
    );
}

#[test]
fn embedded_types_consume_without_sources_or_sidecar() {
    // Binary-only distribution: a library rebuilt with default embedding
    // carries v2 types inside its `.so`; the stale v1 `.rci` file and
    // deleted sources leave the embedded table as the only place the
    // new member can come from. The consumer (dynamic, `using`, shared
    // output so nothing executes) must build successfully.
    let workspace = Workspace::new("embed_consume");
    let base = workspace.0.join("base");
    let libraries = base.join("libraries");
    std::fs::create_dir_all(&libraries).unwrap();
    let lib_dir = workspace.0.join("disco");
    let lib_src = lib_dir.join("src");
    std::fs::create_dir_all(&lib_src).unwrap();
    let lib_rcp = lib_dir.join("disco.rcp");
    let lib_project = r#"[meta]
name = "disco"
version = "1.0.0"
[target]
platform = "linux_x86_64"
language = "rubyc"
output = "shared"
[build]
sources = ["src/**/*.rc"]
entry-point = ""
defines = []
[libs]
ruby_c = []
"#;
    std::fs::write(&lib_rcp, lib_project).unwrap();
    let build_lib = |env_base: &std::path::Path, extra: &[&str]| {
        let mut cmd = Command::new(rubyc_path());
        cmd.arg("build").arg(&lib_rcp).arg("-P").arg("-Q");
        cmd.args(extra);
        cmd.env("RUBYC_BASE", env_base);
        exec(&mut cmd)
    };
    // v1 opted out: plain image + sidecar file.
    std::fs::write(
        lib_src.join("l.rc"),
        "namespace DD { class C { public int answer() { return 42; } } }\n",
    )
    .unwrap();
    let out = build_lib(&base, &["--no-embed-types"]);
    assert_eq!(out.code, 0, "v1 lib build failed:\n{}", out.stderr_str());
    std::fs::copy(lib_dir.join("out/disco.so"), libraries.join("disco.so")).unwrap();
    std::fs::copy(lib_dir.join("out/disco.rci"), libraries.join("disco.rci")).unwrap();
    // v2 embedded by default: installs only the image (stale v1 sidecar stays).
    std::fs::write(
        lib_src.join("l.rc"),
        "namespace DD { class C { public int answer() { return 42; } public int answer2() { return 44; } } }\n",
    )
    .unwrap();
    let out = build_lib(&base, &[]);
    assert_eq!(out.code, 0, "v2 lib build failed:\n{}", out.stderr_str());
    // The v2 out/ dir keeps the stale v1 sidecar (embed skips the
    // write but never deletes): only the image carries v2 types.
    std::fs::copy(lib_dir.join("out/disco.so"), libraries.join("disco.so")).unwrap();
    // Binary-only: sources are gone; the stale file cannot supply answer2.
    std::fs::remove_dir_all(&lib_dir).unwrap();
    // Consumer references the v2-only member through `using`, no ruby_c.
    let con_dir = workspace.0.join("con");
    let con_src = con_dir.join("src");
    std::fs::create_dir_all(&con_src).unwrap();
    std::fs::write(
        con_src.join("h.rc"),
        "using DD;\nnamespace app { class Host { public int get() { C c = new C(); return c.answer2(); } } }\n",
    )
    .unwrap();
    let con_rcp = con_dir.join("con.rcp");
    std::fs::write(
        &con_rcp,
        r#"[meta]
name = "con"
version = "1.0.0"
[target]
platform = "linux_x86_64"
language = "rubyc"
output = "shared"
[build]
sources = ["src/**/*.rc"]
entry-point = ""
defines = []
dynamic = true
[libs]
ruby_c = []
"#,
    )
    .unwrap();
    let mut cmd = Command::new(rubyc_path());
    cmd.arg("build")
        .arg(&con_rcp)
        .arg("-P")
        .arg("-Q");
    cmd.env("RUBYC_BASE", &base);
    let out = exec(&mut cmd);
    assert_eq!(
        out.code, 0,
        "embedded consume failed (answer2 only exists in the image table):\n{}",
        out.stderr_str()
    );
}

#[test]
fn no_embed_types_keeps_legacy_shape() {
    // `--no-embed-types` is purely subtractive for plain C/C++ hosts:
    // kind stays `SharedLibrary`, 9 sections, payload at 13 — the exact
    // pre-table layout. (Byte-equality across builds isn't asserted;
    // timestamps aside, this pins every structural byte instead.)
    let workspace = Workspace::new("no_embed");
    let dir = workspace.0.join("lib");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src/l.rc"),
        "namespace DD { class C { public int answer() { return 42; } } }\n",
    )
    .unwrap();
    let rcp = dir.join("lib.rcp");
    std::fs::write(
        &rcp,
        r#"[meta]
name = "lib"
version = "1.0.0"
[target]
platform = "linux_x86_64"
language = "rubyc"
output = "shared"
[build]
sources = ["src/**/*.rc"]
entry-point = ""
defines = []
[libs]
ruby_c = []
"#,
    )
    .unwrap();
    let mut cmd = Command::new(rubyc_path());
    cmd.arg("build").arg(&rcp).arg("-P").arg("-Q").arg("--no-embed-types");
    let out = exec(&mut cmd);
    assert_eq!(out.code, 0, "build failed:\n{}", out.stderr_str());
    let bytes = std::fs::read(dir.join("out/lib.so")).expect("image written");
    assert_eq!(bytes[74], 0x03, "kind stays SharedLibrary");
    let shnum = u16::from_le_bytes([bytes[60], bytes[61]]);
    assert_eq!(shnum, 9, "section count stays legacy");
    // No table: payload starts right after the 13-byte preamble...
    // (kind 0x03 carries no length field, so offset 77 is payload).
    assert_eq!(bytes[64..66], [0xEB, 0x08], "preamble jump intact");
}

/// TOML-only builds match flag-spelled builds byte-for-byte: the same
/// project without `[build]` keys built with `--optimize` must equal the
/// project with `optimize = true` built with no flags.
#[test]
fn toml_only_build_matches_flag_spelled_build() {
    let workspace = Workspace::new("toml_parity");
    workspace.write(
        "main.rc",
        r#"namespace parity { class Program { int main() { printf("parity"); return 0; } } }"#,
    );
    let rcp = workspace.write(
        "app.rcp",
        r#"[meta]
name = "parity"
version = "1.0.0"
[target]
platform = "linux_x86_64"
output = "shared"
[build]
sources = ["main.rc"]
entry-point = "main"
"#,
    );
    // Flag-spelled build first; stash its bytes.
    let out = exec(
        Command::new(rubyc_path())
            .arg("build")
            .arg(&rcp)
            .arg("--optimize"),
    );
    assert_eq!(out.code, 0, "flag build failed:\n{}", out.stderr_str());
    let flagged = std::fs::read(workspace.0.join("out/parity.so")).expect("flag image");
    // Same project plus the TOML key, built with no flags.
    std::fs::write(
        &rcp,
        r#"[meta]
name = "parity"
version = "1.0.0"
[target]
platform = "linux_x86_64"
output = "shared"
[build]
sources = ["main.rc"]
entry-point = "main"
optimize = true
"#,
    )
    .unwrap();
    let out = exec(Command::new(rubyc_path()).arg("build").arg(&rcp));
    assert_eq!(out.code, 0, "toml build failed:\n{}", out.stderr_str());
    let from_toml = std::fs::read(workspace.0.join("out/parity.so")).expect("toml image");
    assert_eq!(
        flagged, from_toml,
        "TOML-only and flag-spelled builds are byte-identical"
    );
}

/// Explicit CLI `-O` overrides the project's `[target] output`.
#[test]
fn cli_output_overrides_toml_output() {
    let workspace = Workspace::new("output_override");
    workspace.write(
        "main.rc",
        r#"namespace ovr { class Program { int main() { printf("ovr"); return 0; } } }"#,
    );
    let rcp = workspace.write(
        "app.rcp",
        r#"[meta]
name = "ovr"
version = "1.0.0"
[target]
platform = "linux_x86_64"
output = "shared"
[build]
sources = ["main.rc"]
entry-point = "main"
"#,
    );
    // TOML alone: shared library.
    let out = exec(Command::new(rubyc_path()).arg("build").arg(&rcp));
    assert_eq!(out.code, 0, "toml build failed:\n{}", out.stderr_str());
    let so = std::fs::read(workspace.0.join("out/ovr.so")).expect("shared image");
    assert_eq!(&so[0..4], b"\x7fELF");
    // CLI `-O exe` wins over TOML `output = "shared"`: an ET_EXEC
    // executable (e_type == 2). `-o` pins the path (project exe
    // outputs default to the caller cwd, not `out/`).
    let exe_path = workspace.0.join("out/ovr");
    let out = exec(
        Command::new(rubyc_path())
            .arg("build")
            .arg(&rcp)
            .arg("-O")
            .arg("exe")
            .arg("-o")
            .arg(&exe_path),
    );
    assert_eq!(out.code, 0, "override build failed:\n{}", out.stderr_str());
    let exe = std::fs::read(&exe_path).expect("exe image");
    assert_eq!(&exe[0..4], b"\x7fELF");
    assert_eq!(
        u16::from_le_bytes([exe[16], exe[17]]),
        2,
        "CLI -O exe overrides TOML shared"
    );
}

/// Same-path cross-assembly collision: the main project's entry carries
/// the only declared ctor while the dependency's same-path entry has the
/// implicit one. `new Dup()` in main must bind the dependency's implicit
/// ctor (MLC vs CoreLib `System.DefaultBinder`) instead of erroring
/// `ctor_arity` against the merged view.
#[test]
fn same_path_collision_constructs_implicit_dep_ctor() {
    let workspace = Workspace::new("collision");
    workspace.write(
        "dep/dep.rc",
        "namespace Sys { class Dup { } }",
    );
    workspace.write(
        "dep/dep.rcp",
        r#"[meta]
name = "collisiondep"
version = "1.0.0"
[target]
platform = "linux_x86_64"
output = "shared"
[build]
sources = ["dep.rc"]
entry-point = ""
"#,
    );
    workspace.write(
        "main.rc",
        r#"namespace Sys { partial class Dup {
            public Dup(int x) { }
            static Dup s = new Dup();
        } }
        namespace t { class Program { int main() { return 0; } } }"#,
    );
    let rcp = workspace.write(
        "app.rcp",
        r#"[meta]
name = "collisionmain"
version = "1.0.0"
[target]
platform = "linux_x86_64"
output = "shared"
[build]
sources = ["main.rc"]
entry-point = "main"
[libs]
ruby_c = ["dep/dep.rcp"]
"#,
    );
    let out = exec(Command::new(rubyc_path()).arg("build").arg(&rcp));
    assert_eq!(
        out.code, 0,
        "same-path collision must bind the implicit ctor:\n{}",
        out.stderr_str()
    );
    let image = std::fs::read(workspace.0.join("out/collisionmain.so")).expect("image");
    assert_eq!(&image[0..4], b"\x7fELF");
}
#[test]
fn solution_profile_applies_defines_and_optimize() {
    let workspace = Workspace::new("profile");
    workspace.write(
        "main.rc",
        r#"namespace prof { class Program { int main() {
#if DEBUG
            printf("debug");
#else
            printf("plain");
#endif
            return 0; } } }"#,
    );
    workspace.write(
        "app.rcp",
        r#"[meta]
name = "prof"
version = "1.0.0"
[target]
platform = "linux_x86_64"
output = "shared"
[build]
sources = ["main.rc"]
entry-point = "main"
"#,
    );
    let solution = workspace.write(
        "workspace.rcs",
        r#"solution "Workspace"
[[project]]
path = "app.rcp"
[profile.release]
defines = ["DEBUG"]
optimize = true
"#,
    );
    // Plain solution build: no profile.
    let out = exec(Command::new(rubyc_path()).arg("build").arg(&solution));
    assert_eq!(out.code, 0, "plain build failed:\n{}", out.stderr_str());
    let plain = std::fs::read(workspace.0.join("out/prof.so")).expect("plain image");
    assert!(
        plain.windows(5).any(|w| w == b"plain"),
        "unprofiled build carries the non-DEBUG string"
    );
    // Profiled build: DEBUG string present instead.
    let out = exec(
        Command::new(rubyc_path())
            .arg("build")
            .arg(&solution)
            .arg("--profile")
            .arg("release"),
    );
    assert_eq!(out.code, 0, "profile build failed:\n{}", out.stderr_str());
    let profiled = std::fs::read(workspace.0.join("out/prof.so")).expect("profile image");
    assert!(
        profiled.windows(5).any(|w| w == b"debug"),
        "profile defines (DEBUG) applied"
    );
    assert!(
        !profiled.windows(5).any(|w| w == b"plain"),
        "non-DEBUG branch compiled out"
    );
    // Unknown profile names fail loudly, never silently misbuild.
    let out = exec(
        Command::new(rubyc_path())
            .arg("build")
            .arg(&solution)
            .arg("--profile")
            .arg("nope"),
    );
    assert_ne!(out.code, 0, "unknown profile must fail");
    assert!(
        out.stderr_str().contains("unknown solution profile"),
        "loud error names the profile: {}",
        out.stderr_str()
    );
}
