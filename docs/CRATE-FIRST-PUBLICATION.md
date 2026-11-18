# First publication of the three crates

This is the owner's one-time checklist for putting `tilewright-ui`,
`tilewright-app` and `tilewright-ui-render` on crates.io at 0.1.0. Every step
works from a phone's web browser; nothing needs Rust, a terminal or a login
on the build machine. After it, releases publish new versions automatically
through trusted publishing ([RELEASING.md](RELEASING.md)).

Use github.com in your phone's browser, not the GitHub app: the app has no
environment settings or secrets, and its "Run workflow" may not offer the
inputs. If a link opens the app, long-press it and open it in the browser. If
a page looks cut down, switch the browser to its desktop site. On the mobile
layout, a repository's **Settings** is under the **⋯** menu next to its tabs.

Labels on GitHub and crates.io change. If one differs slightly from what is
written here, look for the control that does the same thing; each step says
what the control does.

Why a token at all: crates.io's trusted publishing only publishes new
versions of crates that already exist ("initial publish requires an API
token", [crates.io docs](https://crates.io/docs/trusted-publishing), checked
2026-11-18). So the first version of each name goes up once with a
short-lived token that you create, that lives in GitHub for a few minutes as
an *environment secret* (a password GitHub hands only to jobs that run in
that environment), and that you then delete and revoke. The workflow that
uses it is the one that later publishes with trusted publishing. It packages
the release's tagged commit again, refuses to upload unless that gives exactly
the *attested* files (recorded and signed by the release workflow when it made
the release), and compares crates.io's *checksums* (a fingerprint of each
file) with them afterwards.

Three new names fit crates.io's new-crate limit (5 at once, then 1 per 10
minutes), so all three go up in one run.

**Until this checklist is done, leave the crate version at 0.1.0**: do not
merge a pull request that changes `version` in the root `Cargo.toml`. A
release that owns a newer crate version before step 9 is not published
automatically (step 10 covers it if it happens).

## Never

- Never paste the token into a chat, an issue, a pull request, a commit,
  Discord or a terminal on the build machine. It goes only into the GitHub
  environment secret in step 5.
- Never store it as a repository or organization secret, and never give it
  the `publish-update` scope or a wildcard crate pattern.
- Never create, delete or move a `v*` tag by hand, and never edit a release.
- Do not leave "Allow administrators to bypass" ticked on the environment.

## 1. Merge

On this pull request, check that **Source checks** is green, then
squash-merge it as usual.

**You should see:** in the Actions tab, a **Release** run on `main` whose only
job, **Plan release**, is green with the note "No release exists yet. A merge
never cuts the first release". Nothing is tagged or published, and **CI** on
`main` is green.

## 2. Create the release environment

In the phone's browser (not the app): `seraphinastudios/tilewright-ui`,
**Settings** (under **⋯** on the mobile layout), **Environments** in the
settings menu, **New environment**.

1. Name: `crates-io-release` (exactly), then **Configure environment**.
2. Leave **Required reviewers** and **Wait timer** off.
3. The branch rule first. In the dropdown that limits which branches may
   deploy (labelled **Deployment branches** or **Deployment branches and
   tags**), choose **Selected branches and tags**. Then **Add deployment
   branch or tag rule**: Ref type **Branch**, name pattern `main`, **Add
   rule**. There must be exactly this one rule.
4. Then untick **Allow administrators to bypass configured protection
   rules** and press **Save protection rules** (the button that saves this
   section).
5. Add no secrets or variables yet.
6. Leave the page, open the environment again, and check that both stuck:
   the single rule `main`, and the bypass box unticked. If either is wrong,
   fix it and save again. Step 4 also checks both by machine.

## 3. Cut the first release

In the browser: **Actions**, **Release** in the list of workflows, **Run
workflow** (the button above the list of runs), branch `main`, **Run
workflow**.

**You should see** (about 10 minutes): a green run, and under **Releases** a
release `v0.1.0` with five files: `tilewright-ui-0.1.0.crate`,
`tilewright-app-0.1.0.crate`, `tilewright-ui-render-0.1.0.crate`,
`tilewright-ui-0.1.0-source.tar.gz` and `SHA256SUMS`. Its last job, **Start
crate publication**, says "First publication pending: crates.io has no crate
named tilewright-ui tilewright-app tilewright-ui-render yet".

