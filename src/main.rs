#![feature(assert_matches)]
extern crate notify;
use clap::Parser;
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
    /// Path to data.md
    data_path: String,

    /// Monitor updates and rebuild
    #[clap(short, long)]
    watch: bool,
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
<h3><a href="{url}" class="spec-link">
  [{id}]
  {title}
</a>
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

fn parse_id_line(line: &str) -> Result<String, String> {
    let re = Regex::new(r"#\s*`([0-9a-z_]+)`").unwrap();
    re.captures(line)
        .map(|s| s.get(1).unwrap().as_str().to_string())
        .ok_or(format!("failed to parse id line. line: {}", line))
}

fn parse_page_entry(line: &str) -> Result<PdfPageEntry, String> {
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
        .ok_or(format!("failed to parse page entry. line: {}", line))
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
            ReferenceSourceInfo::Pdf { url, .. } => format!("(<a href=\"{url}\">pdf</a>)"),
            ReferenceSourceInfo::Zip { url, rel_path, .. } => {
                format!("(<a href=\"{url}\">zip</a> @ {rel_path})")
            }
        }
    }
}

fn parse_reference(it: &mut Iter<&str>) -> Result<ReferenceSourceInfo, String> {
    if it.next() != Some(&"```") {
        return Err("Expected ``` after id".to_string());
    }
    let title = it
        .next()
        .map(|s| s.trim())
        .ok_or("title is needed in ref_info")?
        .to_string();
    let ref_type = it
        .next()
        .map(|s| s.trim())
        .ok_or("type is needed in ref_info")?;
    let ref_info = match ref_type {
        "pdf" => ReferenceSourceInfo::Pdf {
            title,
            url: it
                .next()
                .map(|s| s.trim())
                .ok_or("url is needed in ref_info".to_string())?
                .to_string(),
        },
        "zip" => ReferenceSourceInfo::Zip {
            title,
            url: it
                .next()
                .map(|s| s.trim())
                .ok_or("url is needed in ref_info".to_string())?
                .to_string(),
            rel_path: it
                .next()
                .map(|s| s.trim())
                .ok_or("url is needed in ref_info".to_string())?
                .to_string(),
        },
        s => return Err(format!("Unexpected ref_info type: {}", s)),
    };
    if it.next() != Some(&"```") {
        return Err("Expected ``` after ref_info".to_string());
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
        spec_file_add(&mut body_contents, &ref_info, variant);
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
    border-top: 1px solid #9dc0f0;
    border-left: 4px solid #9dc0f0;
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
    id: &String,
    source: &ReferenceSourceInfo,
    indexes: &Vec<PdfPageEntry>,
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
    let mut f = std::fs::File::create("data.md").unwrap();
    let body_contents = body_contents.join("\n");
    f.write_all(body_contents.as_bytes()).unwrap();
}

fn build(path: PathBuf) -> Result<(), String> {
    println!("build from {:?}", path);
    let input = std::fs::read_to_string(path).expect("Failed to read from file");
    let input = input.trim();
    let input: Vec<&str> = input
        .split("\n")
        .map(|s| s.trim())
        .filter(|s| s.len() > 0)
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

    gen_html(&ref_list, "index.html", IndexHtmlVariant::Local);
    gen_html(&ref_list, "docs/index.html", IndexHtmlVariant::Public);
    gen_download_script(&ref_list);
    update_data_md(&ref_list);

    Ok(())
}

fn watch_and_build(rx: Receiver<DebouncedEvent>) -> Result<(), String> {
    use notify::DebouncedEvent::*;
    loop {
        match rx.recv() {
            Ok(event) => {
                println!("{:?}", event);
                match event {
                    Create(path) => build(path)?,
                    _ => {}
                }
            }
            Err(e) => return Err(format!("watch error: {:?}", e)),
        }
    }
}
fn main() {
    let args = Args::parse();
    let file_to_watch = args.data_path;
    let do_watch = args.watch;

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
    println!("File to watch: {}", &file_to_watch);
    watcher
        .watch(file_to_watch.clone(), RecursiveMode::NonRecursive)
        .unwrap();
    if let Err(e) = || -> Result<(), String> {
        build(file_to_watch.into())?;
        if do_watch {
            watch_and_build(rx)?;
        }
        Ok(())
    }() {
        eprintln!("Error: {}", e);
        process::exit(1);
    };
}

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;

    use super::*;

    #[test]
    fn id_line() {
        assert_eq!(parse_id_line("# `id`").as_deref(), Ok("id"));
        assert_matches!(parse_id_line("#"), Err(_));
        assert_matches!(parse_id_line("# "), Err(_));
        assert_matches!(parse_id_line("# ``"), Err(_));
        assert_matches!(parse_id_line("`id`"), Err(_));
    }
    #[test]
    fn page_line() {
        assert_eq!(
            parse_page_entry("- p.268: page1"),
            Ok(PdfPageEntry {
                page: 268,
                description: "page1".to_string()
            })
        );
        assert_matches!(parse_id_line("#"), Err(_));
        assert_matches!(parse_id_line("# "), Err(_));
        assert_matches!(parse_id_line("# ``"), Err(_));
        assert_matches!(parse_id_line("`id`"), Err(_));
    }
}
