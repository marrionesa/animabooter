# Contributing to AnimaBooter

Thanks for helping AnimaBooter stay small, honest and fast. These rules are
short but strict — they are what keeps the project trustworthy.

## Ground rules

1. **Local-only, forever.** The product must never make a network request at
   runtime: no telemetry, no update pings, no "phone home". Contributions
   that add any outbound call will be rejected.
2. **No remotes in the workflow.** Development happens locally
   (`git init` + local commits + local tags). Publishing the repository is a
   manual decision of the owner, outside the scope of day-to-day work.
3. **Honesty over polish.** Never fake progress, sizes or success. If the
   uncompressed size is unknown, show byte counters. If a flush fails,
   report failure. `sync_all` before "done", always.

## Code standards

- Rust, edition 2021. Comments and commit messages in **English**.
- Gates before committing:
  ```bash
  cargo fmt --check
  cargo clippy -- -D warnings
  cargo test
  bun run check    # svelte-check, from the repo root
  bun run build    # vite build
  ```
- Forbidden: `todo!()`, `unimplemented!()`, empty stubs, `unwrap()`/`expect()`
  on device I/O paths (tests excepted), loading a whole image into RAM.
- Errors crossing IPC implement `{ message, hint }` — every user-facing
  failure needs an actionable hint.
- The IPC contract lives twice on purpose (`commands/*.rs` ↔
  `src/lib/types.ts` + `src/lib/ipc.ts`). If you change one side, change the
  other in the same commit.

## Commits

Conventional commits, one logical block per commit:

```text
feat: parallel pipeline engine
fix: re-lock volumes before raw write on Windows
test: verifier corruption suite
docs: bilingual readme
chore: project scaffold
```

## Testing a real flash

`cargo test` covers the engine without hardware. Before shipping a release,
also do one **manual** run with a real USB stick you don't care about:

- [ ] Image select (raw + at least one compressed format)
- [ ] Drive list shows the stick, hides internal disks
- [ ] Hold-to-confirm refuses quick clicks
- [ ] Cancel mid-write → warning + re-flash required
- [ ] `sync_all` message appears in the live log before done
- [ ] Verification bar completes; ResultCard hash matches the distro's
      published sha256 (for uncompressed images)
- [ ] Export PNG + Copy summary work offline
- [ ] Eject reports success

## Performance numbers

If you want to add the benchmark table numbers to the README: measure them
yourself on real hardware, with the same image and the same drive, and put
the exact methodology next to the numbers. We publish only real results.

## License

By contributing you agree your work is released under the project MIT
license.
