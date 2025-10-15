use anyhow::{Context, Result, bail};
use clap::Args;
use globwalk::{FileType, GlobWalkerBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{self, Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

/// Arguments for the `cigen hash` subcommand.
#[derive(Debug, Args)]
pub struct HashArgs {
    /// Glob patterns to include when computing the hash (relative to --base-dir)
    #[arg(short = 'p', long = "pattern")]
    pub patterns: Vec<String>,

    /// Optional job identifier to hash using the loaded cigen config
    #[arg(long = "job")]
    pub job: Option<String>,

    /// Path to the cigen config directory or file (defaults to .cigen)
    #[arg(long = "config", default_value = ".cigen")]
    pub config: PathBuf,

    /// Optional name for the output value (written to $GITHUB_OUTPUT when set)
    #[arg(long = "output")]
    pub output_name: Option<String>,

    /// Base directory the patterns are evaluated relative to (defaults to current dir)
    #[arg(long = "base-dir", default_value = ".")]
    pub base_dir: PathBuf,

    /// Optional cache file path to persist per-file hashes
    #[arg(long = "cache")]
    pub cache_path: Option<PathBuf>,
}

pub fn hash_command(args: HashArgs) -> Result<()> {
    if let Some(job_id) = args.job.as_deref() {
        hash_job(&args, job_id)
    } else {
        if args.patterns.is_empty() {
            bail!(
                "No patterns provided. Use --pattern for file hashing or --job to hash a config job."
            );
        }
        hash_patterns(&args)
    }
}

fn hash_patterns(args: &HashArgs) -> Result<()> {
    let base_dir = canonicalize_path(&args.base_dir)?;

    let mut files = collect_files(&base_dir, &args.patterns)?;
    files.sort();

    let cache_path = args
        .cache_path
        .as_ref()
        .map(|path| resolve_path(&base_dir, path));
    let mut persistent_cache = if let Some(path) = cache_path.as_ref() {
        Some(HashCache::load(path)?)
    } else {
        None
    };

    let mut file_hasher = FileHasher::new(persistent_cache.as_mut());
    let mut aggregate = Sha256::new();

    for rel in &files {
        let absolute = base_dir.join(rel);
        let file_hash = file_hasher.hash_file(&absolute, rel)?;
        aggregate.update(rel.to_string_lossy().as_bytes());
        aggregate.update([0u8]);
        aggregate.update(&file_hash);
    }

    let digest = if files.is_empty() {
        "empty".to_string()
    } else {
        hex::encode(aggregate.finalize())
    };

    if let Some(name) = &args.output_name {
        write_github_output(name, &digest)?;
    }

    println!("{digest}");

    if let Some(cache) = persistent_cache {
        cache.save()?;
    }

    Ok(())
}

fn hash_job(args: &HashArgs, job_id: &str) -> Result<()> {
    let base_dir = canonicalize_path(&args.base_dir)?;
    let config_path = resolve_path(&base_dir, &args.config);

    let (config, config_root) = load_config(&config_path)?;
    let job = config.jobs.get(job_id).with_context(|| {
        format!(
            "Job '{job_id}' not found in config at {}",
            config_path.display()
        )
    })?;

    let workflow_name = job.workflow.clone().unwrap_or_else(|| "ci".to_string());

    let cache_path = args
        .cache_path
        .as_ref()
        .map(|path| resolve_path(&base_dir, path));
    let mut persistent_cache = if let Some(path) = cache_path.as_ref() {
        Some(HashCache::load(path)?)
    } else {
        None
    };

    let mut file_hasher = FileHasher::new(persistent_cache.as_mut());
    let mut pattern_cache: HashMap<String, Vec<u8>> = HashMap::new();

    let mut entries: Vec<SourceEntry> = Vec::new();

    for entry in &job.source_files {
        if let Some(group) = entry.strip_prefix('@') {
            entries.push(SourceEntry::Group(group.to_string()));
        } else {
            entries.push(SourceEntry::Pattern(entry.to_string()));
        }
    }

    for literal in extra_config_patterns(&base_dir, &config_root, &workflow_name, job_id) {
        entries.push(SourceEntry::Pattern(literal));
    }

    let mut final_hasher = Sha256::new();
    final_hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    final_hasher.update(job_id.as_bytes());
    final_hasher.update([0u8]);
    final_hasher.update(workflow_name.as_bytes());

    let canonical_job = canonical_job_json(job)?;
    final_hasher.update([0u8]);
    final_hasher.update(canonical_job.as_bytes());

    let source_groups: BTreeMap<_, _> = config.source_file_groups.iter().collect();

    for entry in entries {
        match entry {
            SourceEntry::Pattern(pattern) => {
                let digest =
                    hash_pattern(&pattern, &base_dir, &mut file_hasher, &mut pattern_cache)?;
                final_hasher.update(b"pattern\0");
                final_hasher.update(pattern.as_bytes());
                final_hasher.update([0u8]);
                final_hasher.update(&digest);
            }
            SourceEntry::Group(name) => {
                let patterns = source_groups.get(&name).with_context(|| {
                    format!("Job '{job_id}' references unknown source file group '{name}'")
                })?;
                let digest = hash_group(
                    &name,
                    patterns,
                    &base_dir,
                    &mut file_hasher,
                    &mut pattern_cache,
                )?;
                final_hasher.update(b"group\0");
                final_hasher.update(name.as_bytes());
                final_hasher.update([0u8]);
                final_hasher.update(&digest);
            }
        }
    }

    let digest = hex::encode(final_hasher.finalize());

    if let Some(name) = &args.output_name {
        write_github_output(name, &digest)?;
    }

    println!("{digest}");

    if let Some(cache) = persistent_cache {
        cache.save()?;
    }

    Ok(())
}

fn hash_group(
    name: &str,
    patterns: &[String],
    base_dir: &Path,
    file_hasher: &mut FileHasher,
    pattern_cache: &mut HashMap<String, Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.update([0u8]);

    if patterns.is_empty() {
        hasher.update(b"empty-group");
        return Ok(hasher.finalize().to_vec());
    }

    let mut sorted: Vec<&String> = patterns.iter().collect();
    sorted.sort();

    for pattern in sorted {
        let digest = hash_pattern(pattern, base_dir, file_hasher, pattern_cache)?;
        hasher.update(pattern.as_bytes());
        hasher.update([0u8]);
        hasher.update(&digest);
    }

    Ok(hasher.finalize().to_vec())
}

fn hash_pattern(
    pattern: &str,
    base_dir: &Path,
    file_hasher: &mut FileHasher,
    pattern_cache: &mut HashMap<String, Vec<u8>>,
) -> Result<Vec<u8>> {
    if let Some(existing) = pattern_cache.get(pattern) {
        return Ok(existing.clone());
    }

    let mut files = list_files_with_git(base_dir, pattern)?;
    files.sort();
    files.dedup();

    let mut aggregate = Sha256::new();

    if files.is_empty() {
        aggregate.update(b"empty");
    } else {
        for rel in files {
            let absolute = base_dir.join(&rel);
            let digest = file_hasher.hash_file(&absolute, &rel)?;
            aggregate.update(rel.to_string_lossy().as_bytes());
            aggregate.update([0u8]);
            aggregate.update(&digest);
        }
    }

    let digest = aggregate.finalize().to_vec();
    pattern_cache.insert(pattern.to_string(), digest.clone());
    Ok(digest)
}

fn list_files_with_git(base_dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    match run_git_ls(base_dir, pattern) {
        Ok(mut files) => {
            if files.is_empty() && pattern_contains_glob(pattern) {
                let glob_pattern = format!(":(glob){}", pattern);
                files = run_git_ls(base_dir, &glob_pattern)?;
            }
            Ok(files)
        }
        Err(_) => {
            if pattern_contains_glob(pattern) {
                return glob_fallback(base_dir, pattern);
            }

            let candidate = base_dir.join(pattern);
            if candidate.exists() {
                let rel = candidate
                    .strip_prefix(base_dir)
                    .unwrap_or(&candidate)
                    .to_path_buf();
                return Ok(vec![rel]);
            }

            Ok(Vec::new())
        }
    }
}

fn run_git_ls(base_dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("ls-files")
        .arg("--")
        .arg(pattern)
        .current_dir(base_dir)
        .output()
        .with_context(|| format!("Failed to execute git ls-files for pattern '{pattern}'"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git ls-files exited with status {} while evaluating pattern '{pattern}': {stderr}",
            output.status
        );
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut files = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            files.push(PathBuf::from(trimmed));
        }
    }
    Ok(files)
}

fn pattern_contains_glob(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[') || pattern.contains(']')
}

fn glob_fallback(base_dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    let walker = GlobWalkerBuilder::from_patterns(base_dir, &[pattern])
        .follow_links(true)
        .case_insensitive(cfg!(windows))
        .file_type(FileType::FILE)
        .build()?;

    for entry in walker.into_iter().filter_map(Result::ok) {
        if let Ok(rel) = entry.path().strip_prefix(base_dir) {
            results.push(rel.to_path_buf());
        }
    }

    Ok(results)
}

fn extra_config_patterns(
    base_dir: &Path,
    config_root: &Path,
    workflow: &str,
    job_id: &str,
) -> Vec<String> {
    let mut results = BTreeSet::new();

    let candidates = [
        config_root
            .join("workflows")
            .join(format!("{workflow}.yml")),
        config_root
            .join("workflows")
            .join(format!("{workflow}.yaml")),
        config_root
            .join("workflows")
            .join(workflow)
            .join("jobs")
            .join(format!("{job_id}.yml")),
        config_root
            .join("workflows")
            .join(workflow)
            .join("jobs")
            .join(format!("{job_id}.yaml")),
    ];

    for candidate in candidates {
        if let Ok(rel) = candidate.strip_prefix(base_dir) {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            results.insert(rel_str);
        }
    }

    results.into_iter().collect()
}

fn canonical_job_json(job: &cigen::schema::Job) -> Result<String> {
    let value = serde_json::to_value(job)?;
    let canonical = canonicalize_json(value);
    serde_json::to_string(&canonical).context("Failed to serialize canonical job JSON")
}

fn canonicalize_json(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut entries: Vec<(String, JsonValue)> = map
                .into_iter()
                .map(|(k, v)| (k, canonicalize_json(v)))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut new_map = JsonMap::new();
            for (k, v) in entries {
                new_map.insert(k, v);
            }
            JsonValue::Object(new_map)
        }
        JsonValue::Array(items) => {
            JsonValue::Array(items.into_iter().map(canonicalize_json).collect())
        }
        other => other,
    }
}

