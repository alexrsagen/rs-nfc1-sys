extern crate bindgen;
extern crate cc;
#[cfg(feature = "vendored")]
extern crate cmake;
extern crate pkg_config;

#[cfg(target_os = "windows")]
extern crate find_winsdk;

use bindgen::Builder;
use std::env::{var, var_os};
use std::path::PathBuf;

#[cfg(feature = "vendored")]
static VERSION: &'static str = "1.8.0";

#[cfg(feature = "vendored")]
macro_rules! on_by_feature {
    ($f: literal) => {
        if cfg!(feature = $f) {
            "ON"
        } else {
            "OFF"
        }
    };
}

#[cfg(all(target_os = "windows", feature = "vendored"))]
fn set_pcsc_config_windows(config: &mut cmake::Config) {
    // Find Windows SDK base path
    let winsdk = find_winsdk::SdkInfo::find(find_winsdk::SdkVersion::Any);
    if let Err(e) = winsdk {
        panic!("Unable to find Windows SDK: {}", e);
    }
    let winsdk = winsdk.unwrap();
    if winsdk.is_none() {
        panic!("Unable to find Windows SDK. Please ensure the appropriate version of the Windows SDK for your target platform is installed with the correct feature set.");
    }
    let winsdk = winsdk.unwrap();

    // Find Windows SDK include path
    let winsdk_include_path = winsdk.installation_folder().join("Include");
    let mut winsdk_um_include_path = None;
    if let Ok(include_dirs) = std::fs::read_dir(&winsdk_include_path) {
        for dir_entry in include_dirs {
            if let Ok(entry) = dir_entry {
                if let Some(dir_name) = entry.file_name().to_str() {
                    if dir_name.starts_with(winsdk.product_version()) {
                        winsdk_um_include_path =
                            Some(winsdk_include_path.join(dir_name).join("um"));
                        break;
                    }
                }
            }
        }
    }
    if let Some(winsdk_um_include_path) = &winsdk_um_include_path {
        config.define("PCSC_INCLUDE_DIRS", winsdk_um_include_path);
    } else {
        panic!("Unable to find Windows SDK include path. Please ensure the appropriate version of the Windows SDK for your target platform is installed with the correct feature set.");
    }

    // Find Windows SDK library path
    let winsdk_lib_path = winsdk.installation_folder().join("Lib");
    let mut winsdk_um_lib_path = None;
    let winsdk_arch = match var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86") => Some("x86"),
        Ok("x86_64") => Some("x64"),
        Ok("arm") => Some("arm"),
        Ok("aarch64") => Some("arm64"),
        _ => None,
    };
    if let Some(winsdk_arch) = winsdk_arch {
        if let Ok(lib_dirs) = std::fs::read_dir(&winsdk_lib_path) {
            for dir_entry in lib_dirs {
                if let Ok(entry) = dir_entry {
                    if let Some(dir_name) = entry.file_name().to_str() {
                        if dir_name.starts_with(winsdk.product_version()) {
                            winsdk_um_lib_path =
                                Some(winsdk_lib_path.join(dir_name).join("um").join(winsdk_arch));
                            break;
                        }
                    }
                }
            }
        }
    }
    if let Some(winsdk_um_lib_path) = &winsdk_um_lib_path {
        config.define("PCSC_LIBRARIES", winsdk_um_lib_path.join("winscard.lib"));
    } else {
        panic!("Unable to find Windows SDK library path. Please ensure the appropriate version of the Windows SDK for your target platform is installed with the correct feature set.");
    }
}

#[cfg(all(target_os = "windows", feature = "vendored"))]
fn set_platform_specific_config(
    config: &mut cmake::Config,
    usb01_include_dir: &PathBuf,
    _usb1_include_dir: &PathBuf,
) {
    config.define(
        "LIBUSB_LIBRARIES",
        &usb01_include_dir.parent().unwrap().join("usb.lib"),
    );
    if cfg!(feature = "driver_pcsc") {
        set_pcsc_config_windows(config);
    }
}

#[cfg(all(not(target_os = "windows"), feature = "vendored"))]
fn set_unix_like_libusb_config(
    config: &mut cmake::Config,
    usb01_include_dir: &PathBuf,
    usb1_include_dir: &PathBuf,
) {
    let usb01_lib = usb01_include_dir
        .parent()
        .unwrap()
        .join("libusb.a")
        .into_os_string()
        .into_string()
        .unwrap();
    let usb1_lib = usb1_include_dir
        .parent()
        .unwrap()
        .join("libusb.a")
        .into_os_string()
        .into_string()
        .unwrap();
    config.define("LIBUSB_LIBRARIES", usb01_lib + ";" + &usb1_lib);
    config.define("LIBUSB_FOUND", "TRUE");
}

