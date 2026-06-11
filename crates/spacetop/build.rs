fn main() {
    // Forward SENTRY_DSN from the build environment into the compiled binary.
    // When absent, emit an empty string so env!("SENTRY_DSN") always compiles.
    let dsn = std::env::var("SENTRY_DSN").unwrap_or_default();
    println!("cargo:rustc-env=SENTRY_DSN={dsn}");
    // Re-run only when the variable changes, not on every build.
    println!("cargo:rerun-if-env-changed=SENTRY_DSN");
}
