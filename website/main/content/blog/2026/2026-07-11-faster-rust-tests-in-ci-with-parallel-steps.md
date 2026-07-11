---
title: Faster Rust tests in CI with parallel steps
date: 2026-07-11
draft: false
description: A few more tricks using recently introduced parallel steps in GitHub Actions to speed up CI runs for Rust projects
tags: [ tips-and-tricks ]
authors: [ nazar-pc ]
---

I've been procrastinating recently on the primary objectives, so I figured I'd share something useful for the broader
Rust community in the meantime again.

[A while back] I shared how to use [YAML anchors] to reuse parts of the workflow for faster CI times. This time I'll
show how to accelerate CI even more with another recent capability in GitHub Actions: [parallel steps]. While the
feature today is really bare-bones, it is already beneficial.

[A while back]: ../2025-09-25-shorter-github-actions-runs
[YAML anchors]: https://github.blog/changelog/2025-09-18-actions-yaml-anchors-and-non-public-workflow-templates/
[parallel steps]: https://github.blog/changelog/2026-06-25-actions-steps-can-now-be-run-in-parallel/

<!--more-->

While there are obvious cases where distinct independent steps can run in parallel, I'll show two applications of
parallel steps for Rust projects that are less obvious but still have the level of complexity that isn't too burdensome
to maintain.

## Testing permutations

A simpler case is testing various permutations. It may be running clippy on each crate individually to test its default
features (vs. as part of the workspace) or testing crates with a different set of enabled features.

Regardless of what your use case is, chances are a lot of CI time will be wasted due to imperfect parallelism. While
Cargo does parallelize compilation of dependencies, the graph is not always perfectly parallelizable, so there are
periods of time when most of the CPU cores are doing nothing.

For example, you may be linting each crate in a workspace with Clippy using something like this:

```yaml
- name: cargo clippy (each crate individually)
  shell: bash
  run: |
    cargo metadata --no-deps --format-version 1 \
      | jq -r '.packages[].name' \
      | tr -d '\r' \
      | while IFS= read -r crate; do
          echo "Checking \`${crate}\`"
          cargo clippy --all-targets --package $crate -- -D warnings
        done
  if: matrix.type == 'individually'
```

This works, but it is inherently sequential. There are GNU parallel and similar tools that could be used to parallelize
it, but IMHO the complexity of it and maintenance burden are too much. With parallel steps this can be achieved
relatively easily with clean logs and relatively low maintainability cost.

We simply write the messages and commands to a file instead of executing them immediately:

```yaml
- name: cargo clippy (each crate individually)
  shell: bash
  run: |
    cargo metadata --no-deps --format-version 1 \
      | jq -r '.packages[].name' \
      | tr -d '\r' \
      | while IFS= read -r crate; do
          echo "Checking \`${crate}\`" >> "${{ runner.temp }}/queue"
          echo "cargo clippy --all-targets --package \"${crate}\" -- -D warnings" >> "${{ runner.temp }}/queue"
        done
  if: matrix.type == 'individually'
```

Then run those commands in parallel, while reusing the code with YAML anchors to reduce duplication:

```yaml
- parallel:
    - shell: bash
      env:
        WORKER_ID: "1"
      run: &parallel_worker |
        set -e

        mkdir -p "${{ runner.temp }}/claims"
        export CARGO_TARGET_DIR="target_${WORKER_ID}"
        while IFS= read -r message && IFS= read -r command; do
          hash=$(printf '%s\n%s\n' "$message" "$command" | sha256sum | cut -d' ' -f1)
          if mkdir "${{ runner.temp }}/claims/${hash}" 2>/dev/null; then
            echo "$message"
            eval "$command"
          fi
        done < "${{ runner.temp }}/queue"
      if: matrix.type == 'individually'

    - shell: bash
      env:
        WORKER_ID: "2"
      run: *parallel_worker
      if: matrix.type == 'individually'

    - shell: bash
      env:
        WORKER_ID: "3"
      run: *parallel_worker
      if: matrix.type == 'individually'

    - shell: bash
      env:
        WORKER_ID: "4"
      run: *parallel_worker
      if: matrix.type == 'individually'
```

GitHub Actions runners for public repositories have 4 cores on Linux and Windows, so 4 workers seemed like a good
choice. Distinct `target` directories are used to make sure Cargo processes do not step on each other's toes, but with
locking improvements in Cargo it should be possible to improve this and benefit from cache reuse.

`mkdir` with command hash is used as a lock, ensuring workers cooperate and each command is only executed exactly once.

With the above changes, clippy linting of individual crates on `windows-2025` improved from ~22 minutes to ~17 minutes,
despite technically doing more work due to reduced cache reuse.

## Parallel Miri tests

Miri tests are more complex and were a much heavier burden to run in CI. The reason is that they are sequential, and not
only in the sense of Miri being a single-threaded interpreter, but also because only one test is running at any given
time. Add high startup latency and it gets very slow quickly.

An obvious solution is to use [nextest], but it actually has negative ROI. Running Miri tests with it on a 64-core CPU
is **slower** than running tests sequentially on a single-core CPU. The reason for it is that `nextest` by design runs
each test in a separate process, which explodes the overhead of the startup cost when you have test binaries with
thousands of tests like I do.

[nextest]: https://github.com/nextest-rs/nextest

On a high level what I wanted here is similar to the above section: run commands in parallel. However, Cargo and Miri
today do not make it easy, so I had to get creative and intercept Miri calls.

