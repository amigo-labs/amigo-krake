//! Usenet/NNTP download backend.
//!
//! Pipeline: NZB parse → NNTP download articles → yEnc decode → reassemble → PAR2 verify/repair

pub mod nntp;
pub mod nzb;
pub mod yenc;

use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, warn};

use self::nntp::{NntpConnectionPool, NntpServerConfig};
use self::nzb::{NzbFile, NzbSegment};
use self::yenc::decode_yenc;

/// Usenet download configuration.
#[derive(Debug, Clone)]
pub struct UsenetConfig {
    pub servers: Vec<NntpServerConfig>,
    pub par2_repair: bool,
    pub auto_unrar: bool,
    pub delete_archives_after_extract: bool,
}

/// The lowest health (per mille of articles fetched) from which the NZB's
/// PAR2 recovery volumes can still repair it: 1000 minus the recovery
/// volumes' share, in per mille, of the other files' size. Without recovery
/// volumes every missing article is fatal, so it is 1000.
pub fn critical_health(nzb: &nzb::Nzb) -> u32 {
    let mut par2 = 0u64;
    let mut recovery = 0u64;
    let mut total = 0u64;
    for file in &nzb.files {
        let name = file.filename().to_ascii_lowercase();
        let bytes = file.total_bytes();
        total += bytes;
        if name.ends_with(".par2") {
            par2 += bytes;
            if name.contains(".vol") {
                recovery += bytes;
            }
        }
    }
    let data = total - par2;
    if data == 0 {
        return 1000;
    }
    let share = (u128::from(recovery) * 1000 / u128::from(data)).min(1000) as u32;
    1000 - share
}

/// Result of downloading a single NZB file entry.
#[derive(Debug)]
pub struct FileDownloadResult {
    pub filename: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub segments_ok: u32,
    pub segments_failed: u32,
}

pub struct UsenetDownloader {
    pools: Vec<NntpConnectionPool>,
}

impl UsenetDownloader {
    pub fn new(config: UsenetConfig) -> Self {
        let mut pools: Vec<NntpConnectionPool> = config
            .servers
            .iter()
            .map(|s| NntpConnectionPool::new(s.clone()))
            .collect();

        // Sort by priority (lower = higher priority)
        pools.sort_by_key(|p| p.priority());

        Self { pools }
    }

    /// Download all files from an NZB.
    pub async fn download_nzb(
        &self,
        nzb_data: &str,
        output_dir: &Path,
    ) -> Result<Vec<FileDownloadResult>, crate::Error> {
        let nzb = nzb::parse_nzb(nzb_data)?;
        info!("NZB contains {} files", nzb.files.len());

        tokio::fs::create_dir_all(output_dir).await?;

        let mut results = Vec::new();

        // A file that cannot be fetched for any reason other than missing or
        // damaged articles (server down, disk full) fails the whole job, so
        // it is retried instead of being counted against the release.
        for file in &nzb.files {
            let filename = file.filename();
            info!("Downloading: {filename} ({} segments)", file.segments.len());

            let result = self
                .download_file(file, output_dir)
                .await
                .inspect_err(|e| {
                    error!("Failed to download {filename}: {e}");
                })?;
            info!(
                "Completed: {filename} ({} bytes, {}/{} segments)",
                result.bytes,
                result.segments_ok,
                result.segments_ok + result.segments_failed
            );
            results.push(result);
        }

        Ok(results)
    }

    /// Download a single file from its NZB segments.
    async fn download_file(
        &self,
        file: &NzbFile,
        output_dir: &Path,
    ) -> Result<FileDownloadResult, crate::Error> {
        let filename = file.filename();
        let output_path = output_dir.join(&filename);
        let group = file.groups.first().cloned().unwrap_or_default();

        let mut all_data: Vec<(u32, Vec<u8>)> = Vec::new();
        let mut segments_ok: u32 = 0;
        let mut segments_failed: u32 = 0;

        for segment in &file.segments {
            match self.download_segment(&group, segment).await {
                Ok(data) => {
                    all_data.push((segment.number, data));
                    segments_ok += 1;
                }
                Err(e @ crate::Error::ArticleUnavailable(_)) => {
                    warn!("Segment {} of {filename} failed: {e}", segment.number);
                    segments_failed += 1;
                }
                Err(e) => return Err(e),
            }
        }

        // Sort segments by number and write to file
        all_data.sort_by_key(|(num, _)| *num);

        let mut out_file = tokio::fs::File::create(&output_path).await?;
        let mut total_bytes: u64 = 0;

        for (_, data) in &all_data {
            out_file.write_all(data).await?;
            total_bytes += data.len() as u64;
        }
        out_file.flush().await?;

        Ok(FileDownloadResult {
            filename,
            path: output_path,
            bytes: total_bytes,
            segments_ok,
            segments_failed,
        })
    }

