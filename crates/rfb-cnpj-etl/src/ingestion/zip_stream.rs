//! Zero-copy in-memory streaming decompression for RFB CNPJ open data zip archives.
//!
//! Feeds decompressed bytes directly to `csv::ReaderBuilder` without intermediate raw CSV files on disk.

use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;
use std::time::Instant;

use thiserror::Error;

use crate::ingestion::transforms::{parse_rfb_reference_code, Windows1252Reader};

/// Errors encountered during streaming zip decompression and CSV parsing.
#[derive(Error, Debug)]
pub enum ZipStreamError {
    #[error("I/O error while reading zip file '{0}': {1}")]
    Io(String, #[source] std::io::Error),

    #[error("ZIP decompression error in '{0}': {1}")]
    Zip(String, #[source] zip::result::ZipError),

    #[error("CSV parse error in '{0}' at record {1}: {2}")]
    Csv(String, usize, #[source] csv::Error),

    #[error("Target data entry not found in zip archive '{0}' (expected suffix: {1:?})")]
    EntryNotFound(String, Option<String>),

    #[error("Channel send error: {0}")]
    ChannelSend(String),

    #[error("Transform error: {0}")]
    Transform(String),
}

/// Statistics collected during streaming decompression and processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipStreamStats {
    pub file_path: String,
    pub entry_name: String,
    pub rows_processed: usize,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub rfb_ref_date: Option<String>,
    pub rfb_ano_mes: Option<String>,
    pub elapsed_millis: u128,
}

/// Metadata about the inner data file contained within an RFB zip archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntryMetadata {
    pub entry_name: String,
    pub entry_index: usize,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub rfb_ref_date: Option<String>,
    pub rfb_ano_mes: Option<String>,
}

/// Buffer size (256 KiB) for streaming decompression buffers to ensure high I/O throughput.
pub const STREAM_BUFFER_SIZE: usize = 256 * 1024;

/// Locates the primary data entry inside an RFB zip archive.
///
/// Filters out directories, macOS metadata (`__MACOSX`, `.DS_Store`), and matches the optional suffix.
pub fn find_data_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    expected_suffix: Option<&str>,
) -> Result<ZipEntryMetadata, ZipStreamError> {
    let mut candidate: Option<ZipEntryMetadata> = None;
    let mut fallback: Option<ZipEntryMetadata> = None;

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| ZipStreamError::Zip("archive".into(), e))?;
        let name = entry.name().to_string();

        // Skip directories and metadata junk
        if name.ends_with('/') || name.starts_with("__MACOSX") || name.starts_with('.') {
            continue;
        }

        let (rfb_ref_date, rfb_ano_mes) = parse_rfb_reference_code(&name)
            .map(|(d, m)| (Some(d), Some(m)))
            .unwrap_or((None, None));

        let meta = ZipEntryMetadata {
            entry_name: name.clone(),
            entry_index: i,
            compressed_size: entry.compressed_size(),
            uncompressed_size: entry.size(),
            rfb_ref_date,
            rfb_ano_mes,
        };

        if fallback.is_none() {
            fallback = Some(meta.clone());
        }

        let matches_suffix = expected_suffix
            .map(|suffix| name.to_uppercase().ends_with(&suffix.to_uppercase()))
            .unwrap_or(true);

        if matches_suffix {
            candidate = Some(meta);
            break;
        }
    }

    candidate.or(fallback).ok_or_else(|| {
        ZipStreamError::EntryNotFound("archive".into(), expected_suffix.map(|s| s.to_string()))
    })
}

/// Inspects an RFB zip archive on disk and returns metadata without decompressing the data.
pub fn inspect_zip<P: AsRef<Path>>(
    zip_path: P,
    expected_suffix: Option<&str>,
) -> Result<ZipEntryMetadata, ZipStreamError> {
    let path_str = zip_path.as_ref().display().to_string();
    let file =
        File::open(zip_path.as_ref()).map_err(|e| ZipStreamError::Io(path_str.clone(), e))?;
    let buf_reader = BufReader::with_capacity(STREAM_BUFFER_SIZE, file);
    let mut archive =
        zip::ZipArchive::new(buf_reader).map_err(|e| ZipStreamError::Zip(path_str.clone(), e))?;

    find_data_entry(&mut archive, expected_suffix)
}

