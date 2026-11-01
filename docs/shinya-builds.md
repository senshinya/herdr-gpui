# Shinya builds

The fork builds macOS Apple Silicon apps with the **Shinya macOS release**
GitHub Actions workflow. Push to `main`, or use **Run workflow**. Successful
builds publish a Release in `senshinya/herdr-gpui` tagged `v<display version>`.

Version metadata lives in [shinya-version.json](../.github/shinya-version.json).
The display version is `<upstream version>-shinya.<revision>`, initially
`20261028.1-shinya.1`. `upstream_version` names the upstream release this fork
follows; `upstream_commit` pins the exact upstream code, including changes
after that release. The current base is upstream `v20261028.1` plus changes
through `0f2f80d5d84d7b51a314d3b2a36b1162d927a159`.

After merging upstream, update both upstream fields and reset the fork revision
to 1. For another fork version on the same base, increment `revision`.
The workflow verifies that the pinned upstream commit is an ancestor of the
commit it builds. The numeric macOS bundle build number is the Actions run
number; the app's display version retains the `-shinya` suffix.

Download the ZIP from the Release or the run's **Artifacts** section. It contains `Herdr.app`,
its companion executables, and third-party notices. `build-info.json` records
the fork version, upstream release and commit, fork commit, and build number;
the same file is embedded in the app's Resources folder. The artifact also
includes the ZIP's SHA-256 checksum.

These packages have ad-hoc signatures and no notarization. App Updates checks
this fork's Releases, compares the upstream version and Shinya revision, and
offers **Open Release** when a newer version is available. Download and install
it from that page. These builds do not automatically download or replace the
installed app, and need no updater signing key.

Release notes are generated from the fork's commits using `git-cliff` and the
repository's changelog configuration. Increment the revision before publishing
another version on the same upstream base; existing releases are not overwritten.
