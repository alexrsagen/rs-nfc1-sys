extern crate cc;
extern crate cmake;
extern crate bindgen;

use bindgen::Builder;
use std::{env, fs};
use std::path::PathBuf;

static VERSION: &'static str = "1.8.0";

macro_rules! on_by_feature {
	($f: literal) => {
		if cfg!(feature = $f) {
			"ON"
		} else {
			"OFF"
		}
	}
}

fn make_source(nfc_dir: &PathBuf, out_dir: &PathBuf) -> Package {
	let usb01_include_dir = PathBuf::from(env::var("DEP_USB_0.1_INCLUDE").expect("usb-compat-01-sys did not export DEP_USB_0.1_INCLUDE"));
	let usb1_include_dir = PathBuf::from(env::var("DEP_USB_1.0_INCLUDE").expect("libusb1-sys did not export DEP_USB_1.0_INCLUDE"));
	let include_dir = out_dir.join("include");
	let build_dir = out_dir.join("build").join("libnfc");

	// Build libnfc and link against it
	fs::create_dir_all(&out_dir).unwrap();
	let mut config = cmake::Config::new(&nfc_dir);
	config.define("DLLTOOL", &env::var("DLLTOOL").unwrap_or(String::from("dlltool")));
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
	config.out_dir(&out_dir);

	if std::env::var("CARGO_CFG_TARGET_OS") == Ok("windows".into()) {
		config.define("LIBUSB_LIBRARIES", &usb01_include_dir.parent().unwrap().join("usb.lib"));
	} else {
		let usb01_lib = usb01_include_dir.parent().unwrap().join("libusb.a").into_os_string().into_string().unwrap();
		let usb1_lib = usb1_include_dir.parent().unwrap().join("libusb.a").into_os_string().into_string().unwrap();
		config.define("LIBUSB_LIBRARIES", usb01_lib + ";" + &usb1_lib);
		config.define("LIBUSB_FOUND", "TRUE");
	}

	config.build();

	// Output metainfo
	println!("cargo:vendored=1");
	println!("cargo:static=1");
	println!("cargo:include={}", include_dir.display());
	println!("cargo:version_number={}", VERSION);
	println!("cargo:rustc-link-lib=dylib=nfc");
	println!("cargo:rustc-link-search=native={}", build_dir.display());

	Package{
		include_paths: vec![include_dir],
	}
}

struct Package {
	include_paths: Vec<PathBuf>,
}

#[cfg(target_env = "msvc")]
fn find_libnfc_pkg(_statik: bool) -> Option<Package> {
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

#[cfg(not(target_env = "msvc"))]
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
	let vendor_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR var not set")).join("vendor");
	let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR var not set"));
	let nfc_dir = vendor_dir.join("nfc");
	let libnfc_pkg = if cfg!(feature = "vendored") {
		make_source(&nfc_dir, &out_dir)
	} else {
		let is_static = std::env::var("CARGO_CFG_TARGET_FEATURE")
			.map(|s| s.contains("crt-static"))
			.unwrap_or_default();

		find_libnfc_pkg(is_static)
			.expect("libnfc not found.")
	};

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
			.allowlist_var("PCSC_.*");
	}

	bindings
		.generate()
		.expect("Unable to generate nfc bindings")
		.write_to_file(out_dir.join("bindings.rs"))
		.expect("Unable to write nfc bindings");
}
