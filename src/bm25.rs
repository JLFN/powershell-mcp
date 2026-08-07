//! BM25 full-text index (tantivy) over the chunk corpus, ported from the
//! ebook-knowledge recipe.

use anyhow::{Context, Result};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, IndexRecordOption, OwnedValue, Schema, TextFieldIndexing, INDEXED, STORED, TEXT,
};
use tantivy::tokenizer::{
    Language, LowerCaser, RemoveLongFilter, SimpleTokenizer, Stemmer, TextAnalyzer,
};
use tantivy::{doc, Index, ReloadPolicy, TantivyDocument};

use crate::chunk::Chunk;

/// A BM25 hit: which chunk, and its raw BM25 score.
pub struct Bm25Hit {
    pub chunk_index: usize,
    pub score: f32,
}

/// Build (or rebuild) the Tantivy index for the corpus. Tantivy is
/// immutable, so a rebuild deletes and recreates the index directory.
pub fn index_chunks(tantivy_dir: &Path, chunks: &[Chunk]) -> Result<()> {
    if tantivy_dir.exists() {
        std::fs::remove_dir_all(tantivy_dir)
            .with_context(|| format!("remove old index {}", tantivy_dir.display()))?;
    }
    std::fs::create_dir_all(tantivy_dir)
        .with_context(|| format!("create index dir {}", tantivy_dir.display()))?;

    let mut schema_builder = Schema::builder();
    let body = schema_builder.add_text_field(
        "body",
        TEXT.set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("en_stem")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        ),
    );
    let chunk_index = schema_builder.add_i64_field("chunk_index", INDEXED | STORED);
    let schema = schema_builder.build();

    let index = Index::create_in_dir(tantivy_dir, schema)
        .with_context(|| format!("create index in {}", tantivy_dir.display()))?;
    index.tokenizers().register(
        "en_stem",
        TextAnalyzer::builder(SimpleTokenizer::default())
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser)
            .filter(Stemmer::new(Language::English))
            .build(),
    );

    let mut writer = index.writer(50_000_000).context("create tantivy writer")?;
    for c in chunks {
        writer.add_document(doc!(
            body => c.text.as_str(),
            chunk_index => c.index as i64,
        ))?;
    }
    writer.commit().context("tantivy commit")?;
    writer
        .wait_merging_threads()
        .context("wait for tantivy merge")?;
    Ok(())
}

/// BM25 search over the corpus, returning the top chunks by score.
pub fn search_chunks(tantivy_dir: &Path, question: &str, limit: usize) -> Result<Vec<Bm25Hit>> {
    let index = Index::open_in_dir(tantivy_dir)
        .with_context(|| format!("open index {}", tantivy_dir.display()))?;
    let schema = index.schema();
    let body: Field = schema.get_field("body").context("schema field body")?;
    let chunk_index: Field = schema
        .get_field("chunk_index")
        .context("schema field chunk_index")?;

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()
        .context("create tantivy reader")?;
    let searcher = reader.searcher();

    let parser = QueryParser::for_index(&index, vec![body]);
    let query = parser.parse_query(question).context("parse query")?;
    let top = searcher
        .search(&query, &TopDocs::with_limit(limit))
        .context("tantivy search")?;

    let mut hits = Vec::with_capacity(top.len());
    for (score, addr) in top {
        let doc: TantivyDocument = searcher.doc(addr).context("fetch tantivy doc")?;
        if let Some(OwnedValue::I64(idx)) = doc.get_first(chunk_index) {
            hits.push(Bm25Hit {
                chunk_index: *idx as usize,
                score,
            });
        }
    }
    Ok(hits)
}
