// ponytail: build a multi-resolution .ico from Logo.png and embed it as the
// binary icon on Windows. no external tools required.

use std::env;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=Logo.png");
    println!("cargo:rerun-if-changed=build.rs");

    let png_bytes = fs::read("Logo.png").expect("Logo.png missing from repo root");
    let img = image::load_from_memory(&png_bytes).expect("Logo.png is not a valid image");

    let sizes = [16u32, 32, 48, 64, 128, 256];
    let mut entries: Vec<(u32, u32, Vec<u8>)> = Vec::with_capacity(sizes.len());
    for &s in &sizes {
        let resized = img.resize_exact(s, s, image::imageops::FilterType::Lanczos3);
        let mut buf = Vec::new();
        resized
            .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
            .expect("png encode");
        entries.push((s, s, buf));
    }

    let ico = build_ico(&entries);

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR set by cargo");
    let ico_path = PathBuf::from(&out_dir).join("Logo.ico");
    fs::write(&ico_path, &ico).expect("write Logo.ico");

    // Also drop a copy next to the binary for the NSIS installer to pick up.
    let dist_ico = PathBuf::from("installer").join("Logo.ico");
    fs::create_dir_all("installer").ok();
    fs::write(&dist_ico, &ico).ok();

    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon(ico_path.to_str().expect("ico path utf-8"));
        res.set("FileDescription", "ephe - Ephemeral Local Text Pad");
        res.set("ProductName", "ephe");
        res.set("OriginalFilename", "ephe.exe");
        res.set("InternalName", "ephe");
        if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
            res.set_windres_path("windres.exe");
        }
        if let Err(e) = res.compile() {
            println!("cargo:warning=winres icon embedding skipped: {e}");
        } else {
            let res_obj = PathBuf::from(&out_dir).join("resource.o");
            if res_obj.exists() {
                println!("cargo:rustc-link-arg={}", res_obj.display());
            }
        }
    }
}

fn build_ico(entries: &[(u32, u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    // ICONDIR
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type = icon
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());

    let header_size = 6 + entries.len() * 16;
    let mut offset = header_size as u32;
    let mut offsets = Vec::with_capacity(entries.len());
    for (_, _, data) in entries {
        offsets.push(offset);
        offset += data.len() as u32;
    }

    for ((w, h, data), off) in entries.iter().zip(offsets.iter()) {
        out.push(if *w >= 256 { 0 } else { *w as u8 });
        out.push(if *h >= 256 { 0 } else { *h as u8 });
        out.push(0); // palette colors
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bit count
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&off.to_le_bytes());
    }

    for (_, _, data) in entries {
        out.extend_from_slice(data);
    }

    out
}
