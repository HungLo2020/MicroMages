---
name: commit-and-verify-ci
description: Commit changes, push them, and drive the GitHub "Test and release" workflow to a fully green result, including the validate job and the rolling `latest` release on main. Use whenever the user asks to commit, push, land, ship, release, or "make CI pass" / "check the workflows" for this repository.
---

# Commit and verify CI

Take a working tree with reviewed changes to a green GitHub Actions run, and on
`main` to a verified rolling `latest` release. Work sequentially: never push
before the change is committed, and never claim success from a local run alone.

## 1. Preflight

1. `git status --short` and `git diff` (plus `git diff --cached` if anything is
   staged). Read the whole diff; if unrelated edits are present, ask what to
   include instead of guessing.
2. `git branch --show-current` and `git remote -v`. Confirm the remote is
   `origin` → `https://github.com/HungLo2020/MicroMages`. Note whether the branch
   is `main`; only `main` publishes the rolling release.
3. `git fetch origin` and check divergence from `origin/<branch>`. Never
   force-push `main` and never rewrite already-published history.

## 2. Mirror CI locally

Run every command from `.github/workflows/release.yml` that can run locally,
in this order, and stop at the first failure:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
python3 -m unittest discover -s DevUtils
cargo build --locked --release
python3 DevUtils/package_linux.py --revision "$(git rev-parse HEAD)"
python3 DevUtils/verify_package.py dist/MicroMages-latest-linux-x64.zip
python3 DevUtils/verify_audio.py dist/MicroMages-latest-linux-x64.zip
xvfb-run -a python3 DevUtils/verify_desktop.py dist/MicroMages-latest-linux-x64.zip
```

Notes:

- `verify_desktop.py` needs a display; use `xvfb-run` exactly like CI.
- Missing system packages (`libsdl2-dev libasound2-dev libudev-dev python3-pil
  xvfb xdotool openbox`) are an environment problem, not a code problem. Tell the
  user rather than pushing a change you could not validate, and do not weaken or
  skip tests to get past them.
- The packaging step uses `HEAD`; if you are validating before committing, run it
  again after committing so `build-info.json` matches the pushed commit.
- Fix root causes. Never regenerate `tests/fixtures/` reference data, relax
  Clippy lints, add `#[allow(...)]`, or mark tests ignored just to get green; see
  the AGENTS section of `README.md`.

## 3. Commit

1. Stage explicitly by path (`git add <paths>`). Do not stage `target/`, `dist/`,
   `data/`, `DevUtils/__pycache__`, or anything gitignored.
2. Write one commit per logical change with a short imperative subject and a body
   explaining *why* when the change is not obvious. Match existing history style:
   sentence-case subjects, no trailing period, no `[skip ci]` or CI bypasses.
3. No attribution trailers, `Co-Authored-By`, or tool signatures.
4. Verify the commit contains what you expect: `git show --stat HEAD`.

## 4. Push

```sh
git push origin HEAD
```

Report the pushed commit SHA. Replace the local packaging run with the pushed SHA
if it changed.

## 5. Watch the workflow

1. Find the run for that SHA (the timing of a run can lag the push by seconds):

   ```sh
   gh run list --branch "$(git branch --show-current)" --limit 5 \
     --json databaseId,headSha,status,conclusion,url,event
   ```

2. Wait for completion and print the result:

   ```sh
   gh run watch <run-id> --exit-status
   gh run view <run-id> --json status,conclusion,jobs
   ```

3. Confirm **every** job:

   - `validate` — formatting, Clippy, tests, unit tests, build, package and the
     three verifiers. This runs for every branch and every pull request.
   - `publish` — must be `success` on `main`. It is legitimately `skipped` on other
     branches and on pull requests; say so rather than reporting a failure.

4. On failure, read the actual log before changing code:

   ```sh
   gh run view <run-id> --log-failed
   ```

   Reproduce the failing step locally, fix the cause, run the matching local
   check, then commit and push again (a new commit, not an amend of published
   history). Network flaps and Actions/APT outages can be retried once with
   `gh run rerun <run-id> --failed`, but never to hide a real failure.

## 6. Verify the release (on `main`)

`publish` runs `DevUtils/publish_latest.py`, which replaces a single mutable
`latest` release. It silently declines to publish when a newer commit has landed
on `main`, so verify it actually shipped *this* commit:

```sh
gh release view latest --json tagName,name,isDraft,targetCommitish,url,assets \
  --jq '{tagName,name,isDraft,targetCommitish,url,assets:[.assets[].name]}'
git ls-remote origin refs/tags/latest
```

Requirements:

- Tag `latest` points at the pushed commit (`git ls-remote` SHA matches).
- The release is not a draft and contains exactly
  `MicroMages-latest-linux-x64.zip` and `MicroMages-latest-linux-x64.zip.sha256`.
- Download and spot-check the published artifact:

  ```sh
  rm -rf /tmp/opencode/release-check && mkdir -p /tmp/opencode/release-check
  gh release download latest --pattern '*linux-x64.zip*' --dir /tmp/opencode/release-check
  cd /tmp/opencode/release-check && sha256sum -c MicroMages-latest-linux-x64.zip.sha256
  python3 /home/matt/Documents/Repos/MicroMages/DevUtils/verify_package.py \
    MicroMages-latest-linux-x64.zip
  ```

- Do not create tags by hand, do not edit release assets manually, and do not
  change release immutability settings; the script owns the rolling release.

## 7. Report

Summarize: the commit SHA and subject, the local commands run, the workflow run
URL and per-job conclusions, and (on `main`) the `latest` release URL, tag SHA,
asset names and sizes, and the checksum result. State plainly if anything was
skipped or could not be verified.
