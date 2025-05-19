use std::{ffi::OsStr, io::Cursor, path::PathBuf};

use frankenstein::{response::MethodResponse, types::File};
use reqwest::Response;

#[tracing::instrument(name = "Save file", skip(file_response))]
async fn save_file(
    file_path: &str,
    response: &MethodResponse<File>,
    file_response: Response,
) -> Result<PathBuf, ()> {
    let original_path = std::path::Path::new(file_path);
    let extension = original_path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("");

    let destination_path_str = format!(
        "./files/{path}.{extension}",
        path = response.result.file_unique_id,
        extension = extension
    );
    let destination_path = std::path::Path::new(&destination_path_str);
    let prefix = destination_path.parent().unwrap();
    std::fs::create_dir_all(prefix).unwrap();
    let mut file = std::fs::File::create(destination_path).map_err(|_| ())?;
    let mut content = Cursor::new(file_response.bytes().await.map_err(|_| ())?);
    std::io::copy(&mut content, &mut file).map_err(|_| ())?;
    Ok(destination_path.to_path_buf())
}
