# Security Policy

[English](SECURITY.md) | [Português Brasileiro](SECURITY.pt-BR.md)

## Supported versions

| Version | Supported          |
|---------|--------------------|
| 0.2.6   | yes (current)      |

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security-sensitive
problems.

Send a private report to the maintainer at the address listed in
`Cargo.toml`'s `authors` field. Encrypt sensitive material with the
maintainer's PGP key (request by email).

Include:

- A clear description of the issue and the attack scenario.
- Steps to reproduce, including the affected version.
- The expected and actual behaviour.
- Any known mitigations or workarounds.

You should receive an acknowledgement within 72 hours. The maintainer
will coordinate disclosure timing with you.

## Threat model

`youtube-legend-cli` is a single-user, non-interactive CLI that:

- Reads a YouTube URL from a positional argument or stdin.
- Performs HTTPS requests to one or two third-party subtitle providers.
- Writes the decoded subtitle body to stdout.
- Stores the last successful response under the user's standard cache
  directory, keyed on `(video_id, language, format)`, with a default
  24-hour TTL.
- Does not collect, transmit, or persist any telemetry.

The `secret_endpoints` module is **gitignored** and never shipped in
crate releases. It carries provider hosts, paths, cookies, and user
agents that are intentionally redacted from open-source consumers.
If you find any of those values exposed in a public artifact, please
report immediately.
