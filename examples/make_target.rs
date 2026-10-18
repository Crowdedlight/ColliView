//! Writes the printable centring target, `calibration/centring_target.pdf`.
//!
//! Usage: `cargo run --example make_target [-- <out.pdf>]`
//!
//! The pattern comes from `src/target.rs`, which the simulator renders too. The PDF is
//! written by hand: one filled path of rectangles and a caption, so no PDF crate is needed.

#[allow(dead_code)]
#[path = "../src/target.rs"]
mod target;

use std::fmt::Write as _;
use std::path::PathBuf;

const PT_PER_MM: f32 = 72.0 / 25.4;
const CAPTION: &str =
    "ColliView centring target  -  print at any scale  -  tape it flat inside the tube, in front of the focuser";

fn main() -> std::io::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("calibration/centring_target.pdf"));
    let pdf = build_pdf();
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&out, &pdf)?;
    println!("Wrote {} ({} bytes)", out.display(), pdf.len());
    Ok(())
}

fn build_pdf() -> Vec<u8> {
    let (pw, ph) = target::PAGE_MM;
    let mm = |v: f32| v * PT_PER_MM;
    // All squares in one path and one fill, so touching squares show no seams.
    let mut content = String::from("0 g\n");
    for r in target::black_runs() {
        // PDF y runs up from the bottom of the page.
        let _ = writeln!(content, "{:.3} {:.3} {:.3} {:.3} re", mm(r.x), mm(ph - r.y - r.h), mm(r.w), mm(r.h));
    }
    content.push_str("f\n");
    let _ = writeln!(content, "BT /F1 8 Tf {:.3} {:.3} Td ({CAPTION}) Tj ET", mm(target::ORIGIN_MM.0), mm(6.0));

    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.3} {:.3}] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>",
            mm(pw),
            mm(ph)
        ),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_owned(),
    ];
    // The second line's high bytes mark the file as binary.
    let mut pdf = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = pdf.len();
    // Each cross-reference entry is exactly 20 bytes.
    let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for o in offsets {
        let _ = writeln!(tail, "{o:010} 00000 n ");
    }
    let _ = write!(tail, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1);
    pdf.extend_from_slice(tail.as_bytes());
    pdf
}
