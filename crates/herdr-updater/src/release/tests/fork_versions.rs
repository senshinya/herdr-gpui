use super::*;

#[test]
fn shinya_revisions_compare_numerically_within_the_upstream_version() {
    for (older, newer) in [
        ("20261028.1-shinya.1", "20261028.1-shinya.2"),
        ("20261028.1-shinya.9", "20261028.1-shinya.10"),
        ("20261028.1-shinya.65535", "20261028.2-shinya.1"),
        ("20261028.99-shinya.1", "20261029.1-shinya.1"),
    ] {
        let older = parse_version(older);
        let newer = parse_version(newer);
        assert!(older.is_some() && newer.is_some());
        assert!(older < newer);
    }
}

#[test]
fn malformed_fork_revisions_never_become_release_paths() {
    for version in [
        "20261028.1-shinya",
        "20261028.1-shinya.0",
        "20261028.1-shinya.01",
        "20261028.1-shinya.65536",
        "20261028.1-shinya.1/other",
        "20261028.1-shinya.1\n",
        "20261028.1-shinya.1-shinya.2",
    ] {
        assert!(parse_version(version).is_none(), "{version}");
    }
}
