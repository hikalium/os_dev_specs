struct SpecFileEntry {
    path: String,
    tag: String,
    title: String,
}

fn spec_file_add(body_contents: &mut Vec<String>, e: &SpecFileEntry) {
    body_contents.push(format!(
        r##"
<li><a href="{}">
  [{}]
  {}
</a></li>
"##,
        e.path.trim(),
        e.tag.trim(),
        e.title.trim()
    ));
}

fn main() {
    let mut body_contents = vec![];
    body_contents.push(String::from("<ul>"));
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
    );
    body_contents.push(String::from("</ul>"));
    let body_contents = body_contents.join("\n");
    println!(
        r##"
<!DOCTYPE html>
<head>
  <meta charset="utf-8">
  <base target="_blank">
</head>
<body>
  <h1>os_dev_specs</h1>
  {}
</body>"##,
        body_contents,
    );
}
