struct SpecFileEntry {
    path: String,
    tag: String,
    title: String,
}
struct SpecFilePageEntry {
    page: u64,
    description: String,
}

fn spec_file_add(
    body_contents: &mut Vec<String>,
    e: &SpecFileEntry,
    indexes: &[&SpecFilePageEntry],
) {
    body_contents.push(format!(
        r##"
<li class="spec">
<a href="{}" class="spec-link">
  [{}]
  {}
</a>
<ul>
{}
</ul>
</li>
"##,
        e.path.trim(),
        e.tag.trim(),
        e.title.trim(),
        indexes
            .iter()
            .map(|p| format!(
                r##"<li><a href="{}#page={}">p.{1}</a>: {}</li>"##,
                e.path.trim(),
                p.page,
                p.description
            ))
            .collect::<Vec<String>>()
            .join("\n")
    ));
}

fn main() {
    let mut body_contents = vec!["<ul>".to_string()];
    spec_file_add(
        &mut body_contents,
        &SpecFileEntry {
            path: "./spec/extensible-host-controler-interface-usb-xhci.pdf".to_string(),
            tag: "XHCI".to_string(),
            title: r##"
eXtensible Host Controller Interface for Universal Serial Bus (xHCI)
Requirements Specification
May 2019 Revision 1.2
        "##
            .to_string(),
        },
        &[&SpecFilePageEntry {
            page: 406,
            description: "5.4.8 Port Status and Control Register (PORTSC)".to_string(),
        }],
    );
    spec_file_add(
        &mut body_contents,
        &SpecFileEntry {
            path: "./spec/usb_20_20190524/usb_20.pdf".to_string(),
            tag: "USB2.0".to_string(),
            title: r##"
Universal Serial Bus Specification Revision 2.0
        "##
            .to_string(),
        },
        &[
            &SpecFilePageEntry {
                page: 268,
                description: "Figure 9-1. Device State Diagram".to_string(),
            },
            &SpecFilePageEntry {
                page: 278,
                description: "9.4 Standard Device Requests".to_string(),
            },
            &SpecFilePageEntry {
                page: 279,
                description: "Table 9-4. Standard Request Codes".to_string(),
            },
            &SpecFilePageEntry {
                page: 279,
                description: "Table 9-5. Descriptor Types".to_string(),
            },
            &SpecFilePageEntry {
                page: 281,
                description: "9.4.3 Get Descriptor Request".to_string(),
            },
            &SpecFilePageEntry {
                page: 301,
                description: "9.6.7 String Descriptor".to_string(),
            },
        ],
    );
    spec_file_add(
        &mut body_contents,
        &SpecFileEntry {
            path: "./spec/CDC1.2_WMC1.1_012011/CDC1.2_WMC1.1/usbcdc12/CDC120-20101103-track.pdf"
                .to_string(),
            tag: "USBCDC1.2".to_string(),
            title: r##"
Universal Serial Bus
Class Definitions for
Communications Devices
        "##
            .to_string(),
        },
        &[&SpecFilePageEntry {
            page: 25,
            description: "Table 12: Type Values for the bDescriptorType Field".to_string(),
        }],
    );
    spec_file_add(
        &mut body_contents,
        &SpecFileEntry {
            path: "./spec/CDC1.2_WMC1.1_012011/CDC1.2_WMC1.1/usbcdc12/ECM120.pdf".to_string(),
            tag: "USBCDC/ECM120".to_string(),
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
