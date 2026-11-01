//! Release discovery for fork builds that open the browser instead of installing.
use super::{
    Error, LATEST_URL, METADATA_LIMIT, RELEASE_LIST_LIMIT, RELEASES_URL, RequestProfile, Result,
    UpdateChannel, cancelled, fork_version, parse_release, parse_releases, parse_version,
    read_bounded, request,
};
use std::sync::atomic::AtomicBool;

pub(crate) struct Release {
    pub(crate) version: String,
    pub(crate) notes: String,
}

pub(crate) fn check(
    current: &str,
    channel: UpdateChannel,
    cancel: &AtomicBool,
) -> Result<Option<Release>> {
    check_with(current, channel, cancel, |url, limit| {
        read_bounded(
            request(url, RequestProfile::Metadata, cancel)?
                .into_body()
                .into_reader(),
            limit,
            cancel,
        )
    })
}

pub(super) fn check_with(
    current: &str,
    channel: UpdateChannel,
    cancel: &AtomicBool,
    mut fetch: impl FnMut(&str, usize) -> Result<Vec<u8>>,
) -> Result<Option<Release>> {
    cancelled(cancel)?;
    if !fork_version(current) {
        return Err(Error::CurrentVersion);
    }
    let current = parse_version(current).ok_or(Error::CurrentVersion)?;
    let (url, limit) = match channel {
        UpdateChannel::Stable => (LATEST_URL, METADATA_LIMIT),
        UpdateChannel::Beta => (RELEASES_URL, RELEASE_LIST_LIMIT),
    };
    let metadata = fetch(url, limit)?;
    cancelled(cancel)?;
    let release = match channel {
        UpdateChannel::Stable => parse_release(&metadata)?,
        UpdateChannel::Beta => parse_releases(&metadata)?,
    };
    if !fork_version(&release.version) {
        return Err(Error::ReleaseVersion);
    }
    if parse_version(&release.version).ok_or(Error::ReleaseVersion)? <= current {
        return Ok(None);
    }
    // Only metadata reaches the UI. Nothing is downloaded or authorized to install.
    Ok(Some(Release {
        version: release.version,
        notes: release.body.unwrap_or_default(),
    }))
}
