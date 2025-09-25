---
title: Shorter GitHub Actions runs
date: 2025-09-25
draft: false
description: A trick recently enabled by GitHub Actions improvements to shorten CI runs
tags: [ tips-and-tricks ]
authors: [ nazar-pc ]
---

This is just a short note about something that was not possible to do as cleanly before.

If you worked with GitHub Actions for a meaningful period of time, and especially for testing Rust code, you will know
that Windows runners are really slow. They are easily the bottleneck in many workflows and until recently there was no
good way around it without turning the whole workflow into a mess. Thankfully, GitHub Actions recently introduced
support for [Yaml anchors] in workflow files, which allows to reuse parts of the workflow instead of copy-pasting them.

[Yaml anchors]: https://github.blog/changelog/2025-09-18-actions-yaml-anchors-and-non-public-workflow-templates/

<!--more-->

For context, I had a job definition that looked something like this:

```yaml
  cargo-test:
    strategy:
      matrix:
        os:
          - ubuntu-24.04
          - ubuntu-24.04-arm
          - macos-15
          - windows-2025
        miri:
          - true
          - false
        type:
          - together
          - features
          - guest-feature
        exclude:
          - os: macos-15
            type: guest-feature
          - os: windows-2025
            type: guest-feature
    runs-on: ${{ matrix.os }}
    steps:
    # Many steps here
```

It is testing code on several operating systems, with/without Miri and already split into several parts to parallelize
the tests, but that was still kind of slow.

The slowest permutation out of the above are `together` type that compiles all tests with default features together and
runs them all. Other variants test certain things more selectively and are significantly shorter. On Windows
specifically, it was usually taking over 17 minutes, but it is relatively slow on other operating systems as well.

So we can't run CI faster than 17 minutes, but the whole workflow actually took 20 to 21 minutes in practice. This is
because there are limits on the number of free runners given, and it just so happened that the slow job on Windows was
starting after some of the faster runs were complete (completely undeterministic process though).

The way to fix it would be to make sure these slower jobs start first, and then faster jobs start later in whatever
order they like since they are likely to complete long before the slowest job anyway. GitHub Actions supports job
dependencies, so I came up with this small helper job:

```yaml
  cargo-test-slow-head-start:
    runs-on: ubuntu-24.04
    steps:
      - name: Artificial delay
        run: sleep 5
```

And then wanted to add it to dependencies for non-Windows jobs to give the Windows job a head start:

```yaml
    runs-on: ${{ matrix.os }}
    needs: ${{ contains(matrix.os, 'windows') && fromJSON('[]') || 'cargo-test-slow-head-start' }}
```

And... that [didn't work].

[didn't work]: https://github.com/orgs/community/discussions/163715

This would have been a clean solution, but unfortunately, it is not supported yet. Since I was not looking forward to
duplicating the large `steps` section into a separate Windows-specific job, I shelved the idea for a few months.

# Solution

But last week GitHub Actions introduced support for [Yaml anchors], so I could annotate anything in the workflow file
and reference it later without copying. Here is how it looks now:

```yaml
  cargo-test:
    strategy:
      matrix:
        os:
          - ubuntu-24.04
          - ubuntu-24.04-arm
          - macos-15
          - windows-2025
        miri:
          - true
          - false
        type:
          # `together` variant is running in a separate job, see `cargo-test-slow`
          # - together
          - features
          - guest-feature
        exclude:
          - os: macos-15
            type: guest-feature
          - os: windows-2025
            type: guest-feature

    runs-on: ${{ matrix.os }}
    # Gives the slow cargo test jobs a head start
    needs: cargo-test-slow-head-start
    env:
      command: ${{ matrix.miri == true && 'miri nextest run' || 'nextest run' }}

    steps: &test-steps
    # Many steps here

  cargo-test-slow:
    strategy:
      matrix:
        os:
          - ubuntu-24.04
          - ubuntu-24.04-arm
          - macos-15
          - windows-2025
        miri:
          - true
          - false
        type:
          - together
          # - features
          # - guest-feature
        exclude:
          - os: macos-15
            type: guest-feature
          - os: windows-2025
            type: guest-feature

    runs-on: ${{ matrix.os }}
    env:
      command: ${{ matrix.miri == true && 'miri nextest run' || 'nextest run' }}

    steps: *test-steps
```

I copied the matrix definition with everything except `together` in the original job and just `together` in the new
`cargo-test-slow` job, while reusing the same exact `steps`, which can be maintained like before. Note `&test-steps`
anchor and `*test-steps` reference to it.

Now my slower test jobs start 5+ seconds earlier, which is fast enough to not delay the rest of the CI too much and fast
enough to give the slow job a head start in scheduling.

# Results

The results are awesome! With Windows `together` job taking 17m10s, the whole CI run took 17m20s. So basically I get the
whole CI run for the time of the slowest one:
<p align="center">
<img alt="GitHub Actions CI matrix showing job times" src="ci-run-visualization.png">
</p>

I hope you find this trick useful, [PR 397] is where this was done and where you can find the final diff with the
changes described here.

[PR 397]: https://github.com/nazar-pc/abundance/pull/397

# Bonus content

You might be wondering: how the heck did I get that full-screen CI visualization? Well, since my feature request wasn't
implemented yet, I added [a small user style] to my browser to fix the annoyance and take advantage of the screen space
in my possession. With it, the visualization will occupy the full vertical space available in the browser window instead
of being limited to the miserable 600 pixels.

[a small user style]: https://github.com/orgs/community/discussions/164636#discussioncomment-14502175
