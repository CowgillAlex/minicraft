use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numbers = version.split('.').take(3).collect::<Vec<_>>().join(",");
    let manifest = fs::read_to_string(root.join("assets/app.manifest"))
        .unwrap()
        .replace("@VERSION@", &format!("{version}.0"));
    fs::write(out.join("app.manifest"), manifest).unwrap();
    let resource = format!(
        r#"
1 ICON "{}"
1 24 "{}"
1 VERSIONINFO
FILEVERSION {},0
PRODUCTVERSION {},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904b0"
  BEGIN
   VALUE "FileDescription", "Cedar Voxel Lab"
   VALUE "FileVersion", "{}"
   VALUE "ProductName", "Cedar Voxel Lab"
   VALUE "ProductVersion", "{}"
   VALUE "OriginalFilename", "voxel-drive.exe"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x409, 1200
 END
END
"#,
        root.join("assets/app.ico").display(),
        out.join("app.manifest").display(),
        numbers,
        numbers,
        version,
        version
    );
    let rc = out.join("version.rc");
    let object = out.join("version.o");
    fs::write(&rc, resource).unwrap();
    let status = Command::new("x86_64-w64-mingw32-windres")
        .arg(&rc)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&object)
        .status()
        .expect("Install MinGW windres to embed the Windows version and icon");
    assert!(status.success(), "Windows resource compilation failed");
    println!("cargo:rustc-link-arg={}", object.display());
}