/// Zero-copy streaming processor reading raw CSV records as `csv::ByteRecord`.
///
/// Decompresses in-memory and feeds records directly into the provided handler callback.
pub fn process_zip_byte_records<P, F>(
    zip_path: P,
    expected_suffix: Option<&str>,
    mut handler: F,
) -> Result<ZipStreamStats, ZipStreamError>
where
    P: AsRef<Path>,
    F: FnMut(&csv::ByteRecord) -> Result<(), ZipStreamError>,
{
    let start_time = Instant::now();
    let path_str = zip_path.as_ref().display().to_string();

    let file =
        File::open(zip_path.as_ref()).map_err(|e| ZipStreamError::Io(path_str.clone(), e))?;
    let buf_reader = BufReader::with_capacity(STREAM_BUFFER_SIZE, file);
    let mut archive =
        zip::ZipArchive::new(buf_reader).map_err(|e| ZipStreamError::Zip(path_str.clone(), e))?;

    let meta = find_data_entry(&mut archive, expected_suffix)?;
    let entry_idx = meta.entry_index;

    let zip_entry = archive
        .by_index(entry_idx)
        .map_err(|e| ZipStreamError::Zip(path_str.clone(), e))?;

    let stream_reader = BufReader::with_capacity(STREAM_BUFFER_SIZE, zip_entry);

    let mut csv_reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(false)
        .quote(b'"')
        .double_quote(true)
        .escape(None)
        .flexible(true)
        .from_reader(stream_reader);

    let mut record = csv::ByteRecord::new();
    let mut rows_processed = 0usize;

    while csv_reader
        .read_byte_record(&mut record)
        .map_err(|e| ZipStreamError::Csv(path_str.clone(), rows_processed, e))?
    {
        rows_processed += 1;
        handler(&record)?;
    }

    Ok(ZipStreamStats {
        file_path: path_str,
        entry_name: meta.entry_name,
        rows_processed,
        compressed_size: meta.compressed_size,
        uncompressed_size: meta.uncompressed_size,
        rfb_ref_date: meta.rfb_ref_date,
        rfb_ano_mes: meta.rfb_ano_mes,
        elapsed_millis: start_time.elapsed().as_millis(),
    })
}

/// Transcoding streaming processor reading UTF-8 records as `csv::StringRecord`.
///
/// Wraps the decompressed stream with `Windows1252Reader` before passing to `csv::ReaderBuilder`.
pub fn process_zip_string_records<P, F>(
    zip_path: P,
    expected_suffix: Option<&str>,
    mut handler: F,
) -> Result<ZipStreamStats, ZipStreamError>
where
    P: AsRef<Path>,
    F: FnMut(&csv::StringRecord) -> Result<(), ZipStreamError>,
{
    let start_time = Instant::now();
    let path_str = zip_path.as_ref().display().to_string();

    let file =
        File::open(zip_path.as_ref()).map_err(|e| ZipStreamError::Io(path_str.clone(), e))?;
    let buf_reader = BufReader::with_capacity(STREAM_BUFFER_SIZE, file);
    let mut archive =
        zip::ZipArchive::new(buf_reader).map_err(|e| ZipStreamError::Zip(path_str.clone(), e))?;

    let meta = find_data_entry(&mut archive, expected_suffix)?;
    let entry_idx = meta.entry_index;

    let zip_entry = archive
        .by_index(entry_idx)
        .map_err(|e| ZipStreamError::Zip(path_str.clone(), e))?;

    let transcoding_reader =
        Windows1252Reader::new(BufReader::with_capacity(STREAM_BUFFER_SIZE, zip_entry));

    let mut csv_reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(false)
        .quote(b'"')
        .double_quote(true)
        .escape(None)
        .flexible(true)
        .from_reader(transcoding_reader);

    let mut record = csv::StringRecord::new();
    let mut rows_processed = 0usize;

    while csv_reader
        .read_record(&mut record)
        .map_err(|e| ZipStreamError::Csv(path_str.clone(), rows_processed, e))?
    {
        rows_processed += 1;
        handler(&record)?;
    }

    Ok(ZipStreamStats {
        file_path: path_str,
        entry_name: meta.entry_name,
        rows_processed,
        compressed_size: meta.compressed_size,
        uncompressed_size: meta.uncompressed_size,
        rfb_ref_date: meta.rfb_ref_date,
        rfb_ano_mes: meta.rfb_ano_mes,
        elapsed_millis: start_time.elapsed().as_millis(),
    })
}

