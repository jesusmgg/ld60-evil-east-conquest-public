use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=template/*");
    println!("cargo:rerun-if-changed=assets/*");

    let out_dir = PathBuf::from(format! {"{}/../../../", env::var("OUT_DIR").unwrap()});
    let out_dir_path = out_dir.as_path();

    println!("This is the out dir: {:?}", &out_dir.as_os_str());

    copy_dir_all(Path::new("template"), &out_dir_path)?;
    copy_dir_all(
        Path::new("assets"),
        PathBuf::from(out_dir).join("assets").as_path(),
    )?;

    Ok(())
}
