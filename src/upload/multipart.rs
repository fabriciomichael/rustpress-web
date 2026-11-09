use std::path::PathBuf;

use bytes::{Bytes, BytesMut};
use multer::{Constraints, Multipart, SizeLimit};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::{UploadConfig, UploadError, UploadedFile};

/// Resultado do processamento de um pedido multipart.
#[derive(Debug, Default)]
pub(crate) struct MultipartData {
    pub(crate) fields: Vec<(String, String)>,
    pub(crate) files: Vec<UploadedFile>,
}

/// Remove ficheiros temporários ou definitivos caso o processamento falhe.
struct UploadCleanup {
    paths: Vec<PathBuf>,
}

impl UploadCleanup {
    fn new() -> Self {
        Self { paths: Vec::new() }
    }

    fn track(&mut self, path: PathBuf) {
        self.paths.push(path);
    }

    fn replace(&mut self, old_path: &PathBuf, new_path: PathBuf) {
        if let Some(path) = self.paths.iter_mut().find(|path| *path == old_path) {
            *path = new_path;
        }
    }

    fn commit(mut self) {
        self.paths.clear();
    }
}

impl Drop for UploadCleanup {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Processa um corpo `multipart/form-data`.
pub(crate) async fn process(
    body: Bytes,
    boundary: &str,
    config: &UploadConfig,
) -> Result<MultipartData, UploadError> {
    config.validate()?;

    fs::create_dir_all(config.storage_dir())
        .await
        .map_err(|error| UploadError::Storage(error.to_string()))?;

    let constraints = Constraints::new().size_limit(
        SizeLimit::new()
            .whole_stream(config.max_total_size())
            .per_field(config.max_total_size()),
    );

    let stream =
        futures_util::stream::once(async move { Ok::<Bytes, std::convert::Infallible>(body) });

    let mut multipart = Multipart::with_constraints(stream, boundary, constraints);

    let mut result = MultipartData::default();
    let mut cleanup = UploadCleanup::new();

    while let Some((_index, field)) = multipart
        .next_field_with_idx()
        .await
        .map_err(|error| UploadError::Multipart(error.to_string()))?
    {
        let name = field.name().map(str::to_owned);

        if let Some(file_name) = field.file_name().map(str::to_owned) {
            if result.files.len() >= config.max_files() {
                return Err(UploadError::TooManyFiles {
                    limit: config.max_files(),
                });
            }

            let stored_name = format!("{}", Uuid::new_v4());
            let final_path = config.storage_dir().join(&stored_name);
            let part_path = temporary_path(&final_path);

            let content_type = field.content_type().map(ToString::to_string);

            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&part_path)
                .await
                .map_err(|error| UploadError::Storage(error.to_string()))?;

            cleanup.track(part_path.clone());

            let mut size = 0_u64;

            let write_result = async {
                let mut field = field;

                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|error| UploadError::Multipart(error.to_string()))?
                {
                    size = size.checked_add(chunk.len() as u64).ok_or(
                        UploadError::FileSizeExceeded {
                            limit: config.max_file_size(),
                        },
                    )?;

                    if size > config.max_file_size() {
                        return Err(UploadError::FileSizeExceeded {
                            limit: config.max_file_size(),
                        });
                    }

                    file.write_all(&chunk)
                        .await
                        .map_err(|error| UploadError::Storage(error.to_string()))?;
                }

                file.flush()
                    .await
                    .map_err(|error| UploadError::Storage(error.to_string()))?;

                Ok::<(), UploadError>(())
            }
            .await;

            if let Err(error) = write_result {
                let _ = fs::remove_file(&part_path).await;
                return Err(error);
            }

            drop(file);

            if let Err(error) = fs::rename(&part_path, &final_path).await {
                let _ = fs::remove_file(&part_path).await;
                return Err(UploadError::Storage(error.to_string()));
            }

            cleanup.replace(&part_path, final_path.clone());

            result.files.push(UploadedFile::new(
                name.unwrap_or_default(),
                Some(file_name),
                stored_name,
                final_path,
                content_type,
                size,
            ));
        } else {
            if result.fields.len() >= config.max_fields() {
                return Err(UploadError::TooManyFields {
                    limit: config.max_fields(),
                });
            }

            let field_name = name.unwrap_or_default();

            let mut value_buffer = BytesMut::new();
            let mut value_size = 0_u64;

            let mut field = field;

            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|error| UploadError::Multipart(error.to_string()))?
            {
                value_size = value_size.checked_add(chunk.len() as u64).ok_or(
                    UploadError::FieldSizeExceeded {
                        limit: config.max_field_size(),
                    },
                )?;

                if value_size > config.max_field_size() {
                    return Err(UploadError::FieldSizeExceeded {
                        limit: config.max_field_size(),
                    });
                }

                value_buffer.extend_from_slice(&chunk);
            }

            let value = String::from_utf8(value_buffer.to_vec())
                .map_err(|error| UploadError::Multipart(error.to_string()))?;

            result.fields.push((field_name, value));
        }
    }

    cleanup.commit();

    Ok(result)
}

fn temporary_path(final_path: &std::path::Path) -> PathBuf {
    let file_name = final_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("upload");

    final_path.with_file_name(format!("{file_name}.part"))
}