So the strategy is the following: we run `cargo miri test`, but instead of running compiled tests, store execution
commands and run them later in parallel. Windows strikes back a bit here since it doesn't support shebang for scripts,
so here is a simple Rust program that will pretend to be Miri and will call real Miri (renamed to `miri.real`)
only for compilation, but not for interpreting:

```rust
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::{env, fs, iter, process};

fn main() {
    let exe = env::current_exe().unwrap();
    let miri_real = exe.parent().unwrap().join("miri.real");

    if env::var_os("MIRI_BE_RUSTC").is_some() {
        let status = process::Command::new(&miri_real)
            .args(env::args_os().skip(1))
            .status()
            .expect("failed to exec miri.real");
        process::exit(status.code().unwrap_or(1));
    }

    if let Ok(inv_path) = env::var("MIRI_REPLAY") {
        let data = fs::read_to_string(&inv_path).unwrap();
        let mut lines = data.lines();

        let cwd = lines.next().unwrap();

        let n_args = lines.next().unwrap().parse().unwrap();
        let args = iter::repeat_with(|| lines.next().unwrap())
            .take(n_args)
            .collect::<Vec<_>>();

        let n_envs = lines.next().unwrap().parse().unwrap();
        let envs = iter::repeat_with(|| lines.next().unwrap().split_once('=').unwrap())
            .take(n_envs)
            .collect::<Vec<_>>();

        let status = process::Command::new(&miri_real)
            .args(&args)
            .env_clear()
            .envs(envs)
            .current_dir(cwd)
            .status()
            .expect("failed to exec miri.real in replay");
        process::exit(status.code().unwrap_or(1));
    }

    let args = env::args().skip(1).collect::<Vec<_>>();

    let crate_name = args
        .windows(2)
        .find_map(|w| (w[0] == "--crate-name").then(|| w[1].as_str()))
        .unwrap_or("unknown");

    let inv_dir = env::var("MIRI_INVOCATIONS_DIR").expect("MIRI_INVOCATIONS_DIR not set");
    let queue_path = env::var("MIRI_QUEUE_PATH").expect("MIRI_QUEUE_PATH not set");
    let inv_path = format!("{inv_dir}/{crate_name}-{}", process::id());

    let cwd = env::current_dir().unwrap();
    let envs = env::vars().collect::<Vec<_>>();

    let mut data = String::new();
    writeln!(data, "{}", cwd.display()).unwrap();
    writeln!(data, "{}", args.len()).unwrap();
    for arg in &args {
        writeln!(data, "{arg}").unwrap();
    }
    writeln!(data, "{}", envs.len()).unwrap();
    for (k, v) in &envs {
        writeln!(data, "{k}={v}").unwrap();
    }

    fs::write(&inv_path, &data).unwrap();

    let mut queue = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&queue_path)
        .unwrap();
    let name = std::path::Path::new(&inv_path)
        .file_name()
        .unwrap()
        .to_string_lossy();
    writeln!(queue, "Miri: running {name}").unwrap();
    writeln!(queue, "MIRI_REPLAY=\"{inv_path}\" \"{}\"", exe.display()).unwrap();
}
```

With this program swapping places with real Miri we can "run" tests sorta like we normally would, except none of the
tests will run just yet:

```yaml
- name: cargo miri test --capture (together)
  shell: bash
  env:
    MIRI_INVOCATIONS_DIR: ${{ runner.temp }}/miri_invocations
    MIRI_QUEUE_PATH: ${{ runner.temp }}/queue
  run: |
    cargo miri test --locked
  if: matrix.miri == true && matrix.type == 'together'
```

Once that step is done, we have all tests compiled and ready to run in parallel (`parallel_worker` anchor here is reused
from clippy):

```yaml
- parallel:
    - shell: bash
      env:
        WORKER_ID: "1"
      run: *parallel_worker
      if: matrix.miri == true

    - shell: bash
      env:
        WORKER_ID: "2"
      run: *parallel_worker
      if: matrix.miri == true

    - shell: bash
      env:
        WORKER_ID: "3"
      run: *parallel_worker
      if: matrix.miri == true

    - shell: bash
      env:
        WORKER_ID: "4"
      run: *parallel_worker
      if: matrix.miri == true
```

And with this trick, the time to run all tests together under Miri on Windows decreased from ~40 minutes to ~24 minutes.
I'm giving Windows as an example since it performs notoriously badly in CI compared to Linux and macOS, but performance
improvements can be observed across the board.

This approach is still coarse-grained since it runs test binaries in parallel rather than tests themselves, but it is
already good enough in my case and what I consider to be an optimal level of complexity.

## Looking forward

These are just higher-level hacks. There is a lot more that could have been done on a lower level. For example, Cargo
could run some tests while still compiling others, achieving similar or better CPU and I/O utilization, while improving
cache reuse. Distinct Miri tests could have been running in parallel by default. Unfortunately, it is likely to take a
lot of time until we see those capabilities in Cargo proper.

I'm hoping this will help others accelerate their Rust CI workflows, and maybe someone will even extract these into a
reusable crate (for the Miri hack) or workflow (for parallel steps) that will reduce the amount of boilerplate required.
If you do, please ping me [on Zulip].

[on Zulip]: https://abundance.zulipchat.com/

You can see the current version of the complete workflow I use [here].

[here]: https://github.com/nazar-pc/abundance/blob/22489c90db4eff47e9618442d056ab5af7a57d95/.github/workflows/rust.yml
