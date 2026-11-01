use super::*;
use crate::Error;
use std::{collections::HashSet, path::Path};

#[test]
fn bonjour_suggests_the_announced_host_and_its_port() {
    let local = HashSet::from(["192.168.1.2".parse().unwrap()]);
    let addresses = ["192.168.1.20".parse().unwrap()];
    let found = bonjour::candidate(
        "studio._ssh._tcp.local.",
        "studio.local.",
        22,
        &addresses,
        &local,
    )
    .unwrap();
    assert_eq!(found.source, Source::Bonjour);
    assert_eq!(found.name, "studio");
    assert_eq!(found.target, "studio.local");
    assert_eq!(found.keys, ["studio.local", "~studio", "192.168.1.20"]);

    let other = bonjour::candidate(
        "nas._ssh._tcp.local.",
        "nas.local.",
        2222,
        &addresses,
        &local,
    )
    .unwrap();
    assert_eq!(other.target, "ssh://nas.local:2222");
}

#[test]
fn bonjour_skips_this_machine_and_unsafe_names() {
    let local = HashSet::from(["192.168.1.2".parse().unwrap()]);
    let mine = ["fe80::1".parse().unwrap(), "192.168.1.2".parse().unwrap()];
    assert!(bonjour::candidate("me._ssh._tcp.local.", "me.local.", 22, &mine, &local).is_none());
    let theirs = ["192.168.1.9".parse().unwrap()];
    for host in ["-oProxyCommand=x.local.", "a b.local.", ""] {
        assert!(
            bonjour::candidate("x._ssh._tcp.local.", host, 22, &theirs, &local).is_none(),
            "{host}"
        );
    }
    assert!(bonjour::candidate("x._ssh._tcp.local.", "x.local.", 0, &theirs, &local).is_none());
}

/// A loader over in-memory files. Paths are compared by component, so the
/// same fixture works with `/` and `\` separators.
fn files(
    entries: &'static [(&'static str, &'static str)],
) -> impl FnMut(&Path) -> std::io::Result<Option<String>> {
    move |path: &Path| {
        Ok(entries
            .iter()
            .find(|(name, _)| path == Path::new(name))
            .map(|(_, text)| (*text).to_owned()))
    }
}

fn aliases(text: &str) -> Vec<(String, String)> {
    let mut reader = ssh_config::Reader::new("/home/me".into(), |_: &Path| Ok(None));
    reader.parse(text, 0, &[]).unwrap();
    reader
        .candidates()
        .into_iter()
        .map(|candidate| (candidate.target, candidate.keys.join(" ")))
        .collect()
}

#[test]
fn ssh_config_lists_concrete_aliases_with_their_host_names() {
    let found = aliases(
        "# comment\n\
         Host *\n  User me\n\
         Host box box-alt !bad web*\n  HostName=box.tail1.ts.net\n  HostName ignored.example\n\
         Host \"quoted\"\n  Hostname %h.example\n\
         Match host other\n  HostName match.example\n\
         HOST github.com gh\n  HostName github.com\n\
         Host work.github.com\n",
    );
    assert_eq!(
        found,
        [
            ("box".into(), "box box.tail1.ts.net ~box".into()),
            ("box-alt".into(), "box-alt box.tail1.ts.net ~box".into()),
            ("quoted".into(), "quoted ~quoted".into()),
        ]
    );
}

#[test]
fn ssh_config_follows_includes_relative_to_the_ssh_directory() {
    let mut reads = Vec::new();
    let mut load = files(&[
        ("/home/me/.ssh/config", "Include hosts ~/other /abs/conf\n"),
        ("/home/me/.ssh/hosts", "Host lab\n"),
        ("/abs/conf", "Include config\n"),
    ]);
    let mut reader = ssh_config::Reader::new("/home/me".into(), |path: &Path| {
        reads.push(path.to_owned());
        load(path)
    });
    reader.include("~/.ssh/config", 0, &[]).unwrap();
    let found: Vec<String> = reader.candidates().into_iter().map(|c| c.target).collect();
    assert_eq!(found, ["lab"]);
    // The include cycle stops at the depth limit instead of looping.
    assert!(reads.len() < 16, "{reads:?}");
    assert_eq!(reads[1], Path::new("/home/me/.ssh/hosts"));
    assert_eq!(reads[2], Path::new("/home/me/other"));
}

#[test]
fn ssh_config_includes_continue_the_enclosing_host_block() {
    let mut reader = ssh_config::Reader::new(
        "/home/me".into(),
        files(&[
            (
                "/home/me/.ssh/config",
                "Host gh\n  Include gh.conf\nHost lab\n  Include lab.conf\n  User me\n",
            ),
            ("/home/me/.ssh/gh.conf", "HostName github.com\n"),
            // A `Host` in an included file ends the block only inside it.
            (
                "/home/me/.ssh/lab.conf",
                "HostName lab.tail1.ts.net\nHost other\n",
            ),
        ]),
    );
    reader.include("~/.ssh/config", 0, &[]).unwrap();
    let found: Vec<(String, String)> = reader
        .candidates()
        .into_iter()
        .map(|c| (c.target, c.keys.join(" ")))
        .collect();
    // `gh` is a git hosting alias once its included HostName is known.
    assert_eq!(
        found,
        [
            ("lab".into(), "lab lab.tail1.ts.net ~lab".into()),
            ("other".into(), "other ~other".into()),
        ]
    );
}

#[test]
fn ssh_config_expands_include_wildcards() {
    let home = tempfile::tempdir().unwrap();
    let directory = home.path().join(".ssh/config.d");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("b.conf"), "Host bravo\n").unwrap();
    std::fs::write(directory.join("a.conf"), "Host alpha\n").unwrap();
    std::fs::write(directory.join("notes.txt"), "Host skipped\n").unwrap();
    let mut reader = ssh_config::Reader::new(home.path().into(), |path: &Path| {
        std::fs::read_to_string(path).map(Some)
    });
    reader.include("config.d/*.conf", 0, &[]).unwrap();
    reader.include("missing.d/*", 0, &[]).unwrap();
    let found: Vec<String> = reader.candidates().into_iter().map(|c| c.target).collect();
    assert_eq!(found, ["alpha", "bravo"]);
}

#[test]
fn ssh_config_read_errors_name_the_file() {
    let mut reader = ssh_config::Reader::new("/home/me".into(), |_: &Path| {
        Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
    });
    let error = reader.include("~/.ssh/config", 0, &[]).unwrap_err();
    let Error::SshConfig { path, source } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(path, Path::new("/home/me/.ssh/config"));
    assert_eq!(source.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn wildcards_match_like_ssh_globs() {
    assert!(ssh_config::wildcard("*.conf", "a.conf"));
    assert!(ssh_config::wildcard("h?st*", "host-1"));
    assert!(ssh_config::wildcard("*", ""));
    assert!(!ssh_config::wildcard("*.conf", "a.conf.bak"));
    assert!(!ssh_config::wildcard("h?st", "hst"));
}
