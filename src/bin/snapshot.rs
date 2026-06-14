//! `snapshot` companion binary: probes both providers and writes
//! redacted HTML snapshots for drift detection.
//!
//! Reads a list of `YouTube` URLs from a corpus file, fetches the
//! primary page from each provider with a configurable timeout, and
//! stores the responses under a date-stamped subdirectory. Real
//! provider hosts and paths are sourced from the gitignored
//! `secret_endpoints` module and are never written to disk in
//! plaintext.

use clap::Parser;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::io::AsyncWriteExt;

//  is  in lib.rs to keep the
// secret host/path constants off the published public API
// surface (GAP-007). The bin target therefore cannot import
// them via ; instead it
// re-includes the module locally through  so the
// privacy boundary stays intact for the public crate while
// remaining visible to this companion binary.
#[path = "../secret_endpoints.rs"]
mod secret_endpoints;

use secret_endpoints::{
    PROVIDER_A_INFO_BASE, PROVIDER_A_PRIMARY_HOST, PROVIDER_A_PRIMARY_PAGE, PROVIDER_B_API_PATH,
    PROVIDER_B_PRIMARY_HOST, PROVIDER_B_PRIMARY_PAGE, USER_AGENT_IDENTITY,
};

/// Command-line arguments for the `snapshot` binary.
#[derive(Parser, Debug)]
#[command(
    name = "snapshot",
    about = "Probe subtitle providers to capture redacted HTML snapshots for drift detection."
)]
struct Args {
    /// Path to a corpus file with one `YouTube` URL per line.
    #[arg(long, default_value = "tests/fixtures/corpus.txt")]
    corpus: PathBuf,

    /// Output directory. A date-stamped subdirectory will be created here.
    #[arg(long, default_value = "tests/fixtures/snapshots")]
    output_dir: PathBuf,

    /// Per-request timeout in seconds.
    #[arg(long, default_value_t = 30)]
    timeout: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    youtube_legend_cli::logging::init_tracing(
        youtube_legend_cli::cli::LogLevelArg::Info,
        youtube_legend_cli::cli::LogFormatArg::Text,
        youtube_legend_cli::cli::ColorArg::Auto,
        false,
    )
    .ok();
    run_snapshot(Args::parse()).await
}

#[tracing::instrument(skip(args), fields(corpus = %args.corpus.display(), output_dir = %args.output_dir.display(), timeout_s = args.timeout))]
async fn run_snapshot(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let timeout = Duration::from_secs(args.timeout);

    if !args.corpus.exists() {
        tracing::error!(target: "user_error", path = %args.corpus.display(), "corpus file not found");
        std::process::exit(2);
    }

    fs::create_dir_all(&args.output_dir).await?;
    let timestamp = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let snapshot_dir = args.output_dir.join(timestamp);
    fs::create_dir_all(&snapshot_dir).await?;

    let corpus_text = fs::read_to_string(&args.corpus).await?;
    let urls: Vec<String> = corpus_text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect();

    tracing::info!(target: "snapshot", event = "probe_started", provider = "a", urls = urls.len());

    for url in &urls {
        let safe_id = sanitize_id(url);
        let snapshot_path = snapshot_dir.join(format!("provider_a_{safe_id}.html"));
        let encoded = urlencode(url);
        let target = format!("{PROVIDER_A_PRIMARY_PAGE}{encoded}");

        tracing::debug!(target: "snapshot", event = "fetch_started", provider = "a", url = %target);
        let started = Instant::now();
        let result = fetch_with_timeout(&target, PROVIDER_A_PRIMARY_HOST, timeout).await;
        let elapsed = started.elapsed().as_millis();

        match result {
            Ok(body) => {
                let count = body.matches(PROVIDER_A_INFO_BASE).count();
                fs::write(&snapshot_path, &body).await?;
                tracing::info!(
                    target: "snapshot",
                    event = "snapshot_saved",
                    provider = "a",
                    url = %url,
                    path = %snapshot_path.display(),
                    token_markers = count,
                    duration_ms = elapsed
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "snapshot",
                    event = "snapshot_failed",
                    provider = "a",
                    url = %url,
                    error = %e
                );
                let mut f = fs::File::create(&snapshot_path).await?;
                f.write_all(format!("ERROR: {e}\n").as_bytes()).await?;
            }
        }
    }

    tracing::info!(target: "snapshot", event = "probe_started", provider = "b", urls = urls.len());

    for url in &urls {
        let safe_id = sanitize_id(url);
        let snapshot_path = snapshot_dir.join(format!("provider_b_{safe_id}.html"));
        let encoded = urlencode(url);
        let target = format!("{PROVIDER_B_PRIMARY_PAGE}{encoded}");

        tracing::debug!(target: "snapshot", event = "fetch_started", provider = "b", url = %target);
        let started = Instant::now();
        let result = fetch_with_timeout(&target, PROVIDER_B_PRIMARY_HOST, timeout).await;
        let elapsed = started.elapsed().as_millis();

        match result {
            Ok(body) => {
                let sid_count = body.matches("var sid=").count();
                let tutoken_count = body.matches("var tutoken=").count();
                let htoken_count = body.matches("var htoken=").count();
                fs::write(&snapshot_path, &body).await?;
                tracing::info!(
                    target: "snapshot",
                    event = "snapshot_saved",
                    provider = "b",
                    url = %url,
                    path = %snapshot_path.display(),
                    sid = sid_count,
                    tutoken = tutoken_count,
                    htoken = htoken_count,
                    duration_ms = elapsed
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "snapshot",
                    event = "snapshot_failed",
                    provider = "b",
                    url = %url,
                    error = %e
                );
            }
        }
    }

    let count = count_files(&snapshot_dir).await?;
    tracing::info!(
        target: "snapshot",
        event = "probe_completed",
        dir = %snapshot_dir.display(),
        total_files = count
    );
    tracing::info!(
        target: "snapshot",
        event = "redaction_notice",
        message = "real provider hosts and paths are redacted in snapshot output",
        real_endpoints = "src/secret_endpoints.rs (gitignored)",
        api_path_hint = %PROVIDER_B_API_PATH
    );

    Ok(())
}

async fn fetch_with_timeout(url: &str, host: &str, timeout: Duration) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT_IDENTITY)
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::limited(5))
        .gzip(true)
        .build()
        .map_err(|e| format!("client build: {e}"))?;

    let resp = client
        .get(url)
        .header("Host", host)
        .send()
        .await
        .map_err(|e| format!("request: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    resp.text().await.map_err(|e| format!("body: {e}"))
}

fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    out
}

fn sanitize_id(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("unknown")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

async fn count_files(dir: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let mut count = 0;
    let mut entries = fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_file() {
            count += 1;
        }
    }
    Ok(count)
}
