//! On Windows, gives the executable its icon and the details Explorer shows
//! for it. Everywhere else there is nothing to do.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/bigstar.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../packaging/bigstar.ico").set("ProductName", "Big Star Editor").set("FileDescription", "Big Star Editor");
        // An executable without its icon still runs; the build says so.
        if let Err(e) = resource.compile() {
            println!("cargo:warning=the executable has no icon: {e}");
        }
    }
}
