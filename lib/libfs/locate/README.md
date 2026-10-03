# chaos-locate

Fast fuzzy file search tool for Chaos.

Uses a single <https://crates.io/crates/ignore> walk per search root and local
case-insensitive subsequence scoring, with bonuses for contiguous path and
basename matches. Sessions reuse the scanned paths for query updates, stream
top-N snapshots, and support cancellation and character-index highlighting.

Ignore files below each explicit root are honored, including nested rules and
negations; parent ignore files are not loaded. Hidden files are included by
default, `.git` metadata is skipped, and ignore processing can be disabled.
Search does not collect Git status, commit recency, or frecency metadata.
