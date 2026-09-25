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

## 1. Local development

Clone and set up the toolchain (Rust stable, Bun or Node ≥ 18, plus the
Tauri 2 system dependencies documented in the README):

```bash
bun install         # frontend dependencies
bun run dev         # vite dev server
cargo tauri dev     # full desktop app with live reload (from src-tauri/ or via `bun run tauri dev`)
```

The IPC contract lives twice on purpose (`src-tauri/src/commands/*.rs` ↔
`src/lib/types.ts` + `src/lib/ipc.ts`). If you change one side, change the
other in the same commit.

## 2. Validations before committing

Every commit block must pass all of these locally:

```bash
cargo fmt --check            # formatting (run in src-tauri/)
cargo clippy -- -D warnings  # lints, warnings are errors
cargo test                   # pipeline suites: progress math, throttle,
                             # cancellation, 1-byte corruption, gzip roundtrip
bun run check                # svelte-check, from the repo root
bun run build                # vite production build
```

Code standards:

- Rust, edition 2021. Comments and commit messages in **English**.
- Forbidden: `todo!()`, `unimplemented!()`, empty stubs, `unwrap()`/`expect()`
  on device I/O paths (tests excepted), loading a whole image into RAM.
- Errors crossing IPC implement `{ message, hint }` — every user-facing
  failure needs an actionable hint.

Commits are conventional, one logical block each:

```text
feat: parallel pipeline engine
fix: re-lock volumes before raw write on Windows
test: verifier corruption suite
docs: bilingual readme
chore: project scaffold
```

Note: there is **no automatic CI on push yet** — `ci.yml` only runs on
manual `workflow_dispatch`, so the checks above are your safety net. Run
them yourself.

## 3. Testing on real hardware

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

Do this on every OS you intend to ship — the platform notes in the README
(udev rules on Linux, elevation on Windows, `/dev/rdiskN` on macOS) apply.

## 4. Release preparation

Releases are built by `.github/workflows/release.yml`, which only runs when
triggered manually (`workflow_dispatch`) or when the owner pushes a `v*`
tag — never on a regular push. Before cutting a release:

- [ ] All validations in section 2 pass.
- [ ] The hardware checklist in section 3 is done for the target platforms.
- [ ] `CHANGELOG.md` has a section for the version.
- [ ] Version numbers (`package.json`, `src-tauri/tauri.conf.json`,
      `src-tauri/Cargo.toml`) are consistent.
- [ ] Tagging, pushing and publishing are done **manually by the owner**.

## Performance numbers

If you want to add the benchmark table numbers to the README: measure them
yourself on real hardware, with the same image and the same drive, and put
the exact methodology next to the numbers. We publish only real results.

## License

By contributing you agree your work is released under the project MIT
license.
