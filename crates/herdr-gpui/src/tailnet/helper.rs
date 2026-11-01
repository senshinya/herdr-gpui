//! Internal ProxyCommand entry; parsed before GPUI or application startup.
use super::{Error, Result};
use herdr_client::tailnet::ProxyRequest;
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::PathBuf,
    process::ExitCode,
};
pub(super) fn parse(args: &[OsString]) -> Result<(PathBuf, ProxyRequest)> {
    if args.len() != 12 {
        return Err(Error::ProxyArguments.into());
    }
    let mut values = BTreeMap::new();
    for pair in args.chunks_exact(2) {
        let key = pair[0].to_str().ok_or(Error::ProxyArguments)?;
        if ![
            "--endpoint",
            "--engine",
            "--network-hex",
            "--peer-hex",
            "--generation",
            "--port",
        ]
        .contains(&key)
            || values.insert(key, &pair[1]).is_some()
        {
            return Err(Error::ProxyArguments.into());
        }
    }
    let get = |key| {
        values
            .get(key)
            .and_then(|s| s.to_str())
            .ok_or(Error::ProxyArguments)
    };
    let port = get("--port")?
        .parse::<u16>()
        .map_err(|_| Error::ProxyArguments)?;
    if port == 0 {
        return Err(Error::ProxyArguments.into());
    }
    let endpoint = values.get("--endpoint").ok_or(Error::ProxyArguments)?;
    let endpoint = PathBuf::from(endpoint);
    if !endpoint.is_absolute() {
        return Err(Error::ProxyArguments.into());
    }
    Ok((
        endpoint,
        ProxyRequest {
            generation: get("--generation")?
                .parse()
                .map_err(|_| Error::ProxyArguments)?,
            engine: get("--engine")?.into(),
            network: decode(get("--network-hex")?)?,
            peer: decode(get("--peer-hex")?)?,
            port,
        },
    ))
}
fn decode(text: &str) -> Result<String> {
    if text.is_empty()
        || text.len() > 512
        || !text.len().is_multiple_of(2)
        || !text.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::ProxyArguments.into());
    }
    let bytes = text
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |b: u8| {
                if b.is_ascii_digit() {
                    b - b'0'
                } else {
                    b.to_ascii_lowercase() - b'a' + 10
                }
            };
            digit(pair[0]) * 16 + digit(pair[1])
        })
        .collect();
    let value = String::from_utf8(bytes).map_err(|_| Error::ProxyArguments)?;
    if value.chars().any(char::is_control) {
        return Err(Error::ProxyArguments.into());
    }
    Ok(value)
}
pub(crate) fn run(args: &[OsString]) -> Option<ExitCode> {
    if args.first().map(OsString::as_os_str) != Some(OsStr::new("tailscale-proxy")) {
        return None;
    }
    let result = parse(&args[1..]).and_then(|(endpoint, request)| {
        herdr_remote::tailscale::proxy::stdio(&endpoint, &request).map_err(Into::into)
    });
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    })
}
