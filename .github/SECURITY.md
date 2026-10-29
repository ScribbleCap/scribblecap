# Security policy

## Project status

ScribbleCap is in early development and has no supported stable release yet.
Security fixes target the current development branch; there is no commitment to
backport fixes to older commits or prerelease builds.

## Report privately

Do not put vulnerability details, exploit files or sensitive images in public
issues, discussions or pull requests. File-parser crashes with security impact,
code execution, access-control failures and recoverable redacted content should
be reported privately.

Email [scribblecap@gmail.com](mailto:scribblecap@gmail.com) with the subject
`ScribbleCap security report`. This is the project's private reporting contact.
Do not open a public issue for the report.

Include the affected commit/version, operating system, impact and minimal
reproduction steps. Use synthetic input files where possible. For redaction
issues, explain how the supposedly removed data can be recovered without sharing
real confidential content.

The maintainer will assess the report and coordinate a fix and disclosure with
the reporter. Response and fix times depend on maintainer availability; no fixed
service level or bounty is promised.
