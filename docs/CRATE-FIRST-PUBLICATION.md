# First publication of the three crates

This is the owner's one-time checklist for putting `tilewright-ui`,
`tilewright-app` and `tilewright-ui-render` on crates.io at 0.1.0. Every step
works from a phone browser or a laptop browser; nothing needs Rust, a terminal
or a login on the build machine. After it, releases publish new versions
automatically through trusted publishing ([RELEASING.md](RELEASING.md)).

Why a token at all: crates.io's trusted publishing only publishes new
versions of crates that already exist ("initial publish requires an API
token", [crates.io docs](https://crates.io/docs/trusted-publishing), checked
2026-11-18). So the first version of each name is uploaded once with a
short-lived token that you create, that lives in GitHub for a few minutes as
an environment secret, and that you then delete and revoke. The workflow that
uses it is the same one that later publishes with trusted publishing, and it
uploads the exact bytes the release attested.

Three new names fit crates.io's new-crate limit (5 at once, then 1 per 10
minutes), so all three go up in one run.

## Never

- Never paste the token into a chat, an issue, a pull request, a commit,
  Discord or a terminal on the build machine. It goes only into the GitHub
  environment secret in step 5.
- Never store it as a repository or organization secret, and never give it
  the `publish-update` scope or a wildcard crate pattern.
- Never delete, move or recreate a `v*` tag or edit a release.
- Do not tick "Allow administrators to bypass" on the environment.

## 1. Before merging

1. Merge #13 (renderer 0.7.2) first; this pull request contains its commit.
2. On this pull request, check that **Source checks** is green.

## 2. Merge

Squash-merge this pull request as usual.

**You should see:** in the Actions tab, a **Release** run on `main` whose only
job, **Plan release**, is green with the note "No release exists yet. A merge
never cuts the first release". Nothing is tagged or published, and **CI** on
`main` is green.

## 3. Create the release environment

GitHub, `seraphinastudios/tilewright-ui`, **Settings**, **Environments**,
**New environment**.

1. Name: `crates-io-release` (exactly). **Configure environment**.
2. Leave **Required reviewers** and **Wait timer** off.
3. Untick **Allow administrators to bypass configured protection rules**.
4. **Deployment branches and tags**: choose **Selected branches and tags**,
   **Add deployment branch or tag rule**, Ref type **Branch**, pattern
   `main`, **Add rule**. There must be exactly this one rule.
5. **Save protection rules**. Add no variables.

**Check:** the Environments page lists `crates-io-release` with "1 branch
allowed" (or open it and see the single rule `main`).

## 4. Cut the first release

**Actions**, **Release**, **Run workflow**, branch `main`, **Run workflow**.

**You should see** (about 10 minutes): a green run, and under **Releases** a
release `v0.1.0` with five assets: `tilewright-ui-0.1.0.crate`,
`tilewright-app-0.1.0.crate`, `tilewright-ui-render-0.1.0.crate`,
`tilewright-ui-0.1.0-source.tar.gz` and `SHA256SUMS`. Its last job, **Start
crate publication**, says "First publication pending: crates.io has no crate
named tilewright-ui tilewright-app tilewright-ui-render yet".

## 5. Create the short-lived token and store it

1. On crates.io, **Log in with GitHub** as `zoeyrose`. **Account Settings**,
   **API Tokens**, **New Token**.
2. Name: `tilewright-ui first publication`. Expiration: **Custom**, tomorrow's
   date.
3. Scopes: tick **publish-new** only.
4. Crates: add the three patterns `tilewright-ui`, `tilewright-app`,
   `tilewright-ui-render` (each exactly, no `*`).
5. **Generate Token**, then copy it. crates.io shows it only once.
6. In GitHub, **Settings**, **Environments**, `crates-io-release`,
   **Environment secrets**, **Add environment secret**. Name:
   `CRATES_IO_FIRST_PUBLICATION_TOKEN`. Value: paste the token. **Add secret**.

## 6. Publish the first versions

**Actions**, **Publish crates**, **Run workflow**: branch `main`, operation
`publish`, credential `first-publication-token`, release_tag `v0.1.0`.

**You should see** (about 5 minutes): **Verify the release's crate archives**
green with "v0.1.0 owns crate version 0.1.0; to upload, in order:
tilewright-ui tilewright-app tilewright-ui-render", then **Publish to
crates.io** green, its last step saying "crates.io lists tilewright-ui,
tilewright-app, tilewright-ui-render at 0.1.0 with the attested checksums".

