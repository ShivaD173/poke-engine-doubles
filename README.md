# Poke Engine Doubles

This is a fork of github.com/pmariglia/poke-engine for playing doubles.

This project is early states and probably has many broken things on main


## Champions spread-move corrections

The review and regression checks for `[Gen 9 Champions] VGC 2026 Reg M-C`
(`champions`) are documented in [the spread-move audit](docs/champions-spread-audit.md).
Build with `--no-default-features --features mega` for Champions. The audit also
lists remaining mechanics differences and how to install this engine in the client.

See [remaining Champions issues](docs/remaining-issues.md) for the unfixed
mechanics, search approximations, and client integration work.
