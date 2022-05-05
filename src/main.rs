#![feature(assert_matches)]
extern crate notify;
use notify::{watcher, RecursiveMode, Watcher};
use regex::Regex;
use std::path::PathBuf;
use std::process;
use std::slice::Iter;
use std::sync::mpsc::channel;
use std::time::Duration;

struct PdfEntry<'a> {
    id: &'a str,
    title: String,
}
struct PdfPageEntry<'a> {
    page: u64,
    description: &'a str,
}
fn spec_file_add(body_contents: &mut Vec<String>, e: &PdfEntry, indexes: &[&PdfPageEntry]) {
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

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;

    use super::*;

    #[test]
    fn id_line() {
        assert_eq!(parse_id_line("# id").as_deref(), Ok("id"));
        assert_matches!(parse_id_line("#"), Err(_));
        assert_matches!(parse_id_line("# "), Err(_));
    }
}

#[derive(Debug)]
enum ReferenceInfo {
    Pdf { url: String },
    Zip { url: String, rel_path: String },
}

fn parse_reference(it: &mut Iter<&str>) -> Result<ReferenceInfo, String> {
    if it.next() != Some(&"```") {
        return Err("Expected ``` after id".to_string());
    }
    let ref_type = it
        .next()
        .map(|s| s.trim())
        .ok_or("type is needed in ref_info")?;
    let ref_info = match ref_type {
        "pdf" => ReferenceInfo::Pdf {
            url: it
                .next()
                .map(|s| s.trim())
                .ok_or("url is needed in ref_info".to_string())?
                .to_string(),
        },
        "zip" => ReferenceInfo::Zip {
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
    while let Some(id_line) = input.next() {
        let id = parse_id_line(id_line)?;
        println!("id: {:?}", id);
        let ref_info = parse_reference(&mut input)?;
        println!("ref: {:?}", ref_info);
    }
    Ok(())
}

fn watch_and_build(file_to_watch: String) -> Result<(), String> {
    use notify::DebouncedEvent::*;
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
    println!("File to watch: {}", &file_to_watch);
    watcher
        .watch(file_to_watch.clone(), RecursiveMode::NonRecursive)
        .unwrap();
    build(file_to_watch.into())?;
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
    let mut args = std::env::args();
    let file_to_watch = args.nth(1).expect("Usage: cargo run <file_to_watch>");
    if let Err(e) = watch_and_build(file_to_watch) {
        eprintln!("Error: {}", e);
        process::exit(1);
    };

    let mut body_contents = vec!["<ul>".to_string()];
    spec_file_add(
        &mut body_contents,
        &PdfEntry {
            id: "acpi_6_4",
            title: r##"
            Advanced Configuration and Power
Interface (ACPI) Specification
        "##
            .to_string(),
        },
        &[],
    );
    spec_file_add(
        &mut body_contents,
        &PdfEntry {
            id: "armv8a_pg_1_0",
            title: r##"
ARM Cortex-A Series Version: 1.0
Programmer’s Guide for ARMv8-A
        "##
            .to_string(),
        },
        &[&PdfPageEntry {
            page: 88,
            description: "6.5.4 Hint instructions (WFI)",
        }],
    );
    spec_file_add(
        &mut body_contents,
        &PdfEntry {
            id: "cdc_1_2",
            title: r##"
Universal Serial Bus
Class Definitions for
Communications Devices
        "##
            .to_string(),
        },
        &[
            &PdfPageEntry {
                page: 16,
                description: "3.4.2 Data Class Interface",
            },
            &PdfPageEntry {
                page: 20,
                description: "02h: Communications Device Class Code",
            },
            &PdfPageEntry {
                page: 20,
                description: "02h: Communications Interface Class Code",
            },
            &PdfPageEntry {
                page: 20,
                description: "06h: Ethernet Networking Control Model: Interface Subclass Code",
            },
            &PdfPageEntry {
                page: 21,
                description: "0Ah: Data Interface Class",
            },
            &PdfPageEntry {
                page: 25,
                description: "Table 12: Type Values for the bDescriptorType Field",
            },
        ],
    );
    spec_file_add(
        &mut body_contents,
        &PdfEntry {
            id: "ecm_1_2",
            title: r##"
Universal Serial Bus
Communications Class
Subclass Specification for
Ethernet Control Model Devices Revision 1.2
        "##
            .to_string(),
        },
        &[],
    );
    spec_file_add(
        &mut body_contents,
        &PdfEntry {
            id: "uefi_2_9",
            title: r##"
            Unified Extensible Firmware Interface (UEFI)
Specification
        "##
            .to_string(),
        },
        &[],
    );
    spec_file_add(
        &mut body_contents,
        &PdfEntry {
            id: "usb_2_0",
            title: r##"
Universal Serial Bus Specification Revision 2.0
        "##
            .to_string(),
        },
        &[
            &PdfPageEntry {
                page: 268,
                description: "Figure 9-1. Device State Diagram",
            },
            &PdfPageEntry {
                page: 278,
                description: "9.4 Standard Device Requests",
            },
            &PdfPageEntry {
                page: 279,
                description: "Table 9-4. Standard Request Codes",
            },
            &PdfPageEntry {
                page: 279,
                description: "Table 9-5. Descriptor Types",
            },
            &PdfPageEntry {
                page: 281,
                description: "9.4.3 Get Descriptor Request",
            },
            &PdfPageEntry {
                page: 282,
                description: r##""All devices must provide a device descriptor and at least one configuration descriptor""##,
            },
            &PdfPageEntry {
                page: 297,
                description: r##"9.6.6 Endpoint Descriptor"##,
            },
            &PdfPageEntry {
                page: 301,
                description: "9.6.7 String Descriptor",
            },
        ],
    );
    spec_file_add(
        &mut body_contents,
        &PdfEntry {
            id: "xhci_1_2",
            title: r##"
eXtensible Host Controller Interface for Universal Serial Bus (xHCI)
Requirements Specification
May 2019 Revision 1.2
        "##
            .to_string(),
        },
        &[
            &PdfPageEntry {
                page: 57,
                description: "Figure 3-3: General Architecture of the xHCI interface",
            },
            &PdfPageEntry {
                page: 83,
                description: "4.3 USB Device Initialization",
            },
            &PdfPageEntry {
                page: 91,
                description: "4.3.6 Setting Alternate Interfaces",
            },
            &PdfPageEntry {
                page: 160,
                description: "4.8 Endpoint",
            },
            &PdfPageEntry {
                page: 161,
                description: "4.8.2 Endpoint Context Initialization",
            },
            &PdfPageEntry {
                page: 163,
                description: "Figure 4-5: Endpoint State Diagram",
            },
            &PdfPageEntry {
                page: 370,
                description: "Register Attributes",
            },
            &PdfPageEntry {
                page: 406,
                description: "5.4.8 Port Status and Control Register (PORTSC)",
            },
            &PdfPageEntry {
                page: 454,
                description: "6.2.3.2 Configure Endpoint Command Usage",
            },
            &PdfPageEntry {
                page: 459,
                description: "6.2.5 Input Context",
            },
            &PdfPageEntry {
                page: 461,
                description: "6.2.5.1 Input Control Context",
            },
            &PdfPageEntry {
                page: 491,
                description: "6.4.3.5 Configure Endpoint Command TRB",
            },
        ],
    );
    body_contents.push(String::from("</ul>"));
    let body_contents = body_contents.join("\n");
    println!(
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
    );
}
