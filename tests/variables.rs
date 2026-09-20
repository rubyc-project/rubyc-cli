//! Variables, integer arithmetic, and interpolated printf: parser shapes,
//! diagnostics, bytecode survival, and executed results.

use rubyc::bytecode::traits::{FromBytecode, ToBytecode};
use rubyc::models::compare::StructuralEq;
use rubyc::parser;

// ---------- helpers ----------

fn parse_expr(source: &str) -> rubyc::models::Expr {
    // Wrap an expression into a statement context to reuse public parse().
    let src = format!("namespace t; class c {{ int main() {{ int z = {source}; }} }}");
    let unit = parser::parse(&src).unwrap();
    let ns = &unit.namespaces[0];
    let rubyc::models::Member::Class(class) = &ns.members[0] else {
        panic!("class")
    };
    let rubyc::models::ClassMember::Method(method) = &class.members[0] else {
        panic!("method")
    };
    match &method.body.as_ref().unwrap().stmts[0] {
        rubyc::models::Stmt::Let { init, .. } => init.clone(),
        other => panic!("expected Let, got {other:?}"),
    }
}

/// Run a program through the CLI binary and return its stdout.
/// (JIT writes straight to process stdout; spawning lets us capture it.)
fn run_source(source: &str) -> String {
    let mut cmd = std::process::Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run");
    let out = crate::tests::common::run(&mut cmd, source.as_bytes());
    assert!(
        !out.timed_out,
        "program hung (killed after {}s): {}",
        crate::tests::common::EXEC_TIMEOUT.as_secs(),
        source
    );
    assert!(
        out.code == 0,
        "program failed (exit {}): {}\nstderr: {}",
        out.code,
        source,
        out.stderr_str()
    );
    out.stdout_str()
}

fn program(body: &str) -> String {
    format!("namespace t; class c {{ int main() {{ {body} }} }}")
}

macro_rules! exec_case {
    ($name:ident, $body:expr, $expected:expr) => {
        #[test]
        fn $name() {
            assert_eq!(run_source(&program($body)), $expected);
        }
    };
}

// ---------- expression parsing ----------

#[test]
fn let_statement_shape() {
    let src = "namespace t; class c { int main() { int MyNumber = 34; } }";
    let unit = parser::parse(src).unwrap();
    let ns = &unit.namespaces[0];
    let rubyc::models::Member::Class(class) = &ns.members[0] else {
        panic!("class")
    };
    let rubyc::models::ClassMember::Method(method) = &class.members[0] else {
        panic!("method")
    };
    match &method.body.as_ref().unwrap().stmts[0] {
        rubyc::models::Stmt::Let {
            type_name,
            name,
            init,
            ..
        } => {
            assert_eq!((type_name.as_str(), name.as_str()), ("int", "MyNumber"));
            assert_eq!(*init, parse_expr("34"));
        }
        other => panic!("expected Let, got {other:?}"),
    }
}

#[test]
fn assignment_statement_shape() {
    let src = "namespace t; class c { int main() { int x = 1; x = x + 4; } }";
    let unit = parser::parse(src).unwrap();
    let ns = &unit.namespaces[0];
    let rubyc::models::Member::Class(class) = &ns.members[0] else {
        panic!("class")
    };
    let rubyc::models::ClassMember::Method(method) = &class.members[0] else {
        panic!("method")
    };
    match &method.body.as_ref().unwrap().stmts[1] {
        rubyc::models::Stmt::Assign { name, value, .. } => {
            assert_eq!(name, "x");
            assert!(matches!(
                value,
                rubyc::models::Expr::Binary {
                    op: rubyc::models::BinOp::Add,
                    ..
                }
            ));
        }
        other => panic!("expected Assign, got {other:?}"),
    }
}

#[test]
fn mul_binds_tighter_than_add() {
    let e = parse_expr("2 + 3 * 4");
    let rubyc::models::Expr::Binary { op, rhs, .. } = &e else {
        panic!()
    };
    assert!(matches!(op, rubyc::models::BinOp::Add));
    let rubyc::models::Expr::Binary { op, .. } = rhs.as_ref() else {
        panic!()
    };
    assert!(matches!(op, rubyc::models::BinOp::Mul));
}

#[test]
fn parens_override_precedence() {
    let e = parse_expr("(2 + 3) * 4");
    let rubyc::models::Expr::Binary { op, lhs, .. } = &e else {
        panic!()
    };
    assert!(matches!(op, rubyc::models::BinOp::Mul));
    let rubyc::models::Expr::Binary { op, .. } = lhs.as_ref() else {
        panic!()
    };
    assert!(matches!(op, rubyc::models::BinOp::Add));
}

