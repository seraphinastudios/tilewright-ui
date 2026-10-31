# Source provenance

The initial shared UI/application implementation is MIT-licensed work authored
for Seraphina Studios in this repository. Classic Atrinik and proprietary game
implementations, assets and fixtures are not reusable inputs to these libraries.

Any exact extraction from MIT Tilewright editor code must be recorded here with
its source revision, paths, original notices, destination and transformations
before its changes are accepted. Renderer fonts remain separately licensed data
and retain their existing verified manifests and notices.

## Recorded extractions

See [text editing provenance](crates/tilewright-ui/PROVENANCE.md),
[renderer adapter provenance](crates/tilewright-ui-render/PROVENANCE.md), and
[native host provenance](examples/native/PROVENANCE.md) for the exact MIT source
revisions, retained notices and adaptations.

## Font preparation

The MIT font-delivery helper `tools/font_pack.py` and its original synthetic regression suite `tools/test_font_pack.py` are copied byte-for-byte from `seraphinastudios/tilewright-renderer` revision `4488ffb2e3ba2ee9edc5dc48a4b7659bf432f698` (2026 Seraphina Studios). Their SHA-256 digests are respectively `cbffc180a44fc473ecad0e7fcd9b6b50e32b4a06b7b3f404a0999a7be6ea6469` and `7a92e12aebbb2ab57a41884cd0af9270df129023b721d070a451b0d1086a19e0`. The unchanged font inventory `tools/unicode-font-pack.json` comes from that revision's `assets/unicode/manifest.json`, SHA-256 `94b81a712fb41a66eb744c88ae2ed312cc5d0ec1a491e9821fcc278d99410f13`.

Font payload remains outside the repository and Rust packages. Explicit `tools/prepare-fonts.py DESTINATION --acquire` preparation fetches only the manifest's exact public `google/fonts` paths at commit `6c1dbe27dcf7d817d4b62cec033ba77fa8e2a5ec`. No ambient credentials/proxies, redirects, system font search, or runtime acquisition are used. Each font and accompanying OFL notice must match the pinned length and SHA-256. The final directory is verified against the exact inventory, rejecting additional files, directories, links, changed notices or shortened content. Fonts retain OFL-1.1 and their accompanying `OFL.txt` notices; they are not relicensed under the repository's MIT license. The immutable packaged archive is not downloaded or extracted by this workflow.
