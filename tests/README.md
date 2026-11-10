# Tests

This directory holds the integration tests. Unit tests live next to the code in `src/`.

| Path | Purpose |
|------|---------|
| `test_java.rs`, `test_rust.rs` | CLI integration tests that run the `log2src` binary and snapshot its output |
| `common_settings/` | Shared helpers: a temp `HOME`, the command builder, and insta filters |
| `java/` | Java sources used as inputs |
| `resources/` | Log files and other inputs |
| `snapshots/` | Expected output, one `.snap` file per assertion |

Unit tests in `src/` have their own snapshots in `src/snapshots/`. They work the same way.

## How snapshot tests work

The tests use [insta](https://insta.rs) and [insta-cmd](https://crates.io/crates/insta-cmd). Each test builds a command with `common_settings::CommandGuard` and calls `assert_cmd_snapshot!(cmd.cmd)`. That records the exit code, stdout and stderr, and compares them with the stored `.snap` file.

A snapshot file is named `<test file>__<test name>.snap`. If a test has several assertions, later ones get `-2`, `-3` and so on, for example `test_java__basic_slf4j-2.snap`.

`common_settings::enable_filters` replaces machine-specific text with stable placeholders, such as `{java_dir}`, `{example_dir}`, `{home}`, `cache.XXX` and `{errmsg}`. Add a filter there if a new path or message differs between machines.

## Running the tests

```sh
cargo test                 # everything
cargo test --test test_java
cargo test --lib           # unit tests in src/
```

When a snapshot doesn't match, the test fails and insta writes the actual output next to the old one as `<name>.snap.new`. A plain `cargo test` stops at the first mismatch in each test binary.

## Updating snapshots

Install the tool once with `cargo install cargo-insta`.

```sh
cargo insta test            # run all tests and collect every mismatch
cargo insta review          # accept or reject each change interactively
cargo insta accept          # accept all pending changes
cargo insta reject          # discard all pending changes
```

To run in one step and accept, use `cargo insta test --accept`.

Don't accept blindly. Look at the diff first, and check that only the intended change appears. For example, when a new field is added to `SourceRef`, every snapshot that prints it gains that field and nothing else.

### Metadata noise

Depending on the `cargo-insta` version, accepting a snapshot can also add `assertion_line:` to the header. It can also add or change an `info: env:` block that contains temporary directory paths such as `/var/folders/.../.tmpXXXX`. These lines don't affect whether a test passes, but they create meaningless diffs. Before committing, check `git diff` and revert header-only changes. Keep the header lines the file had before and commit only the real output changes.

### Before committing

1. Run `cargo test` again and confirm everything passes.
2. Make sure no `*.snap.new` files remain. `git status` should not list them, and they should never be committed.
3. Review `git diff -- '*.snap'` to confirm the changes are expected.

## Adding a test

1. Add the inputs to `java/` or `resources/`, or use a file in `examples/`.
2. Write the test with `common_settings::CommandGuard::new()?`, add its arguments, and call `assert_cmd_snapshot!(cmd.cmd)`. `test_rust.rs` has examples.
3. Run `cargo insta test`, then `cargo insta review` to create the snapshot.
4. Commit the new `.snap` file with the test.

## Removing tests

When you delete or rename a test, delete its stale `.snap` files too. `cargo insta test --unreferenced=reject` lists snapshots that no test uses.
