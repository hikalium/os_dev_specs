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

struct PdfEntry {
    id: String,
    title: String,
}
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PdfPageEntry {
    page: u64,
    description: String,
}
fn spec_file_add(body_contents: &mut Vec<String>, e: &PdfEntry, indexes: &Vec<PdfPageEntry>) {
    let id = e.id.to_string();
    let url = format!("./spec/{}.pdf", e.id.to_string().trim());
    let title = e.title.trim();
    body_contents.push(format!(
        r##"
<li class="spec">
<a href="{url}" class="spec-link">
  [{id}]
  {title}
</a>
<ul>
{}
</ul>
</li>
"##,
        indexes
            .iter()
            .map(|p| {
                let page = p.page;
                let description = &p.description;
                format!(r##"<li><a href="{url}#page={page}">p.{page}</a>: {description}</li>"##,)
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
struct Reference {
    id: String,
    source: ReferenceSourceInfo,
    entries: Vec<PdfPageEntry>,
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

fn build(path: PathBuf) -> Result<(), String> {
    println!("build from {:?}", path);
    let input = std::fs::read_to_string(path).expect("Failed to read from file");
    let input = input.trim();
    let input: Vec<&str> = input
        .split("\n")
        .map(|s| s.trim())
        .filter(|s| s.len() > 0)
        .collect();
    println!("build from {:?}", input);
    let mut input = input.iter();
    let mut ref_list = Vec::new();
    let mut maybe_id_line = input.next();
    while let Some(id_line) = maybe_id_line {
        let id = parse_id_line(id_line)?;
        println!("id: {:?}", id);
        let source = parse_reference(&mut input)?;
        println!("source: {:?}", source);
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
    let mut body_contents = vec!["<ul>".to_string()];
    ref_list.sort();
    for mut ref_info in ref_list {
        ref_info.entries.sort();
        spec_file_add(
            &mut body_contents,
            &PdfEntry {
                id: ref_info.id.clone(),
                title: match ref_info.source {
                    ReferenceSourceInfo::Pdf { title, .. } => title,
                    ReferenceSourceInfo::Zip { title, .. } => title,
                },
            },
            &ref_info.entries,
        );
    }
    body_contents.push(String::from("</ul>"));
    let mut f = std::fs::File::create("index.html").unwrap();
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
a {{
    color: #1d68cd;
    text-decoration: none;
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
  <h1>os_dev_specs</h1>
  {}
</body>"##,
        body_contents,
    ).as_bytes()).unwrap();
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