#[cfg(all(target_os = "macos", feature = "vendored"))]
fn set_platform_specific_config(
    config: &mut cmake::Config,
    usb01_include_dir: &PathBuf,
    usb1_include_dir: &PathBuf,
) {
    set_unix_like_libusb_config(config, usb01_include_dir, usb1_include_dir);
    config.define(
        "CMAKE_SHARED_LINKER_FLAGS",
        "-lobjc -framework IOKit -framework CoreFoundation",
    );
}

#[cfg(all(
    not(target_os = "windows"),
    not(target_os = "macos"),
    feature = "vendored"
))]
fn set_platform_specific_config(
    config: &mut cmake::Config,
    usb01_include_dir: &PathBuf,
    usb1_include_dir: &PathBuf,
) {
    set_unix_like_libusb_config(config, usb01_include_dir, usb1_include_dir);
}

#[cfg(feature = "vendored")]
fn make_source(nfc_dir: &PathBuf, out_dir: &PathBuf) -> Package {
    let usb01_include_dir = PathBuf::from(
        var("DEP_USB_0.1_INCLUDE").expect("usb-compat-01-sys did not export DEP_USB_0.1_INCLUDE"),
    );
    let usb1_include_dir = PathBuf::from(
        var("DEP_USB_1.0_INCLUDE").expect("libusb1-sys did not export DEP_USB_1.0_INCLUDE"),
    );
    let include_dir = out_dir.join("include");
    let build_dir = out_dir.join("build").join("libnfc");

    // Build libnfc and link against it
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut config = cmake::Config::new(&nfc_dir);
    config.define(
        "DLLTOOL",
        &var("DLLTOOL").unwrap_or(String::from("dlltool")),
    );
    config.define("LIBUSB_INCLUDE_DIRS", &usb01_include_dir);
    config.define("BUILD_UTILS", "OFF");
    config.define("BUILD_EXAMPLES", "OFF");
    config.define("BUILD_SHARED_LIBS", "OFF");
    config.define("CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG", &build_dir);
    config.define("CMAKE_ARCHIVE_OUTPUT_DIRECTORY_DEBUG", &build_dir);
    config.define("CMAKE_LIBRARY_OUTPUT_DIRECTORY_DEBUG", &build_dir);
    config.define("CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE", &build_dir);
    config.define("CMAKE_ARCHIVE_OUTPUT_DIRECTORY_RELEASE", &build_dir);
    config.define("CMAKE_LIBRARY_OUTPUT_DIRECTORY_RELEASE", &build_dir);
    config.define("LIBNFC_LOG", on_by_feature!("logging"));
    config.define("LIBNFC_CONFFILES_MODE", on_by_feature!("conffiles"));
    config.define("LIBNFC_ENVVARS", on_by_feature!("envvars"));
    config.define("LIBNFC_DRIVER_PCSC", on_by_feature!("driver_pcsc"));
    config.define(
        "LIBNFC_DRIVER_ACR122_PCSC",
        on_by_feature!("driver_acr122_pcsc"),
    );
    config.define(
        "LIBNFC_DRIVER_ACR122_USB",
        on_by_feature!("driver_acr122_usb"),
    );
    config.define("LIBNFC_DRIVER_ACR122S", on_by_feature!("driver_acr122s"));
    config.define("LIBNFC_DRIVER_ARYGON", on_by_feature!("driver_arygon"));
    config.define(
        "LIBNFC_DRIVER_PN532_UART",
        on_by_feature!("driver_pn532_uart"),
    );
    config.define(
        "LIBNFC_DRIVER_PN53X_USB",
        on_by_feature!("driver_pn53x_usb"),
    );
    config.out_dir(&out_dir);
    set_platform_specific_config(&mut config, &usb01_include_dir, &usb1_include_dir);
    config.build();

    // Output metainfo
    println!("cargo:vendored=1");
    println!("cargo:static=1");
    println!("cargo:include={}", include_dir.display());
    println!("cargo:version_number={}", VERSION);
    println!("cargo:rustc-link-lib=static=nfc");
    println!("cargo:rustc-link-search=native={}", build_dir.display());
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=IOKit");
        println!("cargo:rustc-link-lib=objc");
    }
    if cfg!(target_family = "unix") && pkg_config::probe_library("libudev").is_ok() {
        println!("cargo:rustc-link-lib=udev");
    }

    Package {
        include_paths: vec![include_dir],
    }
}

struct Package {
    include_paths: Vec<PathBuf>,
}

#[cfg(all(target_env = "msvc", not(feature = "vendored")))]
fn find_libnfc_pkg(_is_static: bool) -> Option<Package> {
    match vcpkg::Config::new().find_package("libnfc") {
        Ok(l) => Some(Package {
            include_paths: l.include_paths,
        }),
        Err(e) => {
            println!("Can't find libnfc pkg: {:?}", e);
            None
        }
    }
}

