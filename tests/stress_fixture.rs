//! Contract tests for the long-form C# 11 stress fixture.
//!
//! These tests deliberately stop short of claiming that RubyC can compile the
//! fixture.  They protect the fixture itself and make the current front-end
//! boundary executable while language support is added incrementally.

use rubyc::preprocessor::preprocess;

const STRESS: &str = include_str!("fixtures/stress_test.rc");

#[test]
fn stress_fixture_keeps_its_cross_stage_feature_contract() {
    assert!(STRESS.lines().count() >= 10_000, "the mega fixture was truncated");

    // Parser/front-end surfaces represented by the fixture.
    for syntax in [
        "public enum TestEnum : int",
        "public struct TestStruct",
        "public interface IValueProvider",
        "public record TestRecord",
        " switch ",
        "foreach (",
        "try\n",
        "catch (",
        "finally\n",
        "yield return",
        "=>",
        "??=",
        " with {",
    ] {
        assert!(STRESS.contains(syntax), "missing parser feature sentinel: {syntax:?}");
    }

    // Native lowering/runtime surfaces represented by the fixture.
    for syntax in [
        "new TestClass(",
        "operator +(",
        "checked(",
        "stackalloc",
        "ref int",
        "int[]",
        "printf(",
    ] {
        assert!(STRESS.contains(syntax), "missing native feature sentinel: {syntax:?}");
    }

    // Output is a machine-checkable trace, not prose.  Keep enough records to
    // catch accidental replacement with the much smaller historical fixture.
    assert!(STRESS.matches("printf(\"TRACE|").count() >= 500);
    assert!(STRESS.contains("TRACE|PROGRAM|MAIN|BEGIN"));
    assert!(STRESS.contains("TRACE|PROGRAM|MAIN|FINAL|accumulator={0}"));
    assert!(STRESS.contains("TRACE|PROGRAM|MAIN|END|return=0"));
}

#[test]
fn lowercase_conditional_directives_are_supported() {
    let source = "#define ON\n#if ON\nnamespace kept { }\n#else\n#error wrong branch\n#endif\n";
    let output = preprocess(source, &[], "lowercase.rc").expect("lowercase directives");

    assert!(output.source.contains("namespace kept"));
    assert!(output.diagnostics.is_empty());
}

#[test]
fn stress_fixture_csharp_directives_are_accepted_no_ops() {
    // `#nullable`, `#region`, `#endregion` are accepted no-ops, so the whole
    // fixture preprocesses cleanly.
    let output = preprocess(STRESS, &[], "stress_test.rc")
        .expect("#nullable/#region/#endregion are accepted no-ops");
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:#?}",
        output.diagnostics
    );

    // The active `#if COMPILER_MEGA_FIXTURE` branch survives; the inactive
    // branches are blanked.
    assert!(output.source.contains("PreprocessorBranchValue = 1"));
    assert!(!output.source.contains("PreprocessorBranchValue = 2"));
    assert!(!output.source.contains("PreprocessorBranchValue = 3"));

    // The `#error` lives in the inactive `#else` branch and must never fire.
    assert!(
        !output.source.contains("COMPILER_MEGA_FIXTURE must be defined"),
        "inactive-branch #error text survived preprocessing"
    );
}

#[test]
fn legacy_stress_expectations_cannot_be_mistaken_for_this_fixture() {
    for stale_marker in ["Operation.Add", "Vector2D constructor", "Factorial entered"] {
        assert!(
            !STRESS.contains(stale_marker),
            "historical stress marker unexpectedly entered the C# 11 fixture: {stale_marker}"
        );
    }
}
