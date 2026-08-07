Chunking policy

Every indexable file is split into retrieval units ("chunks") that carry
provenance: the file path relative to the project root, and the 1-based
inclusive first and last source line numbers.

Target size

TARGET_CHUNK_CHARS in src/chunk.rs is 1800 characters, roughly 400-500
tokens. This matches the embedder's 512-token truncation so chunks embed
fully without loss.

Paragraph alignment

Files are split on blank-line boundaries first: consecutive non-blank
lines form a paragraph (a function body, a doc section, a config block),
and paragraphs are greedily packed into chunks up to the target size. A
chunk is therefore never cut inside a function body or a prose paragraph —
retrieval returns whole, readable units instead of mid-statement slices.

Oversized paragraphs

A single paragraph larger than the target (minified files, generated
bundles, very long doc lines) is hard-split on whole-line boundaries into
pieces of at most the target size. Lines themselves are never cut: a line
longer than the target stays whole as its own chunk. This keeps the
indexer robust for code that has no blank lines for hundreds of lines.

Empty and binary files

Empty files produce no chunks. Files whose first 8 KiB contain a NUL byte
are treated as binary and skipped entirely (src/scan.rs read_text).

Line numbers

line_start and line_end are 1-based and inclusive, tracked per paragraph
and propagated through packing and hard splits. For a chunk that packs
several paragraphs, the range spans the first line of the first paragraph
to the last line of the last. Chunks of one file are emitted in source
order with contiguous, non-overlapping ranges, and file_context returns
them in that order so a reader can reconstruct the file.

Why paragraph alignment matters

Vector search on mid-function slices tends to surface fragments without
context, and BM25 on them inflates keyword scores for boilerplate. Whole
function bodies and sections keep both retrieval arms meaningful and make
the returned text directly usable as context for an answering model.