    /// Download and decode a single segment, trying servers in priority order.
    async fn download_segment(
        &self,
        group: &str,
        segment: &NzbSegment,
    ) -> Result<Vec<u8>, crate::Error> {
        // The article counts as unavailable only when every server said so;
        // any other error (connection, auth, timeout) wins, as it says
        // nothing about the release.
        let mut unavailable = None;
        let mut other = None;

        for pool in &self.pools {
            match self.try_segment_from_pool(pool, group, segment).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    debug!(
                        "Segment {} failed on {}: {e}",
                        segment.message_id,
                        pool.server_name()
                    );
                    match e {
                        crate::Error::ArticleUnavailable(_) => unavailable = Some(e),
                        e => other = Some(e),
                    }
                }
            }
        }

        Err(other
            .or(unavailable)
            .unwrap_or_else(|| crate::Error::Other("No NNTP servers configured".into())))
    }

    async fn try_segment_from_pool(
        &self,
        pool: &NntpConnectionPool,
        group: &str,
        segment: &NzbSegment,
    ) -> Result<Vec<u8>, crate::Error> {
        let mut conn = pool.acquire().await?;

        // Select group
        let resp = conn.group(group).await?;
        if resp.code != 211 {
            pool.release(conn).await;
            return Err(crate::Error::Other(format!(
                "GROUP failed: {} {}",
                resp.code, resp.message
            )));
        }

        // Download article body. On error the connection may hold buffered or
        // partial response data, so it must NOT be returned to the pool —
        // dropping it closes the socket and avoids desyncing the next command
        // that would otherwise read the stale tail of this response.
        let body = match conn.body(&segment.message_id).await {
            Ok(body) => {
                pool.release(conn).await;
                body
            }
            Err(e) => {
                drop(conn);
                return Err(e);
            }
        };

        // Decode yEnc. An article that does not decode, or fails its CRC,
        // is damaged on the server.
        let decoded = decode_yenc(&body).map_err(|e| {
            crate::Error::ArticleUnavailable(format!("{}: {e}", segment.message_id))
        })?;

        // Verify CRC if available
        if let Some(expected_crc) = decoded.part_crc32.or(decoded.crc32)
            && !yenc::verify_crc32(&decoded.data, expected_crc)
        {
            return Err(crate::Error::ArticleUnavailable(format!(
                "CRC32 mismatch for segment {}",
                segment.message_id
            )));
        }

        Ok(decoded.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nzb_of(files: &[(&str, u64)]) -> nzb::Nzb {
        let mut xml = String::from(
            r#"<?xml version="1.0"?><nzb xmlns="http://www.newzbin.com/DTD/2003/nzb">"#,
        );
        for (i, (name, bytes)) in files.iter().enumerate() {
            xml.push_str(&format!(
                r#"<file poster="p" date="1" subject="&quot;{name}&quot; yEnc (1/1)"><groups><group>a.b</group></groups><segments><segment bytes="{bytes}" number="1">m{i}@x</segment></segments></file>"#
            ));
        }
        xml.push_str("</nzb>");
        nzb::parse_nzb(&xml).unwrap()
    }

    #[test]
    fn critical_health_is_what_the_recovery_volumes_can_make_up() {
        assert_eq!(critical_health(&nzb_of(&[("movie.mkv", 1000)])), 1000);
        assert_eq!(
            critical_health(&nzb_of(&[
                ("movie.part1.rar", 900),
                ("movie.part2.rar", 100),
                ("movie.par2", 1),
                ("movie.vol00+10.par2", 100),
            ])),
            900
        );
        assert_eq!(critical_health(&nzb_of(&[("only.par2", 10)])), 1000);
    }

    #[test]
    fn test_usenet_config() {
        let config = UsenetConfig {
            servers: vec![NntpServerConfig {
                name: "primary".into(),
                host: "news.example.com".into(),
                port: 563,
                ssl: true,
                username: "user".into(),
                password: "pass".into(),
                connections: 20,
                priority: 0,
            }],
            par2_repair: true,
            auto_unrar: true,
            delete_archives_after_extract: true,
        };

        let downloader = UsenetDownloader::new(config);
        assert_eq!(downloader.pools.len(), 1);
    }
}
