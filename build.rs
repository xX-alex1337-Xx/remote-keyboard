fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        println!("cargo:rerun-if-changed=app.rc");

        let out_dir = std::env::var("OUT_DIR").unwrap();
        let res_path = format!("{}/app.res", out_dir);

        let status = std::process::Command::new("windres")
            .args(["app.rc", "-O", "coff", "-o", &res_path])
            .status();

        if let Ok(s) = status {
            if s.success() {
                println!("cargo:rustc-link-arg={}", res_path);
            }
        }
    }
}
