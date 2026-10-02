# Gate room fixture provenance

Schema and identities copied from Spacedock v0.27.3,
`internal/gates/testdata/gate-room/{briefing,request}.json`.
`upstream-briefing.json` retains exact upstream bytes and its known canonical
SHA-256 `20bff726e2328f30c8f6576fdc347d07f582d28a3580bf2d63ebbd0d951ed2c0`.
`index.json` uses the same shape with committed, locally reproducible markdown
bytes and raw revisions. Test code constructs retained, request-backed, exact
file, folder, archive and Git-object variants in temporary repositories.
JCS golden vectors in parser tests are from RFC 8785 section 3.2.2 and 3.2.3,
the same RFC implemented by upstream cyberphone/json-canonicalization.
No test requires the survey clone, user configuration, or network.