fn load_config(path: &Path) -> Result<(cigen::schema::CigenConfig, PathBuf)> {
    if path.is_dir() {
        let config = cigen::loader::load_split_config(path)?;
        Ok((config, path.to_path_buf()))
    } else {
        let yaml = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file {}", path.display()))?;
        let config = cigen::schema::CigenConfig::from_yaml(&yaml)?;
        let root = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok((config, root))
    }
}

fn canonicalize_path(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!("Failed to resolve path {}", path.display()))
}

fn resolve_path(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn collect_files(base_dir: &Path, patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut unique = HashSet::new();

    let walker = GlobWalkerBuilder::from_patterns(base_dir, patterns)
        .file_type(FileType::FILE)
        .follow_links(false)
        .build()
        .with_context(|| {
            format!(
                "Failed to evaluate glob patterns {:?} relative to {}",
                patterns,
                base_dir.display()
            )
        })?;

    for entry in walker {
        let entry = entry?;
        let relative = entry
            .path()
            .strip_prefix(base_dir)
            .unwrap_or_else(|_| entry.path());
        unique.insert(relative.to_path_buf());
    }

    Ok(unique.into_iter().collect())
}

fn write_github_output(name: &str, value: &str) -> Result<()> {
    if let Ok(path) = std::env::var("GITHUB_OUTPUT") {
        let mut file = File::options()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("Failed to open $GITHUB_OUTPUT file {}", path))?;
        writeln!(file, "{name}={value}")?;
    } else {
        println!("{name}={value}");
    }
    Ok(())
}

