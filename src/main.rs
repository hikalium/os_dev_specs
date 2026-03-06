extern crate notify;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::slice::Iter;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

#[derive(Debug)]
pub struct VerificationResult {
    pub filename: String,
    pub found: bool,
    pub is_pdf: bool,
    pub hash_matched: bool,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Commands,

    /// Path to data.md
    #[clap(default_value = "data.md")]
    data_path: String,

    /// Do not modify any files.
    #[clap(long)]
    dry_run: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Download specifications
    Build,
    /// Download specifications
    Download,
    /// Monitor updates and rebuild
    Watch,
    /// Verify downloaded files
    Verify,
}
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
struct Reference {
    id: String,
    source: ReferenceSourceInfo,
    entries: Vec<PdfPageEntry>,
}
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
struct PdfPageEntry {
    page: u64,
    description: String,
}
#[derive(Clone, Copy, Debug)]
enum IndexHtmlVariant {
    Local,
    Public,
}

fn spec_file_add(body_contents: &mut Vec<String>, ref_info: &Reference, variant: IndexHtmlVariant) {
    let id = &ref_info.id;
    let url = format!("./spec/{}.pdf", id.trim());
    let source = &ref_info.source;
    let source_links = source.gen_links();
    let title = match source {
        ReferenceSourceInfo::Pdf { title, .. } => title.clone(),
        ReferenceSourceInfo::Zip { title, .. } => title.clone(),
    };
    let heading = match variant {
        IndexHtmlVariant::Local => {
            format!(
                r##"
<h3>
  [{id}]
<a href="{url}" class="spec-link">{title}</a>
<br>
<small>
{source_links}
</small>
</h3>
"##,
            )
        }
        IndexHtmlVariant::Public => format!(
            r##"
<h3>
  [{id}]
  {title}
<br>
<small>
{source_links}
</small>
</h3>
"##,
        ),
    };
    body_contents.push(format!(
        r##"
{heading}
<div>
<ul>
{}
</ul>
</div>
"##,
        ref_info
            .entries
            .iter()
            .map(|p| {
                let page = p.page;
                let description = &p.description;
                match variant {
                    IndexHtmlVariant::Local => {
                        format!(
                            r##"<li><a href="{url}#page={page}">p.{page}</a>: {description}</li>"##,
                        )
                    }
                    IndexHtmlVariant::Public => format!(r##"<li>p.{page}: {description}</li>"##),
                }
            })
            .collect::<Vec<String>>()
            .join("\n")
    ));
}

fn parse_id_line(line: &str) -> Result<String> {
    let re = Regex::new(r"#\s*`([0-9a-z_]+)`").unwrap();
    re.captures(line)
        .map(|s| s.get(1).unwrap().as_str().to_string())
        .context(anyhow!("failed to parse id line. line: {}", line))
}

fn parse_page_entry(line: &str) -> Result<PdfPageEntry> {
    let re = Regex::new(r"^-\s*p\.([0-9]+):(.*)$").unwrap();
    re.captures(line)
        .map(|c| PdfPageEntry {
            page: c
                .get(1)
                .unwrap()
                .as_str()
                .parse::<u64>()
                .expect("failed to parse page number"),
            description: c.get(2).unwrap().as_str().trim().to_string(),
        })
        .context(anyhow!("failed to parse page entry. line: {}", line))
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
enum ReferenceSourceInfo {
    Pdf {
        title: String,
        url: String,
    },
    Zip {
        title: String,
        url: String,
        rel_path: String,
    },
}

impl ReferenceSourceInfo {
    fn gen_header(&self) -> String {
        match self {
            ReferenceSourceInfo::Pdf { title, url } => format!(
                "
```
{title}
pdf
{url}
```
"
            ),
            ReferenceSourceInfo::Zip {
                title,
                url,
                rel_path,
            } => format!(
                "
```
{title}
zip
{url}
{rel_path}
```
"
            ),
        }
    }
    fn gen_links(&self) -> String {
        match self {
            ReferenceSourceInfo::Pdf { url, .. } => format!("(<a href=\"{url}\">source pdf</a>)"),
            ReferenceSourceInfo::Zip { url, rel_path, .. } => {
                format!("(<a href=\"{url}\">zip</a> @ {rel_path})")
            }
        }
    }
}

fn parse_reference(it: &mut Iter<&str>) -> Result<ReferenceSourceInfo> {
    if it.next() != Some(&"```") {
        bail!("Expected ``` after id");
    }
    let title = it
        .next()
        .map(|s| s.trim())
        .context(anyhow!("title is needed in ref_info"))?
        .to_string();
    let ref_type = it
        .next()
        .map(|s| s.trim())
        .context("type is needed in ref_info")?;
    let ref_info = match ref_type {
        "pdf" => ReferenceSourceInfo::Pdf {
            title,
            url: it
                .next()
                .map(|s| s.trim())
                .context("url is needed in ref_info".to_string())?
                .to_string(),
        },
        "zip" => ReferenceSourceInfo::Zip {
            title,
            url: it
                .next()
                .map(|s| s.trim())
                .context("url is needed in ref_info".to_string())?
                .to_string(),
            rel_path: it
                .next()
                .map(|s| s.trim())
                .context("url is needed in ref_info".to_string())?
                .to_string(),
        },
        s => return Err(anyhow!("Unexpected ref_info type: {}", s)),
    };
    if it.next() != Some(&"```") {
        bail!("Expected ``` after ref_info".to_string());
    }
    Ok(ref_info)
}

fn gen_download_script(ref_list: &Vec<Reference>) {
    println!("Generating download script...");
    let mut cmds = Vec::new();
    for ref_info in ref_list {
        let id = &ref_info.id;
        let src_info = &ref_info.source;
        match src_info {
            ReferenceSourceInfo::Pdf { url, .. } => {
                cmds.push(format!("def_spec_pdf {} {}", id, url));
            }
            ReferenceSourceInfo::Zip { url, rel_path, .. } => {
                cmds.push(format!("def_spec_zip {} {} {}", id, url, rel_path));
            }
        }
    }
    let cmds = cmds.join("\n");
    println!("{}", cmds);
    let mut f = std::fs::File::create("download_entries.generated.sh").unwrap();
    f.write_all(cmds.as_bytes()).unwrap();
}

fn gen_html(ref_list: &Vec<Reference>, dst_path: &str, variant: IndexHtmlVariant) {
    println!("Generating {dst_path}...");
    let mut body_contents = vec![];
    for ref_info in ref_list {
        spec_file_add(&mut body_contents, ref_info, variant);
    }
    let mut f = std::fs::File::create(dst_path).unwrap();
    let body_contents = body_contents.join("\n");
    f.write_all(format!(
        r##"
<!DOCTYPE html>
<head>
  <meta charset="utf-8">
  <base target="_blank">
  <link href="https://fonts.googleapis.com/css2?family=Source+Code+Pro&amp;display=swap" rel="stylesheet">
  <style>
body {{
    font-family: 'Source Code Pro', monospace;
    margin-left: 16px;
}}
div {{
    margin-left: 64px;
}}
a {{
    color: #1d68cd;
    text-decoration: none;
}}
p {{
    margin-bottom: 32px;
}}
h3 {{
    border-top: 1px dotted #9dc0f0;
    border-left: 8px solid #9dc0f0;
    padding: 8px;
    margin-top: 32px;
    margin-bottom: 8px;
}}
small {{
    color: #888888;
    text-decoration: none;
}}
ul {{
    list-style-type: none;
    margin-top: 8px;
    margin-bottom: 8px;
}}
.spec {{
    margin-top: 16px;
}}
.spec-link {{
    font-size: large;
}}
</style>
</head>
<body>
  <h1>os_dev_specs <small>({variant:?})</small></h1>
  <p>source: <a href="https://github.com/hikalium/os_dev_specs">hikalium/os_dev_specs</a></p>
  {}
</body>"##,
        body_contents,
    ).as_bytes()).unwrap();
}
fn gen_data_md_entry(
    body_contents: &mut Vec<String>,
    id: &str,
    source: &ReferenceSourceInfo,
    indexes: &[PdfPageEntry],
) {
    body_contents.push(format!(
        r##"
# `{}`
{}
{}
"##,
        id,
        source.gen_header(),
        indexes
            .iter()
            .map(|p| {
                let page = p.page;
                let description = &p.description;
                format!(r##"- p.{page}: {description}"##,)
            })
            .collect::<Vec<String>>()
            .join("\n")
    ));
}

fn update_data_md(ref_list: &Vec<Reference>) {
    let mut body_contents = vec![];
    for ref_info in ref_list {
        gen_data_md_entry(
            &mut body_contents,
            &ref_info.id,
            &ref_info.source,
            &ref_info.entries,
        );
    }
    let new_content = body_contents.join("\n");
    let old_content = std::fs::read_to_string("data.md").unwrap_or_default();
    if new_content == old_content {
        return;
    }
    let mut f = std::fs::File::create("data.md").unwrap();
    f.write_all(new_content.as_bytes()).unwrap();
}

fn parse_references(path: PathBuf) -> Result<Vec<Reference>> {
    println!("Parsing references from {:?}", path);
    let input = std::fs::read_to_string(path).expect("Failed to read from file");
    let input = input.trim();
    let input: Vec<&str> = input
        .split("\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let mut input = input.iter();
    let mut ref_list = Vec::new();
    let mut maybe_id_line = input.next();
    while let Some(id_line) = maybe_id_line {
        let id = parse_id_line(id_line)?;
        println!("Parsing id: {:?}", id);
        let source = parse_reference(&mut input)?;
        let mut page_list = Vec::new();
        loop {
            maybe_id_line = input.next();
            if let Some(maybe_page_entry) = maybe_id_line {
                if maybe_page_entry.starts_with("-") {
                    page_list.push(parse_page_entry(maybe_page_entry)?);
                    continue;
                }
            }
            break;
        }
        ref_list.push(Reference {
            id,
            source,
            entries: page_list,
        })
    }
    ref_list.sort();
    for ref_info in &mut ref_list {
        ref_info.entries.sort();
    }
    Ok(ref_list)
}

fn build(ref_list: &Vec<Reference>) -> Result<()> {
    gen_html(ref_list, "index.html", IndexHtmlVariant::Local);
    gen_html(ref_list, "docs/index.html", IndexHtmlVariant::Public);
    gen_download_script(ref_list);
    update_data_md(ref_list);

    Ok(())
}

fn download(ref_list: &Vec<Reference>, dry_run: bool) -> Result<()> {
    let download_dir = "download";
    let spec_dir = "spec";
    let tmp_spec_dir = "spec_tmp";

    std::fs::create_dir_all(download_dir).unwrap();
    std::fs::create_dir_all(spec_dir).unwrap();
    let _ = std::fs::remove_dir_all(tmp_spec_dir);
    std::fs::create_dir_all(tmp_spec_dir).unwrap();

    let mut failed_ids = Vec::new();

    for ref_info in ref_list {
        let id = &ref_info.id;
        let dst_path = format!("{}/{}.pdf", tmp_spec_dir, id);
        let result: Result<()> = (|| {
            match &ref_info.source {
                ReferenceSourceInfo::Pdf { url, .. } => {
                    let download_path = format!("{}/{}.pdf", download_dir, id);
                    if !std::path::Path::new(&download_path).exists() {
                        println!("Downloading {}...", url);
                        let status = process::Command::new("wget")
                            .args([
                                "--no-check-certificate",
                                "--user-agent=Mozilla",
                                "-O",
                                &download_path,
                                url,
                            ])
                            .status()
                            .context(anyhow!("failed to execute wget"))?;
                        if !status.success() {
                            let _ = std::fs::remove_file(&download_path);
                            return Err(anyhow!("wget failed with status: {}", status));
                        }
                    }
                    std::fs::copy(&download_path, &dst_path).context(anyhow!("failed to copy"))?;
                }
                ReferenceSourceInfo::Zip { url, rel_path, .. } => {
                    let download_path = format!("{}/{}.zip", download_dir, id);
                    if !std::path::Path::new(&download_path).exists() {
                        println!("Downloading {}...", url);
                        let status = process::Command::new("wget")
                            .args(["--user-agent=Mozilla", "-O", &download_path, url])
                            .status()
                            .context(anyhow!("failed to execute wget"))?;
                        if !status.success() {
                            let _ = std::fs::remove_file(&download_path);
                            return Err(anyhow!("wget failed with status: {}", status));
                        }
                        let status = process::Command::new("unzip")
                            .args(["-o", "-d", download_dir, &download_path])
                            .status()
                            .context(anyhow!("failed to execute unzip"))?;
                        if !status.success() {
                            return Err(anyhow!("unzip failed with status: {}", status));
                        }
                    }
                    let src_path = format!("{}/{}", download_dir, rel_path);
                    if !std::path::Path::new(&src_path).exists() {
                        return Err(anyhow!("extracted file not found: {}", src_path));
                    }
                    std::fs::copy(&src_path, &dst_path).context(anyhow!("failed to copy"))?;
                }
            }
            Ok(())
        })();

        if let Err(e) = result {
            eprintln!("Error processing {}: {}", id, e);
            failed_ids.push(id.clone());
        }
    }

    let new_map: HashMap<String, String> = ref_list
        .iter()
        .map(|r| {
            let filename = format!("{}.pdf", r.id);
            let filepath = PathBuf::from(format!("{}/{}", tmp_spec_dir, filename));
            if filepath.exists() {
                let output = process::Command::new("sha1sum")
                    .arg(&filepath)
                    .output()
                    .expect("failed to execute sha1sum");
                let hash_line = String::from_utf8(output.stdout).unwrap();
                let hash = hash_line
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                (filename, hash)
            } else {
                (filename, "?".to_string())
            }
        })
        .collect();

    let old_index_path = format!("{}/index.txt", spec_dir);
    let mut needs_update = true;
    if std::path::Path::new(&old_index_path).exists() {
        let old_index_content = std::fs::read_to_string(&old_index_path).unwrap();
        let old_map: HashMap<String, String> = old_index_content
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    Some((parts[1].to_string(), parts[0].to_string()))
                } else {
                    None
                }
            })
            .collect();

        if old_map.iter().all(|(k, v)| new_map.get(k) == Some(v)) && old_map.len() == new_map.len()
        {
            println!("Files up to date");
            needs_update = false;
            let _ = std::fs::remove_dir_all(tmp_spec_dir);
        } else {
            println!("Diff found.");

            let mut all_files: HashSet<String> = old_map.keys().cloned().collect();
            all_files.extend(new_map.keys().cloned());

            let mut sorted_files: Vec<String> = all_files.into_iter().collect();
            sorted_files.sort();

            for filename in sorted_files {
                let old_hash = old_map.get(&filename);
                let new_hash = new_map.get(&filename);

                match (old_hash, new_hash) {
                    (Some(old_h), Some(new_h)) => {
                        if old_h != new_h {
                            let filepath = PathBuf::from(format!("{}/{}", tmp_spec_dir, filename));
                            let verification_result = verify_file(&filepath, new_h)?;
                            println!(
                                "~ {:32} <downloaded> is_pdf: {} hash_matched: {}",
                                filename,
                                verification_result.is_pdf,
                                verification_result.hash_matched
                            );
                        }
                    }
                    (None, Some(new_h)) => {
                        if new_h == "?" {
                            println!(
                                "+ {:32} found: false is_pdf: false hash_matched: false",
                                filename
                            );
                        } else {
                            let filepath = PathBuf::from(format!("{}/{}", tmp_spec_dir, filename));
                            let verification_result = verify_file(&filepath, new_h)?;
                            println!(
                                "+ {:32} <downloaded> is_pdf: {} hash_matched: {}",
                                filename,
                                verification_result.is_pdf,
                                verification_result.hash_matched
                            );
                        }
                    }
                    (Some(_), None) => {
                        println!(
                            "- {:32} found: true is_pdf: true hash_matched: true",
                            filename
                        );
                    }
                    (None, None) => {}
                }
            }

            if dry_run {
                println!("Dry run: no changes will be applied.");
                let _ = std::fs::remove_dir_all(tmp_spec_dir);
                return Ok(());
            }
            println!("Do you want to update? [Enter to proceed, or Ctrl-C to cancel]");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
        }
    }

    if needs_update {
        let _ = std::fs::remove_dir_all(spec_dir);
        std::fs::rename(tmp_spec_dir, spec_dir).unwrap();
        let new_index_content = new_map
            .iter()
            .map(|(filename, hash)| format!("{}  {}", hash, filename))
            .collect::<Vec<String>>()
            .join("\n");
        std::fs::write(&old_index_path, &new_index_content).unwrap();
    }

    if !failed_ids.is_empty() {
        return Err(anyhow!("Some files failed to download: {:?}", failed_ids));
    }

    Ok(())
}

fn verify_file(filepath: &PathBuf, expected_hash: &str) -> Result<VerificationResult> {
    let filename = filepath
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut is_pdf = false;
    let mut hash_matched = false;
    let found = filepath.exists();

    if found {
        // Check file type
        let file_output = process::Command::new("file")
            .arg(filepath)
            .output()
            .context("failed to execute 'file' command")?;
        let file_type = String::from_utf8_lossy(&file_output.stdout);
        is_pdf = file_type.contains("PDF document");

        // Check hash
        let sha1_output = process::Command::new("sha1sum")
            .arg(filepath)
            .output()
            .context("failed to execute 'sha1sum' command")?;
        let sha1_hash = String::from_utf8_lossy(&sha1_output.stdout);
        let calculated_hash = sha1_hash.split_whitespace().next().unwrap_or("");
        hash_matched = calculated_hash == expected_hash;
    }

    Ok(VerificationResult {
        filename,
        found,
        is_pdf,
        hash_matched,
    })
}

fn watch_and_build(rx: Receiver<DebouncedEvent>, target_path: PathBuf) -> Result<()> {
    use notify::DebouncedEvent::*;
    use std::time::Instant;
    let target_path = std::fs::canonicalize(&target_path).unwrap_or(target_path);
    let mut last_self_update = Instant::now();
    loop {
        match rx.recv() {
            Ok(event) => {
                println!("Got event: {:?}", event);
                let mut path_to_check = None;
                match event {
                    Create(p) | Write(p) | Chmod(p) => {
                        path_to_check = Some(p);
                    }
                    Rename(_, p) => {
                        path_to_check = Some(p);
                    }
                    _ => {}
                };

                let should_build = if let Some(p) = path_to_check {
                    std::fs::canonicalize(&p)
                        .map(|p| p == target_path)
                        .unwrap_or(false)
                } else {
                    false
                };

                if should_build {
                    if last_self_update.elapsed() < Duration::from_secs(2) {
                        println!("Ignoring event to avoid loop (too close to last update)");
                        continue;
                    }
                    println!("Rebuilding due to change in {:?}", target_path);
                    let ref_list = parse_references(target_path.clone())?;
                    build(&ref_list)?;
                    last_self_update = Instant::now();
                }
            }
            Err(e) => return Err(anyhow!("watch error: {:?}", e)),
        }
    }
}

fn verify(data_path: &str) -> Result<()> {
    let ref_list = parse_references(PathBuf::from(data_path))?;
    let expected_files: HashSet<String> =
        ref_list.iter().map(|r| format!("{}.pdf", r.id)).collect();

    let index_content =
        std::fs::read_to_string("spec/index.txt").context("failed to read spec/index.txt")?;

    let mut found_files = HashSet::new();

    for line in index_content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 2 {
            continue;
        }
        let hash = parts[0];
        let filename = parts[1];
        let filepath = PathBuf::from(format!("spec/{}", filename));

        found_files.insert(filename.to_string());

        let verification_result = verify_file(&filepath, hash)?;
        println!(
            "{:32} found: {} is_pdf: {} hash_matched: {}",
            verification_result.filename,
            verification_result.found,
            verification_result.is_pdf,
            verification_result.hash_matched
        );
    }

    let missing_files: Vec<String> = expected_files.difference(&found_files).cloned().collect();

    for filename in missing_files {
        println!(
            "{:32} found: false is_pdf: false hash_matched: false",
            filename
        );
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let file_to_watch = PathBuf::from(&args.data_path);
    let canonical_path = std::fs::canonicalize(&file_to_watch).unwrap_or(file_to_watch.clone());
    let parent_dir = canonical_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    println!("File to watch: {:?}", &canonical_path);
    println!("Watching directory: {:?}", parent_dir);
    match args.command {
        Commands::Build => {
            let ref_list = parse_references(canonical_path.clone())?;
            build(&ref_list)?;
            Ok(())
        }
        Commands::Download => {
            if let Err(e) = || -> Result<()> {
                let ref_list = parse_references(file_to_watch)?;
                download(&ref_list, args.dry_run)?;
                Ok(())
            }() {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
            Ok(())
        }
        Commands::Watch => {
            let (tx, rx) = channel();
            let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
            watcher
                .watch(parent_dir, RecursiveMode::NonRecursive)
                .unwrap();
            watch_and_build(rx, canonical_path)
        }
        Commands::Verify => verify(&args.data_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_line() {
        assert_eq!(parse_id_line("# `id`").unwrap(), "id".to_string());
        assert!(matches!(parse_id_line("#"), Err(_)));
        assert!(matches!(parse_id_line("# "), Err(_)));
        assert!(matches!(parse_id_line("# ``"), Err(_)));
        assert!(matches!(parse_id_line("`id`"), Err(_)));
    }
    #[test]
    fn page_line() {
        assert_eq!(
            parse_page_entry("- p.268: page1").unwrap(),
            PdfPageEntry {
                page: 268,
                description: "page1".to_string()
            }
        );
        assert!(matches!(parse_id_line("#"), Err(_)));
        assert!(matches!(parse_id_line("# "), Err(_)));
        assert!(matches!(parse_id_line("# ``"), Err(_)));
        assert!(matches!(parse_id_line("`id`"), Err(_)));
    }

    #[test]
    fn download_prioritizes_missing_file() {
        let tmp_dir = tempfile::Builder::new().prefix("test-").tempdir().unwrap();
        let spec_dir = tmp_dir.path().join("spec");
        std::fs::create_dir(&spec_dir).unwrap();
        let data_md_path = tmp_dir.path().join("data.md");
        let index_txt_path = spec_dir.join("index.txt");

        let mut data_md_content = "# `test_spec`\n".to_string();
        data_md_content.push_str("```\nTest Spec\npdf\nhttps://example.com/test.pdf\n```\n");
        std::fs::write(&data_md_path, data_md_content).unwrap();
        std::fs::write(&index_txt_path, "12345  test_spec.pdf\n").unwrap();

        let ref_list = parse_references(data_md_path).unwrap();

        let existing_ids: HashSet<String> = std::fs::read_dir(&spec_dir)
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |e| e == "pdf") {
                    path.file_stem().map(|s| s.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect();

        let (missing_refs, _): (Vec<_>, Vec<_>) = ref_list
            .iter()
            .cloned()
            .partition(|r| !existing_ids.contains(&r.id));

        assert_eq!(missing_refs.len(), 1);
    }
}
