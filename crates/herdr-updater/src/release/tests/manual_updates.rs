use super::*;

#[test]
fn manual_host_downloads_require_a_nonempty_verification_key() {
    assert!(matches!(
        host::release_identity("20261028.1-shinya.1", Some("")),
        Err(Error::LocalBuild)
    ));
}

fn listing(version: &str, prerelease: bool) -> serde_json::Value {
    serde_json::json!({
        "tag_name": format!("v{version}"), "draft": false, "prerelease": prerelease,
        "body": "Fork release notes", "assets": [{
            "name": "Herdr.zip", "size": 3,
            "browser_download_url": format!(
                "https://github.com/senshinya/herdr-gpui/releases/download/v{version}/Herdr.zip"
            )
        }]
    })
}

#[test]
fn manual_checks_offer_newer_fork_releases_without_fetching_or_trusting_binaries()
-> anyhow::Result<()> {
    let mut requests = Vec::new();
    let found = manual::check_with(
        "20261028.1-shinya.1",
        UpdateChannel::Stable,
        &AtomicBool::new(false),
        |url, _| {
            requests.push(url.to_owned());
            serde_json::to_vec(&listing("20261028.1-shinya.2", false)).map_err(Error::ReleaseJson)
        },
    )?
    .context("new fork release")?;
    assert_eq!(found.version, "20261028.1-shinya.2");
    assert_eq!(found.notes, "Fork release notes");
    assert_eq!(
        requests,
        ["https://api.github.com/repos/senshinya/herdr-gpui/releases/latest"]
    );
    assert_eq!(
        release_page(&found.version),
        "https://github.com/senshinya/herdr-gpui/releases/tag/v20261028.1-shinya.2"
    );
    Ok(())
}

#[test]
fn same_or_older_fork_versions_are_current() -> anyhow::Result<()> {
    for version in ["20261028.1-shinya.1", "20261028.1-shinya.9"] {
        let found = manual::check_with(
            "20261028.1-shinya.9",
            UpdateChannel::Stable,
            &AtomicBool::new(false),
            |_, _| serde_json::to_vec(&listing(version, false)).map_err(Error::ReleaseJson),
        )?;
        assert!(found.is_none());
    }
    Ok(())
}

#[test]
fn manual_checks_refuse_foreign_versions_and_download_origins() {
    for mut release in [
        listing("20261029.1", false),
        listing("20261028.1-shinya.2", false),
    ] {
        if release["tag_name"] == "v20261028.1-shinya.2" {
            release["assets"][0]["browser_download_url"] = "https://example.com/Herdr.zip".into();
        }
        assert!(
            manual::check_with(
                "20261028.1-shinya.1",
                UpdateChannel::Stable,
                &AtomicBool::new(false),
                |_, _| serde_json::to_vec(&release).map_err(Error::ReleaseJson),
            )
            .is_err()
        );
    }
}

#[test]
fn a_cancelled_manual_check_never_starts_a_request() {
    assert!(matches!(
        manual::check_with(
            "20261028.1-shinya.1",
            UpdateChannel::Stable,
            &AtomicBool::new(true),
            |_, _| panic!("cancelled checks cannot fetch"),
        ),
        Err(Error::Cancelled)
    ));
}

#[test]
fn beta_checks_compare_fork_revisions_and_refuse_late_cancellation() -> anyhow::Result<()> {
    let cancel = AtomicBool::new(false);
    let listing = serde_json::to_vec(&[
        listing("20261028.1-shinya.9", false),
        listing("20261028.1-shinya.10", true),
    ])?;
    let found = manual::check_with(
        "20261028.1-shinya.1",
        UpdateChannel::Beta,
        &cancel,
        |url, _| {
            assert_eq!(
                url,
                "https://api.github.com/repos/senshinya/herdr-gpui/releases?per_page=10"
            );
            Ok(listing.clone())
        },
    )?
    .context("newer beta")?;
    assert_eq!(found.version, "20261028.1-shinya.10");
    assert!(matches!(
        manual::check_with(
            "20261028.1-shinya.1",
            UpdateChannel::Beta,
            &cancel,
            |_, _| {
                cancel.store(true, Ordering::Release);
                Ok(listing.clone())
            },
        ),
        Err(Error::Cancelled)
    ));
    Ok(())
}
