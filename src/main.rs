extern crate notify;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use regex::Regex;
use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::slice::Iter;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Commands,

    /// Path to data.md
    #[clap(default_value = "data.md")]
    data_path: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Download specifications
    Build,
    /// Download specifications
    Download,
    /// Monitor updates and rebuild
    Watch,
}
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Reference {
    id: String,
    source: ReferenceSourceInfo,
    entries: Vec<PdfPageEntry>,
}
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
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

fn download(ref_list: &Vec<Reference>) -> Result<()> {
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

    if !failed_ids.is_empty() {
        eprintln!(
            "The following references failed to process: {:?}",
            failed_ids
        );
    }

    let index_txt_path = format!("{}/index.txt", tmp_spec_dir);
    let output = process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "sha1sum {}/*.pdf | sed \"s#{}/##g\"",
            tmp_spec_dir, tmp_spec_dir
        ))
        .output()
        .expect("failed to execute sha1sum");
    std::fs::write(&index_txt_path, &output.stdout).unwrap();

    let old_index_path = format!("{}/index.txt", spec_dir);
    let mut needs_update = true;
    if std::path::Path::new(&old_index_path).exists() {
        let status = process::Command::new("diff")
            .args(["-u", &old_index_path, &index_txt_path])
            .status()
            .expect("failed to execute diff");
        if status.success() {
            println!("Files up to date");
            needs_update = false;
        } else {
            println!("Diff found. Do you want to update? [Enter to proceed, or Ctrl-C to cancel]");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
        }
    }

    if needs_update {
        let _ = std::fs::remove_dir_all(spec_dir);
        std::fs::create_dir_all(spec_dir).unwrap();
        let status = process::Command::new("cp")
            .args(["-rv", &format!("{}/.", tmp_spec_dir), spec_dir])
            .status()
            .expect("failed to execute cp");
        if !status.success() {
            return Err(anyhow!("cp failed with status: {}", status));
        }
    }

    if !failed_ids.is_empty() {
        return Err(anyhow!("Some files failed to download: {:?}", failed_ids));
    }

    Ok(())
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
                download(&ref_list)?;
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
}