#[derive(Debug)]
enum SourceEntry {
    Pattern(String),
    Group(String),
}

struct FileHasher<'a> {
    cache: HashMap<PathBuf, Vec<u8>>,
    persistent: Option<&'a mut HashCache>,
}

impl<'a> FileHasher<'a> {
    fn new(persistent: Option<&'a mut HashCache>) -> Self {
        Self {
            cache: HashMap::new(),
            persistent,
        }
    }

    fn hash_file(&mut self, absolute: &Path, relative: &Path) -> Result<Vec<u8>> {
        if let Some(bytes) = self.cache.get(relative) {
            return Ok(bytes.clone());
        }

        let metadata = fs::metadata(absolute)
            .with_context(|| format!("Failed to read metadata for {}", absolute.display()))?;

        if let Some(cache) = &mut self.persistent
            && let Some(bytes) = cache.lookup(relative, &metadata)?
        {
            self.cache.insert(relative.to_path_buf(), bytes.clone());
            return Ok(bytes);
        }

        let file = File::open(absolute)
            .with_context(|| format!("Failed to open file for hashing: {}", absolute.display()))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 1024 * 64];

        loop {
            let read = reader
                .read(&mut buffer)
                .with_context(|| format!("Failed to read {}", absolute.display()))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }

        let digest = hasher.finalize().to_vec();
        if let Some(cache) = &mut self.persistent {
            cache.store(relative, &metadata, &digest)?;
        }
        self.cache.insert(relative.to_path_buf(), digest.clone());
        Ok(digest)
    }
}

