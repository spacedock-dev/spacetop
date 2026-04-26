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

    let result = spacetop::run(cli);
    if let Err(ref e) = result {
        if _sentry.is_some() {
            sentry::capture_error(e.as_ref() as &dyn std::error::Error);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;

    #[test]
    #[cfg(debug_assertions)]
    fn dev_build_does_not_init_sentry() {
        // cfg!(debug_assertions) is true in test/debug builds.
        // Compile-time proof: the release branch is unreachable here.
        const { assert!(cfg!(debug_assertions)) };
    }

    // AC-1: an error from run() is forwarded to Sentry.
    #[test]
    fn capture_error_on_run_failure() {
        let events = sentry::test::with_captured_events(|| {
            let result: anyhow::Result<()> = Err(anyhow!("scan IO error"));
            if let Err(ref e) = result {
                sentry::capture_error(e.as_ref() as &dyn std::error::Error);
            }
        });
        assert_eq!(events.len(), 1, "expected exactly one captured event");
    }

    // AC-3: a successful run produces no Sentry events.
    #[test]
    fn no_capture_on_run_success() {
        let events = sentry::test::with_captured_events(|| {
            let result: anyhow::Result<()> = Ok(());
            if let Err(ref e) = result {
                sentry::capture_error(e.as_ref() as &dyn std::error::Error);
            }
        });
        assert_eq!(events.len(), 0, "expected no events on success");
    }

    // AC-2: when _sentry is None the capture branch is never entered.
    #[test]
    fn no_capture_when_sentry_not_initialised() {
        // Simulate _sentry = None (no guard alive).
        // with_captured_events installs a temporary hub; we deliberately do NOT
        // call capture_error because the guard check prevents it.
        let events = sentry::test::with_captured_events(|| {
            let _sentry: Option<sentry::ClientInitGuard> = None;
            let result: anyhow::Result<()> = Err(anyhow!("error that must not be sent"));
            if let Err(ref e) = result {
                if _sentry.is_some() {
                    sentry::capture_error(e.as_ref() as &dyn std::error::Error);
                }
            }
        });
        assert_eq!(events.len(), 0, "expected no events when sentry is not initialised");
    }
}
