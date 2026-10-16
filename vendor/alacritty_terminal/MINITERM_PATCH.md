# MiniTerm terminal-core patch

Source: crates.io `alacritty_terminal` 0.25.1, upstream Alacritty commit
`12082407d9cb3384a2bd83c578e556ec3faa2c4d` (Apache-2.0; see LICENSE-APACHE).
The published crate's source and manifests are retained. Registry cache files
and the upstream reference-fixture test target are omitted.

Local change: `Cell::push_zerowidth` retains at most 64 suffix codepoints per
cell, checking before copy-on-write. Excess combining characters are ignored.
This follows the resource bound in Ghostty `src/terminal/page.zig` at commit
`22fa134a07f5749edd9e92c6bc17d8aaf61843be`. No Ghostty code is copied.
The app-level parser regression in `src/terminal_budget.rs` covers split input,
wide cells, copying and parser progress after the limit.

Keep this small patch when updating the pinned engine, or remove it only after
the upstream engine provides equivalent bounded cell storage. MiniTerm source
fingerprints include this directory's Rust source and manifests.