#[cfg(test)]
mod tests {
    use super::{process, temporary_path};
    use crate::upload::UploadConfig;
    use bytes::Bytes;
    use std::path::Path;

    #[test]
    fn builds_temporary_path() {
        let path = Path::new("storage/uploads/test");
        assert_eq!(temporary_path(path), Path::new("storage/uploads/test.part"));
    }

    #[tokio::test]
    async fn processes_text_field() {
        let boundary = "TEST-BOUNDARY";
        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"username\"\r\n\
             \r\n\
             fabricio\r\n\
             --{boundary}--\r\n"
        );

        let dir =
            std::env::temp_dir().join(format!("rustpress-upload-test-{}", uuid::Uuid::new_v4()));

        let config = UploadConfig::new(&dir)
            .with_max_total_size(1024 * 1024)
            .with_max_file_size(512 * 1024)
            .with_max_field_size(1024);

        let result = process(Bytes::from(body), boundary, &config)
            .await
            .expect("multipart should be processed");

        assert_eq!(
            result.fields,
            vec![("username".to_owned(), "fabricio".to_owned())]
        );
        assert!(result.files.is_empty());

        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn rejects_text_field_exceeding_size_limit() {
        let boundary = "TEST-BOUNDARY";
        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"description\"\r\n\
             \r\n\
             1234567890\r\n\
             --{boundary}--\r\n"
        );

        let dir =
            std::env::temp_dir().join(format!("rustpress-upload-test-{}", uuid::Uuid::new_v4()));

        let config = UploadConfig::new(&dir)
            .with_max_total_size(1024)
            .with_max_file_size(512)
            .with_max_field_size(5);

        let result = process(Bytes::from(body), boundary, &config).await;

        assert!(matches!(
            result,
            Err(crate::upload::UploadError::FieldSizeExceeded { limit: 5 })
        ));

        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn processes_file_upload() {
        let boundary = "TEST-BOUNDARY";
        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"avatar\"; filename=\"avatar.txt\"\r\n\
             Content-Type: text/plain\r\n\
             \r\n\
             conteudo-do-ficheiro\r\n\
             --{boundary}--\r\n"
        );

        let dir =
            std::env::temp_dir().join(format!("rustpress-upload-test-{}", uuid::Uuid::new_v4()));

        let config = UploadConfig::new(&dir)
            .with_max_total_size(1024 * 1024)
            .with_max_file_size(512 * 1024);

        let result = process(Bytes::from(body), boundary, &config)
            .await
            .expect("multipart file should be processed");

        assert!(result.fields.is_empty());
        assert_eq!(result.files.len(), 1);

        let uploaded = &result.files[0];

        assert_eq!(uploaded.original_name(), Some("avatar.txt"));
        assert_eq!(uploaded.content_type(), Some("text/plain"));
        assert_eq!(uploaded.size(), 20);
        assert!(uploaded.path().exists());

        let content = tokio::fs::read(uploaded.path())
            .await
            .expect("uploaded file should exist");

        assert_eq!(content, b"conteudo-do-ficheiro");

        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn rejects_file_exceeding_size_limit() {
        let boundary = "TEST-BOUNDARY";
        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"document\"; filename=\"document.txt\"\r\n\
             Content-Type: text/plain\r\n\
             \r\n\
             123456\r\n\
             --{boundary}--\r\n"
        );

        let dir =
            std::env::temp_dir().join(format!("rustpress-upload-test-{}", uuid::Uuid::new_v4()));

        let config = UploadConfig::new(&dir)
            .with_max_total_size(1024)
            .with_max_file_size(5)
            .with_max_field_size(512);

        let result = process(Bytes::from(body), boundary, &config).await;

        assert!(matches!(
            result,
            Err(crate::upload::UploadError::FileSizeExceeded { limit: 5 })
        ));

        let mut entries = tokio::fs::read_dir(&dir)
            .await
            .expect("upload directory should exist");

        assert!(
            entries.next_entry().await.unwrap().is_none(),
            "temporary files should be removed after failure"
        );

        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn removes_previous_files_when_later_file_fails() {
        let boundary = "TEST-BOUNDARY";
        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"first\"; filename=\"first.txt\"\r\n\
             Content-Type: text/plain\r\n\
             \r\n\
             ok\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"second\"; filename=\"second.txt\"\r\n\
             Content-Type: text/plain\r\n\
             \r\n\
             123456\r\n\
             --{boundary}--\r\n"
        );

        let dir =
            std::env::temp_dir().join(format!("rustpress-upload-test-{}", uuid::Uuid::new_v4()));

        let config = UploadConfig::new(&dir)
            .with_max_total_size(1024)
            .with_max_file_size(5)
            .with_max_field_size(512);

        let result = process(Bytes::from(body), boundary, &config).await;

        assert!(matches!(
            result,
            Err(crate::upload::UploadError::FileSizeExceeded { limit: 5 })
        ));

        let mut entries = tokio::fs::read_dir(&dir)
            .await
            .expect("upload directory should exist");

        assert!(
            entries.next_entry().await.unwrap().is_none(),
            "all files from a failed request should be removed"
        );

        let _ = tokio::fs::remove_dir_all(dir).await;
    }
}
