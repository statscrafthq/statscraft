use std::io::{Read, Seek, SeekFrom};

use flate2::Decompress;

use crate::error::{Result, SavError};
use crate::io_utils::SavReader;

/// ZSAV zlib header: offsets to the trailer.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ZHeader {
    pub zheader_offset: i64,
    pub ztrailer_offset: i64,
    pub ztrailer_length: i64,
}

/// ZSAV trailer: compression parameters.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ZTrailer {
    pub bias: i64,
    pub zero: i64,
    pub block_size: i32,
    pub n_blocks: i32,
    pub entries: Vec<ZTrailerEntry>,
}

/// A single block entry in the ZSAV trailer.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ZTrailerEntry {
    pub uncompressed_offset: i64,
    pub compressed_offset: i64,
    pub uncompressed_size: i32,
    pub compressed_size: i32,
}

/// Read the ZSAV zlib header (24 bytes, immediately after the dictionary termination).
pub fn read_zheader<R: Read>(reader: &mut SavReader<R>) -> Result<ZHeader> {
    Ok(ZHeader {
        zheader_offset: reader.read_i64()?,
        ztrailer_offset: reader.read_i64()?,
        ztrailer_length: reader.read_i64()?,
    })
}

/// Read the ZSAV trailer from the file.
///
/// Requires a seekable reader to jump to the trailer offset.
pub fn read_ztrailer<R: Read + Seek>(
    reader: &mut SavReader<R>,
    zheader: &ZHeader,
) -> Result<ZTrailer> {
    reader
        .inner_mut()
        .seek(SeekFrom::Start(zheader.ztrailer_offset as u64))?;

    let bias = reader.read_i64()?;
    let zero = reader.read_i64()?;
    let block_size = reader.read_i32()?;
    let n_blocks = reader.read_i32()?;

    let mut entries = Vec::with_capacity((n_blocks.max(0) as usize).min(4096));
    for _ in 0..n_blocks {
        entries.push(ZTrailerEntry {
            uncompressed_offset: reader.read_i64()?,
            compressed_offset: reader.read_i64()?,
            uncompressed_size: reader.read_i32()?,
            compressed_size: reader.read_i32()?,
        });
    }

    Ok(ZTrailer {
        bias,
        zero,
        block_size,
        n_blocks,
        entries,
    })
}

/// Decompress a single ZSAV block from disk on demand.
///
/// Seeks to the block's absolute file offset, reads compressed data,
/// and decompresses it. Used by the streaming scanner to avoid loading
/// all blocks into memory at once.
pub fn decompress_single_block<R: Read + Seek>(
    reader: &mut SavReader<R>,
    entry: &ZTrailerEntry,
) -> Result<Vec<u8>> {
    let sizes = usize::try_from(entry.compressed_size)
        .ok()
        .zip(usize::try_from(entry.uncompressed_size).ok());
    // zlib cannot expand data more than about 1032 times, so a larger declared
    // size is corrupt and must not decide an allocation
    let Some((compressed_size, uncompressed_size)) =
        sizes.filter(|&(c, u)| u <= c.saturating_mul(1100).saturating_add(64))
    else {
        return Err(SavError::Zlib(format!(
            "block declares {} compressed and {} uncompressed bytes",
            entry.compressed_size, entry.uncompressed_size
        )));
    };
    let offset = u64::try_from(entry.compressed_offset)
        .map_err(|_| SavError::Zlib(format!("block offset {}", entry.compressed_offset)))?;
    reader.inner_mut().seek(SeekFrom::Start(offset))?;
    let compressed = reader.read_bytes(compressed_size)?;
    let mut output = vec![0u8; uncompressed_size];

    let mut decompressor = Decompress::new(true);
    match decompressor.decompress(&compressed, &mut output, flate2::FlushDecompress::Finish) {
        Ok(flate2::Status::Ok | flate2::Status::StreamEnd) => {
            // a block that inflates to less than declared yields only what it holds
            output.truncate(decompressor.total_out() as usize);
            Ok(output)
        }
        Ok(flate2::Status::BufError) => Err(SavError::Zlib(
            "single block decompression buffer too small".to_string(),
        )),
        Err(e) => Err(SavError::Zlib(format!(
            "single block decompression failed: {e}"
        ))),
    }
}

