fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=FontCheck.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("FontCheck.ico");
        let _ = res.compile();
    }
}
