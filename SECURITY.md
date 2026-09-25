# Security Policy

## Supported Versions

AnimaBooter is in an **early alpha stage** (`0.1.0-alpha`). There is no
stable release line yet, so no long-term support matrix exists at this time.
Until a first stable release is published, assume that only the latest code
on `main` is the reference and that anything can still change.

## Reporting a Vulnerability

Please report vulnerabilities responsibly:

- **Do not** open a public GitHub issue for anything that could be exploited
  or that exposes users of a flashing tool to data loss.
- An official private contact channel (e.g. a security email or GitHub
  Security Advisories) has **not been set up yet**; the exact reporting
  mechanism is to be defined by the maintainer. Until then, use the most
  private channel available to reach the owner ([@marrionesa](https://github.com/marrionesa)).
- Include a description, the affected platform(s) (Windows / macOS / Linux)
  and, if possible, reproduction steps.

## Security Considerations

AnimaBooter writes raw disk images to block devices. That is inherently
destructive, so the application enforces the following safeguards (all
documented in the README):

- Only **removable** drives are listed by default. Internal disks are hidden
  unless `unsafe_mode` is explicitly enabled in `settings.json`, which
  requires **two** separate confirmations in the UI.
- Before any device is opened, the `flash` command re-checks hard,
  platform-specific rules:
  - Linux: a drive hosting the root filesystem (`/`) is refused
    unconditionally.
  - Windows: the volume containing `%SystemRoot%` is refused
    unconditionally, even in unsafe mode.
  - macOS: internal disks are refused unless unsafe mode is on.
- Every destructive step announces its intent into the live log before
  executing.
- The destructive action uses **hold-to-confirm (~1.2 s)** so a misclick
  cannot trigger a write.
- Cancelling a flash mid-write warns that the device is left in an unknown
  state and must be re-flashed.
- The application is offline by design: no telemetry, no network calls at
  runtime.
