# Shinya builds

The fork builds macOS Apple Silicon apps with the **Shinya macOS build**
GitHub Actions workflow. Push to `main` or a `build/macos-*` branch, or use
**Run workflow** after the workflow is present on the default branch.

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

Download the ZIP from the run's **Artifacts** section. It contains `Herdr.app`,
its companion executables, and third-party notices. `build-info.json` records
the fork version, upstream release and commit, fork commit, and build number;
the same file is embedded in the app's Resources folder. The artifact also
includes the ZIP's SHA-256 checksum.

These are development packages with ad-hoc signatures and no notarization.
They are not published as GitHub Releases. The workflow supplies no upstream
update signing key, and the fork suffix does not pass the upstream updater's
calendar-release parser, so this build cannot update itself to an upstream
release.
