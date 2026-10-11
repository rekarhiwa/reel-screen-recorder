fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "ڕیل — سکرین ڕیکۆردەر");
        res.set("FileDescription", "REEL Kurdish screen recorder");
        res.compile().expect("failed to compile Windows resources");
    }
}
