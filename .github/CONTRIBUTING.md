# Contributing

Thank you for your interest in contributing to daiku!

## Getting Started
- Fork the repository on GitHub.
- Create a feature branch from the `main` branch.
- Make focused, well-documented changes.
- Ensure all tests pass and the code compiles successfully.

## Issues
- Search existing issues before opening a new one to avoid duplicates.
- Provide clear, step-by-step reproduction steps for bugs.
- Include context, rationale, and use cases for feature requests.

## Pull Requests
- Keep Pull Requests small, clean, and focused on a single topic.
- Link any related issues in the PR description.
- Follow the repository's code style and conventions.

## How daiku is developed
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/).
- Split work into small, reviewed commits, each of which passes `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.
  A change to the CLI's behaviour also passes the fixture checks in
  [`AGENTS.md`](../AGENTS.md).
- Pull Requests are merged by rebase, so each commit lands on `main` as it was
  reviewed.
