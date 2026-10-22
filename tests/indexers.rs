//! Indexers: one parameter and several.
//!
//! A multi-parameter indexer (`this[int r, int c]`) used to be REPORTED
//! (E0041) because the AST held a single index expression and the extra
//! arguments were parsed and dropped — the read then answered the wrong
//! slot (measured: 0 where `i + j` gives 7). The AST now carries the extra
//! arguments, the getter binds every parameter in order, and the setter
//! matches the index-parameter count (the value is not one of them).
//!
//! Each case is a whole program whose `Main` returns the value C# requires.

use crate::tests::common::{run, rubyc_path};
use std::process::Command;

#[track_caller]
fn exit_code(src: &str) -> i32 {
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["run", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert!(!out.timed_out, "the program hung:\n{src}");
    assert!(
        !out.stderr_str().contains("panicked at"),
        "the compiler panicked:\n{src}\n{}",
        out.stderr_str()
    );
    out.code
}

#[test]
fn two_parameter_indexer_reads_and_writes() {
    let src = r#"
namespace t;
class Grid {
    private int[] cells;
    public Grid() { cells = new int[16]; }
    public int this[int r, int c] {
        get { return cells[r * 4 + c]; }
        set { cells[r * 4 + c] = value; }
    }
}
class C {
    int Main() {
        Grid g = new Grid();
        g[1, 2] = 42;
        g[0, 3] = 7;
        return g[1, 2] + g[0, 3];
    }
}
"#;
    assert_eq!(exit_code(src), 49, "two indices must bind to two parameters");
}

#[test]
fn three_parameter_indexer_binds_in_order() {
    let src = r#"
namespace t;
class Cube {
    public int this[int a, int b, int c] {
        get { return a * 100 + b * 10 + c; }
        set { }
    }
}
class C {
    int Main() { Cube q = new Cube(); return q[1, 2, 3]; }
}
"#;
    assert_eq!(exit_code(src), 123, "parameters bind left to right");
}

#[test]
fn one_parameter_indexer_still_works() {
    let src = r#"
namespace t;
class G {
    public static int V;
    public int this[int c] { get { return V; } set { V = value; } }
}
class C {
    int Main() { G g = new G(); g[1] = 42; return g[1]; }
}
"#;
    assert_eq!(exit_code(src), 42, "the single-parameter path must not regress");
}

#[test]
fn indexer_argument_is_evaluated_once() {
    let src = r#"
namespace t;
class Counted {
    public static int calls;
    public static int Next() { calls = calls + 1; return calls; }
}
class G {
    private int[] cells;
    public G() { cells = new int[8]; }
    public int this[int i] { get { return cells[i]; } set { cells[i] = value; } }
}
class C {
    int Main() {
        G g = new G();
        g[Counted.Next()] = 5;
        return g[1] * 10 + Counted.calls;
    }
}
"#;
    assert_eq!(exit_code(src), 51, "the index expression runs once on the store path");
}
