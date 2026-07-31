# rendezvous-interop-tests

CI-only interop validation between bolt-rendezvous and bolt-core.

## Purpose

Verifies peer code validation parity (and expected divergence) between
the rendezvous server's permissive rules and bolt-core's strict canonical
rules — without injecting bolt-core into the main bolt-rendezvous crate.

## Layout assumption

bolt-core-sdk must be checked out as a **sibling** of bolt-rendezvous:

```
<parent>/
  bolt-rendezvous/          ← this repo
  bolt-core-sdk/            ← sibling checkout
    rust/bolt-core/         ← path dependency target
```

In CI, this is achieved by checking out bolt-rendezvous and bolt-core-sdk
as sibling directories under `$GITHUB_WORKSPACE`.

## Running locally

```sh
# From bolt-rendezvous root (with sibling bolt-core-sdk present):
cargo test --manifest-path interop-tests/Cargo.toml
```

## Workspace note

If a root `[workspace]` is ever added to bolt-rendezvous/Cargo.toml,
`interop-tests` **must** be excluded:

```toml
[workspace]
exclude = ["interop-tests"]
```

Otherwise `cargo test` at the root would attempt to resolve the
`../../bolt-core-sdk/rust/bolt-core` path dependency, which does not
exist for subtree consumers (localbolt, localbolt-app pull `signal/`
only).

## Subtree safety

This crate must **never** be vendored into product repos. Subtrees pull
`signal/` only — `interop-tests/` is outside that prefix by design.