#[cfg(all(not(target_env = "msvc"), not(feature = "vendored")))]
fn find_libnfc_pkg(is_static: bool) -> Option<Package> {
    match pkg_config::Config::new().statik(is_static).probe("libnfc") {
        Ok(l) => {
            for lib in l.libs {
                if is_static {
                    println!("cargo:rustc-link-lib=static={}", lib);
                }
            }
            // Provide metadata and include directory for dependencies
            if is_static {
                println!("cargo:static=1");
            }
            l.include_paths.iter().for_each(|path| {
                println!("cargo:include={}", path.to_str().unwrap());
            });
            println!("cargo:version_number={}", l.version);

            Some(Package {
                include_paths: l.include_paths,
            })
        }
        Err(e) => {
            println!("Can't find libnfc pkg: {:?}", e);
            None
        }
    }
}

fn main() {
    let vendor_dir =
        PathBuf::from(var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR var not set"))
            .join("vendor");
    let out_dir = PathBuf::from(var("OUT_DIR").expect("OUT_DIR var not set"));
    let nfc_dir = vendor_dir.join("nfc");

    #[cfg(feature = "vendored")]
    let libnfc_pkg = make_source(&nfc_dir, &out_dir);

    #[cfg(not(feature = "vendored"))]
    let libnfc_pkg =
        find_libnfc_pkg(cfg!(target_feature = "crt-static")).expect("libnfc not found.");

    let mut bindings = Builder::default();
    for path in libnfc_pkg.include_paths {
        bindings = bindings.clang_arg(format!("-I{}", path.display()));

        let header_path = path.join("nfc").join("nfc.h");
        if header_path.exists() {
            bindings = bindings.header(header_path.to_str().unwrap())
        }
    }

    // Generate libnfc bindings
    bindings = bindings
        .allowlist_function("iso14443[ab]_.*")
        .allowlist_function("str_nfc_.*")
        .allowlist_function("nfc_.*")
        .allowlist_type("nfc_.*")
        .allowlist_var("NFC_.*");

    if cfg!(feature = "drivers") {
        let libnfc_dir = nfc_dir.join("libnfc");

        bindings = bindings
            .clang_arg(format!("-I{}", libnfc_dir.display()))
            .header(libnfc_dir.join("iso7816.h").to_str().unwrap())
            .allowlist_var("ISO7816_.*")
            .header(
                libnfc_dir
                    .join("drivers")
                    .join("acr122_pcsc.h")
                    .to_str()
                    .unwrap(),
            )
            .header(
                libnfc_dir
                    .join("drivers")
                    .join("acr122_usb.h")
                    .to_str()
                    .unwrap(),
            )
            .header(
                libnfc_dir
                    .join("drivers")
                    .join("acr122s.h")
                    .to_str()
                    .unwrap(),
            )
            .allowlist_function("acr122s?_.*")
            .allowlist_type("acr122s?_.*")
            .allowlist_var("ACR122S?_.*")
            .header(
                libnfc_dir
                    .join("drivers")
                    .join("arygon.h")
                    .to_str()
                    .unwrap(),
            )
            .allowlist_function("arygon_.*")
            .allowlist_type("arygon_.*")
            .allowlist_var("ARYGON_.*")
            .header(libnfc_dir.join("chips").join("pn53x.h").to_str().unwrap())
            .header(
                libnfc_dir
                    .join("drivers")
                    .join("pn53x_usb.h")
                    .to_str()
                    .unwrap(),
            )
            .header(
                libnfc_dir
                    .join("drivers")
                    .join("pn532_i2c.h")
                    .to_str()
                    .unwrap(),
            )
            .header(
                libnfc_dir
                    .join("drivers")
                    .join("pn532_spi.h")
                    .to_str()
                    .unwrap(),
            )
            .header(
                libnfc_dir
                    .join("drivers")
                    .join("pn532_uart.h")
                    .to_str()
                    .unwrap(),
            )
            .header(
                libnfc_dir
                    .join("drivers")
                    .join("pn71xx.h")
                    .to_str()
                    .unwrap(),
            )
            .allowlist_function("pn(53[x23]|71xx)_.*")
            .allowlist_type("pn(53[x23]|71xx)_.*")
            .allowlist_var("PN(53[Xx23]|71XX)_.*")
            .header(libnfc_dir.join("drivers").join("pcsc.h").to_str().unwrap())
            .allowlist_function("pcsc_.*")
            .allowlist_type("pcsc_.*")
            .allowlist_var("PCSC_.*");
    }

    bindings
        .generate()
        .expect("Unable to generate nfc bindings")
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Unable to write nfc bindings");
}
