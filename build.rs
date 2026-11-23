use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=resources/surround-tag.rc");
    println!("cargo:rerun-if-changed=resources/surround-tag.ico");
    println!("cargo:rerun-if-changed=resources/surround-tag.manifest");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let icon = Path::new("resources/surround-tag.ico");
    let manifest = Path::new("resources/surround-tag.manifest");
    let template = Path::new("resources/surround-tag.rc");

    fs::copy(icon, out_dir.join("surround-tag.ico"))
        .expect("could not copy the Windows icon into OUT_DIR");
    fs::copy(manifest, out_dir.join("surround-tag.manifest"))
        .expect("could not copy the Windows manifest into OUT_DIR");

    // The resource compiler runs against a preprocessed copy in OUT_DIR, so
    // icon/manifest references must be relative to that directory.
    let resource = fs::read_to_string(template)
        .expect("could not read resources/surround-tag.rc")
        .replace("resources/surround-tag.ico", "surround-tag.ico")
        .replace("resources/surround-tag.manifest", "surround-tag.manifest");
    let resource_path = out_dir.join("surround-tag.rc");
    fs::write(&resource_path, resource).expect("could not write the generated resource file");

    match embed_resource::compile(&resource_path, embed_resource::NONE) {
        embed_resource::CompilationResult::Failed(error) => {
            panic!("Windows resource compilation failed: {error}");
        }
        embed_resource::CompilationResult::NotAttempted(error) => {
            println!("cargo:warning=Windows resources were not embedded: {error}");
        }
        embed_resource::CompilationResult::NotWindows | embed_resource::CompilationResult::Ok => {}
    }
}
