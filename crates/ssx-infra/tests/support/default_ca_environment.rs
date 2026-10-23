/// Default-pool qualification is separate from caller-configured CA profiles.
/// Re-exec the exact case without mutating the parallel test process environment.
pub(crate) fn default_ca_test_environment(test: &str) -> bool {
    if std::env::var_os("SSL_CERT_FILE").is_none() && std::env::var_os("SSL_CERT_DIR").is_none() {
        return false;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env_remove("SSL_CERT_FILE")
        .env_remove("SSL_CERT_DIR")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success()
            && stdout
                .lines()
                .any(|line| line == format!("test {test} ... ok")),
        "default-CA exact child case failed or did not execute: {test}: {stdout} {}",
        String::from_utf8_lossy(&output.stderr)
    );
    true
}