If the run shows **Cancelled** (a merge started its own run meanwhile), run
it again the same way.

## 4. Dry run, before any token exists

In the browser: **Actions**, **Publish crates**, **Run workflow**: branch
`main`, operation `verify`, credential `first-publication-token`, release_tag
`v0.1.0`.

**You should see** (about 3 minutes): one green job, **Verify the release's
crate archives**, whose summary says:

- "v0.1.0 owns crate version 0.1.0; to upload, in order: tilewright-ui
  tilewright-app tilewright-ui-render"
- "Checked: the environment crates-io-release admits only main, with no
  administrator bypass."
- "Checked: SHA256SUMS, GitHub's asset digests and build provenance from
  release.yml on main." (*Build provenance* is GitHub's signed record of
  which workflow built each file.)
- "Checked: packaging v0.1.0 again on this runner gives the attested bytes."
- "Not tested by a verify run: the crates.io token, its exchange, GitHub
  admitting the publish job to the environment, the publish job's own
  repackaging, the upload and the index confirmation."

Those untested parts run for the first time in step 6.

**If it is red**, read its error:

- "does not exist (… HTTP 404)": the environment is missing or misnamed; redo
  step 2 and run step 4 again.
- "must admit only the branch main, with no administrator bypass": fix step 2
  (steps 2.3 to 2.6) and run step 4 again.
- "could not read the environment" or anything else: run step 4 once more;
  if it is red again, stop and ask (below).

Do not go on to step 5 until step 4 is green.

## 5. Create the short-lived token and store it

1. On crates.io, log in with GitHub as `zoeyrose`, then open your account
   settings (**Account Settings**) and its **API Tokens** page, **New Token**.
2. Name: `tilewright-ui first publication`. Expiration: **Custom...**,
   tomorrow's date.
3. Scopes (what the token may do): tick **publish-new** only, which may
   only create crates that do not exist yet.
4. Crates (which names it may touch): with **Add pattern**, add the three
   patterns `tilewright-ui`, `tilewright-app`, `tilewright-ui-render` (each
   exactly, no `*`).
5. **Generate Token**, then copy it. crates.io shows it only once.
6. In the phone's browser (not the app): the repository's **Settings**,
   **Environments**, `crates-io-release`. Under **Environment secrets**, press
   the button that adds a secret (**Add environment secret** or **Add
   secret**). Name: `CRATES_IO_FIRST_PUBLICATION_TOKEN`. Value: paste the
   token. **Add secret**.

## 6. Publish the first versions

Right away, in the browser: **Actions**, **Publish crates**, **Run
workflow**: branch `main`, operation `publish`, credential
`first-publication-token`, release_tag `v0.1.0`.

**You should see** (about 5 minutes): the verify job green as in step 4, then
**Publish to crates.io** green, its last step saying "crates.io lists
tilewright-ui, tilewright-app, tilewright-ui-render at 0.1.0 with the attested
checksums".

**Check:** https://crates.io/crates/tilewright-ui,
https://crates.io/crates/tilewright-app and
https://crates.io/crates/tilewright-ui-render each show version 0.1.0 and
owner `zoeyrose`.

**If it fails:** `main` and the release are unchanged. Read the red step's
error:

| Error says | Do |
| --- | --- |
| no crates.io token | The secret's name or place is wrong (environment secret, exact name); fix step 5.6, run step 6 again |
| a 403 or "unauthorized" from crates.io | The token's scope or crate patterns are wrong: delete the secret, revoke the token, redo step 5, run again |
| 429 "too many new crates" | Wait 10 minutes per missing crate, run again |
| "gave up waiting for crates.io's index" | The uploads that succeeded are published. Wait 10 minutes and run step 6 again: it uploads only what is missing and checks the rest |
| anything else, or "other bytes" | Stop. Do step 7 now and ask (below) |

Running step 6 again is safe: it uploads only the crates crates.io still
lacks, and refuses names that already exist.

## 7. Remove the token

Both, right after step 6 succeeds (or when you stop):

