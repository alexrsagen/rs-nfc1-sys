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

	// Build libnfc and link against it
	fs::create_dir_all(&out_dir).unwrap();
	let nfc_dir = vendor_dir.join("nfc");
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
		.header(include_dir.join("nfc").join("nfc.h").to_str().unwrap())
		.clang_arg(format!("-I{}", include_dir.display()))
		.allowlist_function("nfc_.*")
		.allowlist_type("nfc_.*")
		.allowlist_var("NFC_.*")
		.generate()
		.expect("Unable to generate nfc bindings");

	bindings
		.write_to_file(out_dir.join("bindings.rs"))
		.expect("Unable to write nfc bindings");
}