**Check:** https://crates.io/crates/tilewright-ui,
https://crates.io/crates/tilewright-app and
https://crates.io/crates/tilewright-ui-render each show version 0.1.0 and
owner `zoeyrose`.

**If it fails:** nothing is wrong with `main`. Read the red step's error:

| Error says | Do |
| --- | --- |
| the environment `crates-io-release` is missing or must admit only main | Fix step 3, run step 6 again |
| no crates.io token | The secret's name or place is wrong (environment secret, exact name); fix step 5.6, run again |
| a 403 or "unauthorized" from crates.io | The token's scope or crate patterns are wrong; delete it, redo step 5, run again |
| 429 "too many new crates" | Wait 10 minutes per missing crate, run again; it uploads only what is missing |
| anything else, or "other bytes" | Stop. Delete the secret and revoke the token (step 7) and ask (below) |

Running step 6 again is safe: it uploads only the crates crates.io still
lacks, and refuses names that already exist.

## 7. Remove the token

Do both, right after step 6 succeeds (or when you give up):

1. GitHub, `crates-io-release`, **Environment secrets**: delete
   `CRATES_IO_FIRST_PUBLICATION_TOKEN`.
2. crates.io, **Account Settings**, **API Tokens**: **Revoke** `tilewright-ui
   first publication`.

**Check:** the environment shows no secrets; the token is gone from the list.

## 8. Add a trusted publisher to each crate

For each of `tilewright-ui`, `tilewright-app` and `tilewright-ui-render`: on
crates.io, the crate's page, **Settings**, **Trusted Publishing**, **Add**,
**GitHub**, and enter exactly:

| Field | Value |
| --- | --- |
| Repository owner | `seraphinastudios` |
| Repository name | `tilewright-ui` |
| Workflow filename | `publish-crate.yml` |
| Environment | `crates-io-release` |

Always fill in the environment: without it any branch could publish.

**Check:** in the same browser, open
`https://crates.io/api/v1/trusted_publishing/github_configs?crate=NAME` for
each name. Each lists exactly one config with `repository_owner`
`seraphinastudios`, `repository_owner_id` `314492348`, `repository_name`
`tilewright-ui`, `workflow_filename` `publish-crate.yml` and `environment`
`crates-io-release`. Delete any other.

## 9. Prove trusted publishing

**Actions**, **Publish crates**, **Run workflow**: branch `main`, operation
`publish`, credential `trusted-publishing`, release_tag `v0.1.0`.

**You should see:** verify says "crates.io already has all three crates at
0.1.0 with this release's bytes. Nothing to upload."; **Publish to
crates.io** is green, and its step **Exchange the job's OIDC token for a
crates.io token** succeeded. That proves the environment, the workflow name
and at least one trusted publisher; step 8's check covers the other two. If
the exchange fails with "No Trusted Publishing config found", redo step 8.

## Done

- crates.io shows the three crates at 0.1.0, owned by `zoeyrose`.
- docs.rs shows https://docs.rs/tilewright-ui/0.1.0 and the other two built
  (it can take an hour; a docs.rs failure does not affect the crates).
- The environment has no secret; crates.io lists no `tilewright-ui first
  publication` token.
- Optional, after the first version that trusted publishing uploads: each
  crate's **Settings**, **Trusted Publishing**, turn on **Require trusted
  publishing for all new versions**.

From then on, a pull request that bumps the crate version publishes the
three crates when it merges.

## If you are stuck

Ask in the Tilewright coordinating session with the link to the red run.
Do not paste the token there. A failed step leaves `main` and the release
unchanged; only step 6 uploads anything, and only the release's own bytes.

## Alternative: a laptop terminal

With Rust installed through rustup, from a clean clone of the `v0.1.0` tag
(`git clone https://github.com/seraphinastudios/tilewright-ui && cd
tilewright-ui && git checkout v0.1.0`): `cargo login` (paste the token at the
prompt), then `cargo publish --locked -p tilewright-ui`, then the same for
`tilewright-app` and `tilewright-ui-render`, then `cargo logout` and revoke the
token. It needs a Rust toolchain on the laptop, builds there, and nothing
checks that the uploads equal the release's attested `.crate` bytes, so step
6 is preferred.
