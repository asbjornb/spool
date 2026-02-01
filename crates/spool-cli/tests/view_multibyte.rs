use assert_cmd::Command;

#[test]
fn view_multibyte_spool_does_not_panic() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/multibyte.spool"
    );

    Command::cargo_bin("spool")
        .unwrap()
        .args(["view", fixture])
        .assert()
        .success();
}
