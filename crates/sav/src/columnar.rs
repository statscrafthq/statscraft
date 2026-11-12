//! Direct-to-columnar Arrow batch building.
//!
//! Eliminates the `Vec<Vec<CellValue>>` intermediate by pushing decoded values
//! directly from decompressed slots into pre-allocated Arrow column builders.
//!
//! **Performance rule:** The hot paths (`push_raw_chunk`) must stay minimal:
//! only Float64 + String. Date and time variables stay the stored numbers.
//! Never add new ColBuilder variants or match arms to the hot loops; it causes
//! icache pressure that slows ALL columns.

use std::sync::Arc;

use arrow::array::{ArrayRef, Float64Builder, StringViewBuilder};
use arrow::datatypes::{Field, Schema};
use arrow::record_batch::RecordBatch;
use encoding_rs::Encoding;
use rayon::prelude::*;

use crate::arrow_convert;
use crate::constants::{VarType, is_sysmis};
use crate::dictionary::ResolvedDictionary;
use crate::encoding;
use crate::error::Result;
use crate::io_utils;
use crate::variable::VariableRecord;

/// Row byte threshold for switching to tiled parallel column processing.
/// When row_bytes exceeds this, the full-chunk column-at-a-time pattern
/// thrashes L3 cache because each column pass scans the entire 256 MB chunk
/// with only 8 useful bytes per large stride. Tiling into L3-sized row chunks
/// keeps the working set cache-hot for all rayon threads.
///
/// At 12,288 bytes (~1,536 slots), files with ~915 visible columns (which may
/// have ~1,736 total slots due to multi-slot string variables) correctly hit
/// the tiled path.
const WIDE_ROW_THRESHOLD: usize = 12_288;

/// Target tile size in bytes for wide-file row tiling.
/// Each tile should comfortably fit in L3 cache (~12-36 MB on modern CPUs)
/// so all rayon threads access cache-hot data during parallel column processing.
/// 4 MB leaves headroom for builder state and other cache residents.
const L3_TILE_BYTES: usize = 4 * 1024 * 1024;

/// Pre-computed info for one VLS (very long string) segment.
/// VLS variables (width > 255) are stored across multiple 32-slot segments.
/// Each segment occupies 32 slots (256 bytes), storing up to 255 bytes of data.
/// Note: n_segments is computed with divisor 252 (matching file record layout),
/// so trailing segments may have 0 useful bytes.
struct VlsSegmentInfo {
    /// Number of useful data bytes in this segment (up to 255, 0 for trailing
    /// empty segments).
    useful_bytes: usize,
}

/// Pre-computed mapping from a visible variable to its slot position and type.
struct ColumnMapping {
    /// Starting slot index in the raw slot array.
    slot_index: usize,
    /// Width in bytes of a string variable (0 for a numeric one).
    width: usize,
    /// Number of segments for very long strings (1 for normal vars).
    n_segments: usize,
    /// Pre-computed VLS segment layout (empty for n_segments <= 1).
    vls_layout: Vec<VlsSegmentInfo>,
}

/// Arrow column builder (typed). Only two variants: keeps hot path icache small.
enum ColBuilder {
    Float64(Float64Builder),
    Str(StringViewBuilder),
}

/// Builds an Arrow RecordBatch by pushing values directly from decompressed
/// slots into columnar builders, skipping the `CellValue` intermediate.
pub struct ColumnarBatchBuilder {
    mappings: Vec<ColumnMapping>,
    builders: Vec<ColBuilder>,
    file_encoding: &'static Encoding,
    /// The output schema.
    schema: Arc<Schema>,
    rows_appended: usize,
    /// Reusable byte buffer for string assembly (avoids per-string allocation).
    /// Used only in the sequential path; parallel paths use thread-local buffers.
    string_buf: Vec<u8>,
}

