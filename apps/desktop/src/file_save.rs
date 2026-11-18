use std::{io::Write, path::Path};

pub(crate) fn save_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    save_file_with(path, |file| file.write_all(bytes))
}

pub(crate) fn save_file_with(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<(), String> {
    let parent = path.parent().ok_or("保存路径无效")?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    write(file.as_file_mut()).map_err(|error| error.to_string())?;
    file.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_stream_preserves_the_existing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("conversation.md");
        save_file(&path, b"original").unwrap();
        let result = save_file_with(&path, |file| {
            file.write_all(b"partial export")?;
            Err(std::io::Error::other("simulated write failure"))
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
