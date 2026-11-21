# Security Policy

## Supported versions

daiku is pre-1.0. Security fixes ship in a new release, and only the latest
release is supported. Releases before `v0.3.0`, including `v0.2.0`, the last
release of the Go CLI, get no fixes.

Install daiku with one of the installers or a release archive, as the
[README](../README.md#install) describes.

## Reporting a vulnerability

Please do not open a public issue for security problems.

Report privately through GitHub: go to the
[Security tab](https://github.com/btdstudio/daiku/security/advisories/new) and
choose **Report a vulnerability**.

Include what you found, how to reproduce it, and the affected version or
commit. This is a small project maintained in spare time; you can expect an
acknowledgement within a few days. Once a fix is available, the advisory will
be published and you will be credited unless you prefer otherwise.

## Scope

In scope: the `daiku` commands (`init`, `update`, `check`, and the plan and
build commands, including `commit` and `pr`, which run git and gh in a
registered repo), the embedded scaffold, and the workspace files it creates
(`.daiku/`, `STATE.json`, `LOG.md`, `REPOS.json`, `plan/`, `build/`).

Out of scope: vulnerabilities in third-party dependencies with no impact on
daiku (report those upstream), and the contents of a user's own project.
