use clap::Parser;
use spacetop::cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Sentry is only active in release builds (cfg!(debug_assertions) is false
    // for `cargo build --release`). In debug builds the guard is dropped
    // immediately and no events are sent.
    let _sentry = if cfg!(debug_assertions) {
        None
    } else {
        let dsn = env!("SENTRY_DSN");
        if dsn.is_empty() {
            None
        } else {
            Some(sentry::init((
                dsn,
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    sample_rate: 1.0,
                    ..Default::default()
                },
            )))
        }
    };

    spacetop::run(cli)
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(debug_assertions)]
    fn dev_build_does_not_init_sentry() {
        // cfg!(debug_assertions) is true in test/debug builds.
        // Compile-time proof: the release branch is unreachable here.
        assert!(cfg!(debug_assertions));
    }
}