/// Producer function that reads a zip file, transforms rows into `T`, and streams batches into a channel.
///
/// Designed for parallel ingestion pipelines where decompression/parsing occurs on worker threads
/// and batch database insertion occurs on the writer thread.
pub fn stream_zip_batches<P, T, TransformFn>(
    zip_path: P,
    expected_suffix: Option<&str>,
    batch_size: usize,
    sender: &crossbeam_channel::Sender<anyhow::Result<Vec<T>>>,
    transform: TransformFn,
) -> Result<ZipStreamStats, ZipStreamError>
where
    P: AsRef<Path>,
    T: Send + 'static,
    TransformFn: Fn(&csv::ByteRecord) -> Result<T, ZipStreamError>,
{
    let mut current_batch = Vec::with_capacity(batch_size);

    let stats = process_zip_byte_records(zip_path, expected_suffix, |record| {
        let item = transform(record)?;
        current_batch.push(item);

        if current_batch.len() >= batch_size {
            let full_batch = std::mem::replace(&mut current_batch, Vec::with_capacity(batch_size));
            sender
                .send(Ok(full_batch))
                .map_err(|e| ZipStreamError::ChannelSend(e.to_string()))?;
        }
        Ok(())
    })?;

    // Flush final partial batch
    if !current_batch.is_empty() {
        sender
            .send(Ok(current_batch))
            .map_err(|e| ZipStreamError::ChannelSend(e.to_string()))?;
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::{SimpleFileOptions, ZipWriter};

    fn create_test_zip(zip_path: &Path, entry_name: &str, content: &[u8]) {
        let file = File::create(zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(entry_name, options).unwrap();
        zip.write_all(content).unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn test_process_zip_byte_records_streaming() {
        let temp = tempfile::tempdir().unwrap();
        let zip_file = temp.path().join("test.zip");
        let csv_data = b"11111111;ALFA;2062\r\n22222222;BETA;2054\r\n";
        create_test_zip(&zip_file, "data.csv", csv_data);

        let mut rows = Vec::new();
        let stats = process_zip_byte_records(&zip_file, None, |rec| {
            rows.push((rec[0].to_vec(), rec[1].to_vec()));
            Ok(())
        })
        .unwrap();

        assert_eq!(stats.rows_processed, 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, b"11111111");
        assert_eq!(rows[1].1, b"BETA");
    }

    #[test]
    fn test_stream_zip_batches_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let zip_file = temp.path().join("batches.zip");
        let mut csv_data = String::new();
        for i in 0..105 {
            csv_data.push_str(&format!("{i:08};NAME_{i}\r\n"));
        }
        create_test_zip(&zip_file, "data.csv", csv_data.as_bytes());

        let (tx, rx) = crossbeam_channel::bounded(4);
        let zip_path_clone = zip_file.clone();

        let producer = std::thread::spawn(move || {
            stream_zip_batches(&zip_path_clone, None, 50, &tx, |rec| Ok(rec[0].to_vec())).unwrap()
        });

        let mut total_received = 0;
        let mut batch_count = 0;
        while let Ok(batch_res) = rx.recv() {
            let batch = batch_res.unwrap();
            total_received += batch.len();
            batch_count += 1;
        }

        let stats = producer.join().unwrap();
        assert_eq!(stats.rows_processed, 105);
        assert_eq!(total_received, 105);
        assert_eq!(batch_count, 3); // 50, 50, 5
    }
}
