#[cfg(windows)]
extern crate winres;

use std::env;
use std::fs::{self, File};
use std::path::Path;

use toml;

mod cargo;

#[cfg(windows)]
fn platform_specific() {
	let mut res = winres::WindowsResource::new();
	res.set("FileDescription", "Emulsion");
	res.set_icon("resource_dev/emulsion.ico");
	res.compile().unwrap();
}

#[cfg(target_os = "macos")]
fn platform_specific() {
	if env::var_os("MAKE_INFO_PLIST").is_none() {
		return;
	}

	struct DocType {
		pub ext: &'static [&'static str],
		pub name: &'static str,
	}
	let doc_types = [
		//DocType { ext: &["jpg", "jpeg"], name: "JPEG Image" },
		DocType { ext: &["jpeg"], name: "JPEG Image" },
		DocType { ext: &["png"], name: "PNG Image" },
		DocType { ext: &["bmp"], name: "BMP Image" },
		DocType { ext: &["gif"], name: "GIF Image" },
		DocType { ext: &["tga"], name: "TGA Image" },
		DocType { ext: &["avif"], name: "AVIF Image" },
		DocType { ext: &["webp"], name: "WEBP Image" },
		DocType { ext: &["tif"], name: "TIF Image" },
		DocType { ext: &["tiff"], name: "TIFF Image" },
		DocType { ext: &["ico"], name: "ICO Image" },
		DocType { ext: &["hdr"], name: "HDR Image" },
		DocType { ext: &["pbm"], name: "PBM Image" },
		DocType { ext: &["pam"], name: "PAM Image" },
		DocType { ext: &["ppm"], name: "PPM Image" },
		DocType { ext: &["pgm"], name: "PGM Image" },
	];

	let mut doc_types_str = String::new();
	for doc_type in doc_types.iter() {
		doc_types_str.push_str("<dict>\n");
		doc_types_str.push_str("<key>CFBundleTypeName</key>\n");
		doc_types_str.push_str(&format!("<string>{}</string>\n", doc_type.name));
		doc_types_str.push_str("<key>LSItemContentTypes</key>\n");
		doc_types_str.push_str("<array>\n");
		for ext in doc_type.ext {
			doc_types_str.push_str(&format!("<string>public.{}</string>\n", ext));
		}
		doc_types_str.push_str("</array>\n");
		doc_types_str.push_str("<key>NSExportableTypes</key>\n");
		doc_types_str.push_str("<array/>\n");
		doc_types_str.push_str("<key>LSHandlerRank</key>\n");
		doc_types_str.push_str("<string>Alternate</string>\n");
		doc_types_str.push_str("<key>CFBundleTypeIconFiles</key>\n");
		doc_types_str.push_str("<array/>\n");
		doc_types_str.push_str("<key>CFBundleTypeRole</key>\n");
		doc_types_str.push_str("<string>Viewer</string>\n");
		doc_types_str.push_str("</dict>\n");
	}

	let cargo_str = fs::read_to_string("Cargo.toml").unwrap();
	let cargo_desc: cargo::CargoDescriptor = toml::from_str(cargo_str.as_str()).unwrap();

	let info_plist = format!(
		r#"
		<?xml version="1.0" encoding="UTF-8"?>
		<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
		<plist version="1.0">
		<dict>
			<key>CFBundleDevelopmentRegion</key>
			<string>English</string>
			<key>CFBundleInfoDictionaryVersion</key>
			<string>6.0</string>
			<key>CFBundleDocumentTypes</key>
			<array>
				{doc_types}
			</array>
			<key>CFBundleExecutable</key>
			<string>{exe_name}</string>
			<key>CFBundleIconFile</key>
			<string>Emulsion.icns</string>
			<key>CFBundleIdentifier</key>
			<string>io.github.arturkovacs.emulsion</string>
			<key>CFBundleName</key>
			<string>Emulsion</string>
			<key>CFBundlePackageType</key>
			<string>APPL</string>
			<key>CFBundleVersion</key>
			<string>{version}</string>
			<key>CSResourcesFileMapped</key>
			<true/>
			<key>LSRequiresCarbon</key>
			<true/>
			<key>NSHighResolutionCapable</key>
			<true/>
			<key>NSHumanReadableCopyright</key>
			<string>Copyright (c) 2020 The Emulsion Contributors</string>
		</dict>
		</plist>	
	"#,
		doc_types = doc_types_str,
		exe_name = cargo_desc.package.name,
		version = cargo_desc.package.version
	);

	fs::write("Info.plist", info_plist).unwrap();
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn platform_specific() {}

fn main() {
	platform_specific();

	let dir_name = "resource";
	let profile = env::var("PROFILE").unwrap();

	let target_resource_path = Path::new("target").join(profile).join(dir_name);
	fs::create_dir_all(target_resource_path.clone()).unwrap();

	for entry in fs::read_dir("resource/").unwrap() {
		let entry = entry.unwrap();
		if entry.file_type().unwrap().is_file() {
			fs::copy(entry.path(), target_resource_path.join(entry.file_name())).unwrap();
		}
	}
}
