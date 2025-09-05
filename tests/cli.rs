use trycmd::TestCases;

#[test]
fn cli_coverage() {
    let cases = TestCases::new();
    // Ensure cigen resolves to compiled bin
    cases.register_bin("cigen", std::path::Path::new(env!("CARGO_BIN_EXE_cigen")));
    // Stable env
    cases.env("CIGEN_SKIP_CIRCLECI_CLI", "1");

    // Note: paths in outputs will be absolute to the CI runner; acceptable for snapshots.

    cases.case("tests/cmd/**/*.trycmd");
}