1. In the phone's browser (not the app): the repository's **Settings**,
   **Environments**, `crates-io-release`, **Environment secrets**: delete
   `CRATES_IO_FIRST_PUBLICATION_TOKEN` (the bin icon or **Remove** beside it).
2. crates.io, **Account Settings**, **API Tokens**: **Revoke** `tilewright-ui
   first publication`.

**Check:** the environment shows no secrets; the token is gone from the list.

## 8. Add a trusted publisher to each crate

For each of `tilewright-ui`, `tilewright-app` and `tilewright-ui-render`: on
crates.io, open the crate's page, its **Settings** tab, the **Trusted
Publishing** section, **Add**. Publisher **GitHub**, then exactly:

| Field | Value |
| --- | --- |
| Repository owner | `seraphinastudios` |
| Repository name | `tilewright-ui` |
| Workflow filename | `publish-crate.yml` |
| Environment name (optional) | `crates-io-release`: fill it in even though it says optional; without it any branch could publish |

**Add**.

**Check:** the crate's **Settings** page lists exactly one trusted publisher,
showing Repository `seraphinastudios/tilewright-ui`, Workflow
`publish-crate.yml` and Environment `crates-io-release`. Remove any other.
(Optional, on a laptop:
`https://crates.io/api/v1/trusted_publishing/github_configs?crate=NAME` shows
the same as JSON, with `repository_owner_id` `314492348`.)

## 9. Final check run: prove trusted publishing

In the browser: **Actions**, **Publish crates**, **Run workflow**: branch
`main`, operation `publish`, credential `trusted-publishing`, release_tag
`v0.1.0`.

**You should see:** verify says "crates.io already has all three crates at
0.1.0 with this release's bytes. Nothing to upload."; **Publish to
crates.io** is green, and its step **Exchange the job's OIDC token for a
crates.io token** (GitHub's signed identity of the run, traded for a
30-minute crates.io token) succeeded. That proves the environment, the
workflow name and at least one trusted publisher; step 8's check covers the
other two.

**If the exchange step is red**, its message says which part of step 8 to fix:

| Message | Fix |
| --- | --- |
| "No Trusted Publishing config found" | A crate has no trusted publisher, or the repository owner or name is wrong: redo step 8 |
| "… does not match the workflow filename …" | The workflow filename must be exactly `publish-crate.yml` |
| "… does not match the environment …" | The environment name must be exactly `crates-io-release` |
| "… does not match the repository owner ID …" | Delete that trusted publisher and add it again with owner `seraphinastudios` |

Then run step 9 again.

## 10. Only if a later release owns a newer crate version

Normally skip this. If a release after `v0.1.0` was cut with a newer crate
version before step 9 (its **Start crate publication** said "First
publication pending" and names that version), run step 9 again with that
release's tag: it uploads that version with trusted publishing.

## Done

- crates.io shows the three crates at 0.1.0, owned by `zoeyrose`.
- docs.rs shows https://docs.rs/tilewright-ui/0.1.0 and the other two built
  (it can take an hour; a docs.rs failure does not affect the crates).
- The environment has no secret; crates.io lists no `tilewright-ui first
  publication` token.
- Optional, after the first version that trusted publishing uploads: each
  crate's **Settings**, turn on **Require trusted publishing for all new
  versions**.

From then on, a pull request that bumps the crate version publishes the
three crates when it merges. A published version can never be replaced, only
*yanked* (hidden from new users, still downloadable for existing ones); a
fix always goes out as a new version.

## If you are stuck

Ask in the Tilewright coordinating session with the link to the red run.
Do not paste the token there. A failed step leaves `main` and the release
unchanged; only step 6 uploads anything.

## Alternative: a laptop terminal

With Rust installed through rustup, from a clean clone of the `v0.1.0` tag
(`git clone https://github.com/seraphinastudios/tilewright-ui && cd
tilewright-ui && git checkout v0.1.0`): `cargo login` (paste the token at the
prompt), then `cargo publish --locked -p tilewright-ui`, then the same for
`tilewright-app` and `tilewright-ui-render`, then `cargo logout` and revoke the
token. It keeps the token off GitHub, but `cargo publish` builds every
dependency on the laptop while the token is stored there, and nothing checks
the uploads against the release's files. Steps 4 to 7 are preferred.