#[test]
fn operators_are_left_associative() {
    // 10 - 4 - 3 == (10 - 4) - 3
    let e = parse_expr("10 - 4 - 3");
    let rubyc::models::Expr::Binary { op, lhs, rhs, .. } = &e else {
        panic!()
    };
    assert!(matches!(op, rubyc::models::BinOp::Sub));
    assert_eq!(
        **rhs,
        rubyc::models::Expr::Literal(rubyc::models::Literal::Int(3))
    );
    let rubyc::models::Expr::Binary { op, .. } = lhs.as_ref() else {
        panic!()
    };
    assert!(matches!(op, rubyc::models::BinOp::Sub));
}

#[test]
fn unary_minus_parses() {
    let e = parse_expr("-5");
    assert!(matches!(
        e,
        rubyc::models::Expr::Unary {
            op: rubyc::models::UnOp::Neg,
            ..
        }
    ));
}

#[test]
fn deeply_nested_parens_parse() {
    let e = parse_expr("5 + ((7 + 5) * (9))");
    assert!(matches!(
        e,
        rubyc::models::Expr::Binary {
            op: rubyc::models::BinOp::Add,
            ..
        }
    ));
}

// ---------- diagnostics ----------

/// Return the compile-time diagnostic for `body`, or a sentinel if the
/// code actually compiles.
///
/// Compile-only (`ir::lower`), never executing: executing here (via
/// `jit_run`) would run `exit_group` and kill the test harness for any
/// code that actually compiles.
fn expect_error(body: &str) -> String {
    match parser::parse(&program(body)) {
        Ok(unit) => match rubyc::native::ir::lower(&unit) {
            Err(e) => e.to_string(),
            Ok(_) => String::from("<no compile error: code lowered successfully>"),
        },
        Err(diagnostics) => diagnostics
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
            .join("; "),
    }
}

#[test]
fn undeclared_variable_is_reported() {
    let message = expect_error("x = 1;");
    assert!(message.contains("undeclared"), "{message}");
}

#[test]
fn reading_undeclared_variable_is_reported() {
    let message = expect_error("printf(\"{0}\", nope);");
    assert!(message.contains("undeclared"), "{message}");
}

#[test]
fn duplicate_declaration_is_reported() {
    let message = expect_error("int x = 1; int x = 2;");
    assert!(message.contains("already declared"), "{message}");
}

#[test]
fn unknown_type_is_rejected() {
    let message = expect_error("widget x = 1;");
    assert!(message.contains("type"), "{message}");
}

#[test]
fn float_type_rejected_with_guidance() {
    let message = expect_error("double d = 1;");
    assert!(message.contains("floating point"), "{message}");
}

#[test]
fn placeholder_out_of_range_is_reported() {
    let message = expect_error("printf(\"{5}\", 1);");
    assert!(message.contains("only"), "{message}");
}

#[test]
fn calling_unknown_function_is_reported() {
    let message = expect_error("frobnicate(1);");
    assert!(message.contains("frobnicate"), "{message}");
}

// ---------- bytecode round-trip ----------

#[test]
fn variables_survive_bytecode_round_trip() {
    let src =
        program("int MyNumber = 34; MyNumber = MyNumber + 4; printf(\"{0} {1}\", MyNumber, -3);");
    let unit = parser::parse(&src).unwrap();
    let bytes = unit.to_bytecode();
    let decoded = rubyc::models::CompilationUnit::from_bytecode(&bytes).unwrap();
    assert!(decoded.structural_eq(&unit));
}

// ---------- executed arithmetic (via CLI, real machine code) ----------

exec_case!(
    declare_and_print,
    r#"int MyNumber = 34; printf("The number is {0}", MyNumber);"#,
    "The number is 34"
);
exec_case!(
    reassign_add_literal,
    "int MyNumber = 34; MyNumber = MyNumber + 4; printf(\"{0}\", MyNumber);",
    "38"
);
exec_case!(
    assign_var_plus_literal,
    "int MyNumber1 = 10; int MyNumber2 = 20; MyNumber1 = MyNumber2 + 4; printf(\"{0}\", MyNumber1);",
    "24"
);
exec_case!(
    assign_var_plus_var,
    "int MyNumber1 = 10; int MyNumber2 = 20; int MyNumber3 = 30; MyNumber1 = MyNumber2 + MyNumber3; printf(\"{0}\", MyNumber1);",
    "50"
);

