use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Only fetch new version if not in docs.rs environment
    if env::var("DOCS_RS").is_ok() {
        // For docs.rs, use a fallback version
        write_version_file("2.9.0");
        return;
    }

    // Check if we have a cached version that's still fresh
    // To clear the cache and fetch the latest version, run:
    //   cargo clean
    // Or manually delete:
    //   rm target/nearcore_version_cache.txt
    let cache_path = get_cache_path();
    println!("cargo:rerun-if-changed={}", cache_path.display());
    let version = if let Some(cached_version) = read_cached_version(&cache_path) {
        println!(
            "cargo:warning=Using cached nearcore version: {}",
            cached_version
        );
        println!("cargo:warning=To fetch latest: rm {}", cache_path.display());
        cached_version
    } else {
        // Try to fetch the latest version from GitHub
        let version = fetch_latest_version().unwrap_or_else(|e| {
            panic!(
                "Failed to fetch latest nearcore version: {}\n\
                \n\
                This build requires fetching the latest nearcore version from GitHub.\n\
                Possible solutions:\n\
                1. Check your internet connection\n\
                2. If GitHub API is rate-limited, wait and try again",
                e
            );
        });

        // Cache the version for future builds
        if let Err(e) = write_cache_file(&cache_path, &version) {
            eprintln!("Warning: Failed to cache version: {}", e);
        } else {
            println!(
                "cargo:warning=Cached nearcore version {} to {}",
                version,
                cache_path.display()
            );
        }

        version
    };

    write_version_file(&version);
}

fn get_cache_path() -> PathBuf {
    // Store cache in target directory which persists between builds
    let out_dir = env::var("OUT_DIR").unwrap();
    let mut cache_path = PathBuf::from(out_dir);
    // Go up from target/debug/build/<package>/out to target/
    cache_path.pop(); // out
    cache_path.pop(); // <package>
    cache_path.pop(); // build
    cache_path.pop(); // debug or release
    cache_path.push("nearcore_version_cache.txt");
    cache_path
}

fn read_cached_version(cache_path: &Path) -> Option<String> {
    // Check if cache file exists and is less than 24 hours old
    if cache_path.exists() {
        if let Ok(metadata) = fs::metadata(cache_path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                    // Cache is valid for 14 days
                    if elapsed < Duration::from_secs(14 * 24 * 60 * 60) {
                        if let Ok(content) = fs::read_to_string(cache_path) {
                            let version = content.trim().to_string();
                            if !version.is_empty() {
                                return Some(version);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn write_cache_file(cache_path: &Path, version: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(cache_path, version)?;
    Ok(())
}

fn fetch_latest_version() -> Result<String, Box<dyn std::error::Error>> {
    // Use blocking reqwest client for build script
    let client = reqwest::blocking::Client::builder()
        .user_agent("near-sandbox-rs-build")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let response = client
        .get("https://api.github.com/repos/near/nearcore/releases/latest")
        .send()?;

    if !response.status().is_success() {
        return Err(format!("GitHub API returned status: {}", response.status()).into());
    }

    let release_data: serde_json::Value = response.json()?;

    let tag_name = release_data
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or("tag_name not found in release data")?;

    // Remove the 'v' prefix if present (e.g., "v2.9.0" -> "2.9.0")
    let version = tag_name.strip_prefix('v').unwrap_or(tag_name).to_string();

    println!("cargo:warning=Fetched latest nearcore version: {}", version);

    Ok(version)
}

fn write_version_file(version: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("nearcore_version.rs");

    let content = format!(
        r#"/// The latest nearcore sandbox version, fetched at build time.
/// This version is automatically updated when the crate is built.
pub const LATEST_SANDBOX_VERSION: &str = "{}";"#,
        version
    );

    fs::write(dest_path, content).expect("Failed to write version file");
}
