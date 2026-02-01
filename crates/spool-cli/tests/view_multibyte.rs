use assert_cmd::cargo::cargo_bin_cmd;

#[test]
fn view_multibyte_spool_does_not_panic() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/multibyte.spool"
    );

    cargo_bin_cmd!("spool")
        .args(["view", fixture])
        .assert()
        .success();
}
