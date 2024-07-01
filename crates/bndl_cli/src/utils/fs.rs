use log::debug;
use std::{fs, io, path::PathBuf};

pub fn copy_dir_all(src: &PathBuf, dst: &PathBuf, excl: Option<&Vec<String>>) -> io::Result<()> {
    'outer: for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if file_type.is_symlink() {
            continue;
        }

        if let Some(excl) = excl {
            for ex in excl.iter() {
                if entry.path().to_string_lossy().contains(ex) {
                    debug!("Ignoring {:?} while copying to {:?}", entry.path(), dst);

                    continue 'outer;
                }
            }
        }

        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()), excl)?;
        } else {
            if !dst.exists() {
                fs::create_dir_all(&dst)?;
            }

            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }

    Ok(())
}
