# Commit-pinned Linux builds

Every push to `master` runs `linux-build`. It builds portable x86_64 Linux
release binaries on Debian bookworm (glibc 2.36), tests `chaos-clamp`, checks
shared-library resolution, and runs `chaos --version`. The bundle contains
`chaos`, `chaos_journald`, `alcatraz`, `chaos-forkve-wrapper`, the installer, and
`SOURCE_REVISION`. Runtime hosts need `libdbus-1-3` and glibc 2.36 or newer.
These builds do not replace the full `rust-ci` gate or stable releases.

Each successful master build publishes a **prerelease**, not `latest`, tagged
`build-<full commit SHA>`. Assets are named:

- `chaos-linux-x86_64-<full commit SHA>.tar.gz`
- `chaos-linux-x86_64-<full commit SHA>.tar.gz.sha256`

Consumers should pin the full source revision and verify the checksum before
extracting. Also check `SOURCE_REVISION` against the requested revision. Do not
use the moving master branch or the latest-release endpoint as a deployment
input. Checksums detect transfer corruption, not a compromised publisher.

The workflow can be manually dispatched on master to retry a failed build.
Existing releases are not overwritten. PRs changing the workflow build and
upload a temporary Actions artifact, but cannot publish a release. Other
architectures continue to use the regular release workflow or source builds.
No downstream deployment or running session is changed by publishing a build.
