fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/platform/macos/bridge.m");
        cc::Build::new()
            .file("src/platform/macos/bridge.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .flag("-mmacosx-version-min=11.0")
            .flag("-Wno-deprecated-declarations")
            .flag("-Wno-unguarded-availability-new")
            .compile("luna_platform");
        for framework in [
            "Cocoa",
            "Carbon",
            "CoreGraphics",
            "CoreMedia",
            "CoreVideo",
            "ImageIO",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
    }
    tauri_build::build();
}