exec_case!(
    subtraction,
    "int a = 10; int b = 4; printf(\"{0}\", a - b);",
    "6"
);
exec_case!(
    multiplication,
    "int a = 6; int b = 7; printf(\"{0}\", a * b);",
    "42"
);
exec_case!(division_truncates_down, "printf(\"{0}\", 7 / 2);", "3");
exec_case!(
    division_negative_truncates_toward_zero,
    "int a = -7; printf(\"{0}\", a / 2);",
    "-3"
);

exec_case!(precedence_mul_first, "printf(\"{0}\", 2 + 3 * 4);", "14");
exec_case!(
    paren_overrides_precedence,
    "printf(\"{0}\", (2 + 3) * 4);",
    "20"
);
exec_case!(
    left_associative_subtraction,
    "printf(\"{0}\", 10 - 4 - 3);",
    "3"
);
exec_case!(mixed_ops, "printf(\"{0}\", 10 - 2 * 3 + 4 / 2);", "6"); // 10-6+2

exec_case!(
    nested_double_parens,
    "int MyNumber1 = 0; int MyNumber2 = 10; int MyNumber3 = 30; MyNumber1 = 5 + ((MyNumber2 + 5) * MyNumber3); printf(\"{0}\", MyNumber1);",
    "455"
);
exec_case!(
    deeply_nested_parens,
    "printf(\"{0}\", ((1 + 2) * ((3 + 4) * (5 + 6))));",
    "231"
);
exec_case!(
    parens_around_variables,
    "int a = 2; int b = 3; int c = 4; printf(\"{0}\", ((a + b)) * ((c + 1)));",
    "25"
);

exec_case!(
    negative_result_prints_minus_sign,
    "printf(\"{0}\", 3 - 10);",
    "-7"
);
exec_case!(unary_minus_literal, "int x = -5; printf(\"{0}\", x);", "-5");
exec_case!(unary_minus_expression, "printf(\"{0}\", -(3 + 4));", "-7");
exec_case!(double_negation, "int x = 5; printf(\"{0}\", -(-x));", "5");

exec_case!(
    large_i64_values,
    "printf(\"{0}\", 100000 * 100000);",
    "10000000000"
);
exec_case!(zero_value, "int z = 0; printf(\"{0}\", z);", "0");
exec_case!(value_zero_from_math, "printf(\"{0}\", 5 - 5);", "0");

// interpolated printf variants
exec_case!(
    user_sample_program,
    r#"
    int MyNumber = 34;
    MyNumber = MyNumber + 4;
    int MyNumber1 = 10;
    int MyNumber2 = 20;
    int MyNumber3 = 30;
    MyNumber1 = MyNumber2 + 4;
    MyNumber1 = MyNumber2 + MyNumber3;
    printf("The number is {0}", MyNumber1);
    printf("The number is {0} {1}", MyNumber1, MyNumber2);
"#,
    "The number is 50The number is 50 20"
);
exec_case!(
    two_placeholders,
    "int a = 1; int b = 2; printf(\"{0} {1}\", a, b);",
    "1 2"
);
exec_case!(
    reversed_placeholders,
    "int a = 1; int b = 2; printf(\"{1} {0}\", a, b);",
    "2 1"
);
exec_case!(
    same_placeholder_twice,
    "int a = 7; printf(\"{0} and {0}\", a);",
    "7 and 7"
);
exec_case!(
    expression_as_argument,
    "int a = 5; printf(\"{0}\", a + 1);",
    "6"
);
exec_case!(escaped_braces, "printf(\"{{literal}}\");", "{literal}");
exec_case!(
    text_only_printf,
    "printf(\"no args here\");",
    "no args here"
);

// integer type family accepted this milestone (i64 slots)
exec_case!(
    long_type,
    "long big = 4000000000; printf(\"{0}\", big);",
    "4000000000"
);
exec_case!(
    byte_type,
    "byte small = 200; printf(\"{0}\", small);",
    "200"
);
exec_case!(sbyte_type, "sbyte s = -100; printf(\"{0}\", s);", "-100");
exec_case!(
    short_type,
    "short sh = 12345; printf(\"{0}\", sh);",
    "12345"
);
exec_case!(
    ushort_type,
    "ushort us = 60000; printf(\"{0}\", us);",
    "60000"
);
exec_case!(
    uint_type,
    "uint ui = 3000000000; printf(\"{0}\", ui);",
    "3000000000"
);
exec_case!(
    ulong_type,
    "ulong ul = 9000000000; printf(\"{0}\", ul);",
    "9000000000"
);
exec_case!(nint_type, "nint ni = 42; printf(\"{0}\", ni);", "42");