impl ColumnarBatchBuilder {
    /// Create a new builder from a resolved dictionary.
    ///
    /// If `projection` is Some, only the specified variable indices are built.
    /// `capacity` is the expected number of rows (for pre-sizing builders).
    pub fn new(dict: &ResolvedDictionary, projection: Option<&[usize]>, capacity: usize) -> Self {
        let vars: Vec<&VariableRecord> = match projection {
            Some(proj) => proj.iter().map(|&i| &dict.variables[i]).collect(),
            None => dict.variables.iter().collect(),
        };

        let mut mappings = Vec::with_capacity(vars.len());
        let mut builders = Vec::with_capacity(vars.len());
        let mut fields = Vec::with_capacity(vars.len());
        for var in &vars {
            // Pre-compute VLS segment layout
            let vls_layout = if var.n_segments > 1 {
                let width = match &var.var_type {
                    VarType::String(w) => *w,
                    _ => 0,
                };
                (0..var.n_segments)
                    .map(|seg| {
                        // Each segment stores up to 255 bytes of content in
                        // 256 bytes (32 slots) of data space. The file may
                        // allocate more segments than needed (n_segments uses
                        // divisor 252 for record layout), so use saturating
                        // subtraction to handle trailing empty segments.
                        let bytes_before = seg * 255;
                        let remaining = width.saturating_sub(bytes_before);
                        let seg_useful = remaining.min(255);
                        VlsSegmentInfo {
                            useful_bytes: seg_useful,
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };

            mappings.push(ColumnMapping {
                slot_index: var.slot_index,
                width: match var.var_type {
                    VarType::String(w) => w,
                    VarType::Numeric => 0,
                },
                n_segments: var.n_segments,
                vls_layout,
            });

            let output_type = arrow_convert::var_to_arrow_type(var);
            fields.push(Field::new(&var.long_name, output_type, true));

            match &var.var_type {
                VarType::Numeric => {
                    // ALL numerics use Float64Builder: keeps hot path minimal
                    builders.push(ColBuilder::Float64(Float64Builder::with_capacity(capacity)));
                }
                VarType::String(_) => {
                    // Only enable dedup for categorical columns (those with value labels).
                    // High-cardinality columns (IDs, free-text) pay hash overhead with no benefit.
                    let has_value_labels = dict
                        .metadata
                        .variable_value_labels
                        .contains_key(&var.long_name);
                    let sb = if has_value_labels {
                        StringViewBuilder::new().with_deduplicate_strings()
                    } else {
                        StringViewBuilder::new()
                    };
                    builders.push(ColBuilder::Str(sb));
                }
            }
        }

        ColumnarBatchBuilder {
            mappings,
            builders,
            file_encoding: dict.file_encoding,
            schema: Arc::new(Schema::new(fields)),
            rows_appended: 0,
            string_buf: Vec::with_capacity(1024),
        }
    }

    /// Push a chunk of raw bytes into columnar builders.
    /// `chunk` holds `num_rows` rows of `slots_per_row * 8` bytes each; a shorter
    /// chunk only yields the complete rows it holds.
    ///
    /// Dispatch strategy:
    /// - **Wide files** (row_bytes > 12,288): tiled parallel, processes L3-sized
    ///   row tiles with rayon, avoiding cache thrashing from large column strides.
    /// - **Narrow files, large chunks** (at least 2^18 values): column-at-a-time with
    ///   rayon parallelism. Each thread fills its own builder independently.
    /// - **Narrow files, small chunks**: sequential column-at-a-time.
    pub fn push_raw_chunk(&mut self, chunk: &[u8], num_rows: usize, slots_per_row: usize) {
        if slots_per_row == 0 {
            return;
        }
        let (slots, _) = chunk.as_chunks::<8>();
        let rows = num_rows.min(slots.len() / slots_per_row);
        let slots = &slots[..rows * slots_per_row];
        let row_bytes = slots_per_row * 8;

        // Wide files: tiled parallel avoids L3 cache thrashing.
        // Column-at-a-time with large stride (e.g. 14,656 bytes for 1832 slots)
        // causes each column pass to touch the entire 256 MB chunk with only 8
        // useful bytes per stride, thrashing L3 cache. Tiling processes small
        // row windows that fit in L3, keeping data cache-hot for all rayon threads.
        if row_bytes > WIDE_ROW_THRESHOLD {
            let tile_rows = (L3_TILE_BYTES / row_bytes).max(64);
            for tile in slots.chunks(tile_rows * slots_per_row) {
                self.push_rows(tile, slots_per_row, true);
            }
        } else {
            self.push_rows(slots, slots_per_row, slots.len() >= 1 << 18);
        }
        self.rows_appended += rows;
    }

    /// Push whole rows into the builders, one column at a time, in parallel
    /// over columns when `parallel` is set.
    fn push_rows(&mut self, slots: &[[u8; 8]], slots_per_row: usize, parallel: bool) {
        let mappings = &self.mappings;
        let file_encoding = self.file_encoding;
        if parallel {
            self.builders
                .par_iter_mut()
                .zip(mappings.par_iter())
                .for_each(|(builder, mapping)| {
                    let mut local_buf = Vec::with_capacity(256);
                    push_column(
                        builder,
                        &mut local_buf,
                        slots,
                        slots_per_row,
                        mapping,
                        file_encoding,
                    );
                });
        } else {
            for (builder, mapping) in self.builders.iter_mut().zip(mappings) {
                push_column(
                    builder,
                    &mut self.string_buf,
                    slots,
                    slots_per_row,
                    mapping,
                    file_encoding,
                );
            }
        }
    }

    /// Finish building and return the RecordBatch.
    pub fn finish(self) -> Result<RecordBatch> {
        let columns: Vec<ArrayRef> = self
            .builders
            .into_iter()
            .map(|b| -> ArrayRef {
                match b {
                    ColBuilder::Float64(mut b) => Arc::new(b.finish()),
                    ColBuilder::Str(mut b) => Arc::new(b.finish()),
                }
            })
            .collect();
        Ok(RecordBatch::try_new(self.schema, columns)?)
    }

    /// Number of rows appended so far.
    pub fn len(&self) -> usize {
        self.rows_appended
    }
}

// ---------------------------------------------------------------------------
// Column processing helpers (shared by parallel, sequential, and tiled paths)
// ---------------------------------------------------------------------------

/// Append one column of whole rows (`slots_per_row` slots each) to its builder.
#[inline(always)]
fn push_column(
    builder: &mut ColBuilder,
    string_buf: &mut Vec<u8>,
    slots: &[[u8; 8]],
    slots_per_row: usize,
    mapping: &ColumnMapping,
    file_encoding: &'static Encoding,
) {
    match builder {
        ColBuilder::Float64(builder) => {
            for row in slots.chunks_exact(slots_per_row) {
                match row.get(mapping.slot_index) {
                    Some(bytes) if !is_sysmis(f64::from_le_bytes(*bytes)) => {
                        builder.append_value(f64::from_le_bytes(*bytes))
                    }
                    _ => builder.append_null(),
                }
            }
        }
        ColBuilder::Str(builder) => {
            for row in slots.chunks_exact(slots_per_row) {
                push_string_from_raw_slots(
                    builder,
                    string_buf,
                    row,
                    mapping.slot_index,
                    mapping.width,
                    mapping.n_segments,
                    &mapping.vls_layout,
                    file_encoding,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// String push helpers
// ---------------------------------------------------------------------------

/// Assemble a string from raw 8-byte slots and push directly into a StringViewBuilder.
#[inline]
#[allow(clippy::too_many_arguments)]
fn push_string_from_raw_slots(
    builder: &mut StringViewBuilder,
    string_buf: &mut Vec<u8>,
    raw_slots: &[[u8; 8]],
    start_slot: usize,
    width: usize,
    n_segments: usize,
    vls_layout: &[VlsSegmentInfo],
    file_encoding: &'static Encoding,
) {
    string_buf.clear();

    if n_segments <= 1 {
        let n_slots = width.div_ceil(8);
        for i in 0..n_slots {
            let idx = start_slot + i;
            if idx < raw_slots.len() {
                string_buf.extend_from_slice(&raw_slots[idx]);
            }
        }
    } else {
        // Very long string: copy slots per segment, truncating each segment
        // to its useful byte count to strip slot-alignment padding.
        let mut slot = start_slot;
        let mut cumulative = 0;
        for seg_info in vls_layout {
            cumulative += seg_info.useful_bytes;
            let slots_to_read = seg_info.useful_bytes.div_ceil(8);
            for i in 0..slots_to_read {
                if slot + i < raw_slots.len() {
                    string_buf.extend_from_slice(&raw_slots[slot + i]);
                }
            }
            // Strip padding bytes at end of this segment (e.g. byte 255 of
            // a 255-byte primary segment stored in 32 slots = 256 bytes).
            string_buf.truncate(cumulative);
            slot += 32;
        }
    }

    string_buf.truncate(width);
    let trimmed = io_utils::trim_trailing_padding(string_buf);
    let decoded = encoding::decode_str_lossy(trimmed, file_encoding);
    builder.append_value(&*decoded);
}