#[derive(Default, Serialize, Deserialize)]
struct HashCache {
    #[serde(skip)]
    path: Option<PathBuf>,
    entries: HashMap<String, CacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    modified: u64,
    size: u64,
    hash: String,
}

impl HashCache {
    fn load(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create cache directory {}", parent.display())
            })?;
        }

        let entries = if path.exists() {
            let file = File::open(path)
                .with_context(|| format!("Failed to open cache file {}", path.display()))?;
            serde_json::from_reader(BufReader::new(file))
                .with_context(|| format!("Failed to parse cache file {}", path.display()))?
        } else {
            HashMap::new()
        };

        Ok(Self {
            path: Some(path.to_path_buf()),
            entries,
        })
    }

    fn lookup(&self, relative: &Path, metadata: &fs::Metadata) -> Result<Option<Vec<u8>>> {
        let signature = file_signature(metadata)?;
        let key = relative.to_string_lossy();
        if let Some(entry) = self.entries.get(key.as_ref())
            && entry.modified == signature.modified
            && entry.size == signature.size
        {
            return Ok(Some(hex::decode(&entry.hash)?));
        }
        Ok(None)
    }

    fn store(&mut self, relative: &Path, metadata: &fs::Metadata, hash: &[u8]) -> Result<()> {
        let signature = file_signature(metadata)?;
        let key = relative.to_string_lossy().to_string();
        self.entries.insert(
            key,
            CacheEntry {
                modified: signature.modified,
                size: signature.size,
                hash: hex::encode(hash),
            },
        );
        Ok(())
    }

    fn save(self) -> Result<()> {
        let Some(path) = self.path else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create cache directory {}", parent.display())
            })?;
        }

        let mut file = File::create(&path)
            .with_context(|| format!("Failed to create cache file {}", path.display()))?;
        serde_json::to_writer_pretty(&mut file, &self.entries)
            .with_context(|| format!("Failed to write cache file {}", path.display()))?;
        Ok(())
    }
}

struct FileSignature {
    modified: u64,
    size: u64,
}

fn file_signature(metadata: &fs::Metadata) -> Result<FileSignature> {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    Ok(FileSignature {
        modified,
        size: metadata.len(),
    })
}
