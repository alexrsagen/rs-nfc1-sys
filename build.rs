extern crate cc;
extern crate cmake;
extern crate bindgen;

use bindgen::Builder;
use std::{env, fs};
use std::path::PathBuf;

static VERSION: &'static str = "1.8.0";

fn main() {
	let usb01_include_dir = PathBuf::from(env::var("DEP_USB_0.1_INCLUDE").expect("usb-compat-01-sys did not export DEP_USB_0.1_INCLUDE"));
	let vendor_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR var not set")).join("vendor");
	let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR var not set"));
	let build_dir = out_dir.join("build").join("libnfc");
	let include_dir = out_dir.join("include");
	let nfc_dir = vendor_dir.join("nfc");
	let libnfc_dir = nfc_dir.join("libnfc");

	// Build libnfc and link against it
	fs::create_dir_all(&out_dir).unwrap();
	cmake::Config::new(&nfc_dir)
		.define("DLLTOOL", &env::var("DLLTOOL").unwrap_or(String::from("dlltool")))
		.define("LIBUSB_INCLUDE_DIRS", &usb01_include_dir)
		.define("LIBUSB_LIBRARIES", &usb01_include_dir.parent().unwrap().join("usb.lib"))
		.define("BUILD_UTILS", "OFF")
		.define("BUILD_EXAMPLES", "OFF")
		.define("BUILD_SHARED_LIBS", "OFF")
		.define("CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG", &build_dir)
		.define("CMAKE_ARCHIVE_OUTPUT_DIRECTORY_DEBUG", &build_dir)
		.define("CMAKE_LIBRARY_OUTPUT_DIRECTORY_DEBUG", &build_dir)
		.define("CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE", &build_dir)
		.define("CMAKE_ARCHIVE_OUTPUT_DIRECTORY_RELEASE", &build_dir)
		.define("CMAKE_LIBRARY_OUTPUT_DIRECTORY_RELEASE", &build_dir)
		.out_dir(&out_dir)
		.build();

	// Output metainfo
	println!("cargo:vendored=1");
	println!("cargo:static=1");
	println!("cargo:include={}", include_dir.display());
	println!("cargo:version_number={}", VERSION);
	println!("cargo:rustc-link-lib=static=nfc");
	println!("cargo:rustc-link-lib=dylib=nfc");
	println!("cargo:rustc-link-search=native={}", build_dir.display());

	// Generate libnfc bindings
	let bindings = Builder::default()
		.clang_arg(format!("-I{}", include_dir.display()))
		.clang_arg(format!("-I{}", libnfc_dir.display()))
		.header(include_dir.join("nfc").join("nfc.h").to_str().unwrap())
		.allowlist_function("iso14443a_.*")
		.allowlist_function("nfc_.*")
		.allowlist_type("nfc_.*")
		.allowlist_var("NFC_.*")
		.header(libnfc_dir.join("iso7816.h").to_str().unwrap())
		.allowlist_var("ISO7816_.*")
		.header(libnfc_dir.join("drivers").join("acr122_pcsc.h").to_str().unwrap())
		.header(libnfc_dir.join("drivers").join("acr122_usb.h").to_str().unwrap())
		.header(libnfc_dir.join("drivers").join("acr122s.h").to_str().unwrap())
		.allowlist_function("acr122s?_.*")
		.allowlist_type("acr122s?_.*")
		.allowlist_var("ACR122S?_.*")
		.header(libnfc_dir.join("drivers").join("arygon.h").to_str().unwrap())
		.allowlist_function("arygon_.*")
		.allowlist_type("arygon_.*")
		.allowlist_var("ARYGON_.*")
		.header(libnfc_dir.join("chips").join("pn53x.h").to_str().unwrap())
		.header(libnfc_dir.join("drivers").join("pn53x_usb.h").to_str().unwrap())
		.header(libnfc_dir.join("drivers").join("pn532_i2c.h").to_str().unwrap())
		.header(libnfc_dir.join("drivers").join("pn532_spi.h").to_str().unwrap())
		.header(libnfc_dir.join("drivers").join("pn532_uart.h").to_str().unwrap())
		.header(libnfc_dir.join("drivers").join("pn71xx.h").to_str().unwrap())
		.allowlist_function("pn(53[x23]|71xx)_.*")
		.allowlist_type("pn(53[x23]|71xx)_.*")
		.allowlist_var("PN(53[Xx23]|71XX)_.*")
		.header(libnfc_dir.join("drivers").join("pcsc.h").to_str().unwrap())
		.allowlist_function("pcsc_.*")
		.allowlist_type("pcsc_.*")
		.allowlist_var("PCSC_.*")
		.generate()
		.expect("Unable to generate nfc bindings");

	bindings
		.write_to_file(out_dir.join("bindings.rs"))
		.expect("Unable to write nfc bindings");
}
