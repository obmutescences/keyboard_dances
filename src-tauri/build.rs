fn main() {
    println!("cargo:rerun-if-changed=../ui/index.html");
    println!("cargo:rerun-if-changed=../ui/main.js");
    println!("cargo:rerun-if-changed=../ui/styles.css");

    ensure_png_icon();
    tauri_build::build()
}

fn ensure_png_icon() {
    let manifest_dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let icon_path = manifest_dir.join("icons").join("icon.png");
    if icon_path.exists() {
        return;
    }

    std::fs::create_dir_all(icon_path.parent().expect("icon parent")).expect("create icon dir");
    let file = std::fs::File::create(&icon_path).expect("create icon.png");
    let mut encoder = png::Encoder::new(file, 128, 128);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("write png header");

    let mut rgba = Vec::with_capacity(128 * 128 * 4);
    for y in 0_i32..128 {
        for x in 0_i32..128 {
            let border = x == 18 || x == 109 || y == 32 || y == 95;
            let key_body = (19..=108).contains(&x) && (33..=94).contains(&y);
            let dot = ((x - 50).pow(2) + (y - 64).pow(2)) < 120
                || ((x - 78).pow(2) + (y - 64).pow(2)) < 120;
            let pixel = if border {
                [29, 45, 54, 255]
            } else if dot {
                [255, 193, 84, 255]
            } else if key_body {
                [80, 173, 132, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba.extend_from_slice(&pixel);
        }
    }

    writer.write_image_data(&rgba).expect("write png data");
}
