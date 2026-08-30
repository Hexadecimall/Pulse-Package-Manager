//! Compiles the C shim that drives libwine (see src/wine/wine_run.c). Pulse is
//! multilanguage: the wine-run path is C linked into the library via FFI.

fn main() {
    cc::Build::new()
        .file("src/wine/wine_run.c")
        .warnings(false)
        .compile("pulse_wine");
    println!("cargo:rerun-if-changed=src/wine/wine_run.c");
}
