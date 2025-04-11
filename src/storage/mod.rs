use image::DynamicImage;

pub mod local_storage;
pub mod s3_storage;

pub trait FileStorage {
    fn save_file(
        &self,
        url: &str,
        filename: &str,
    ) -> impl std::future::Future<Output = Result<String, anyhow::Error>>;
    fn load_file(
        &self,
        url: &str,
    ) -> impl std::future::Future<Output = Result<DynamicImage, anyhow::Error>>;
    fn remove_file(
        &self,
        url: &str,
    ) -> impl std::future::Future<Output = Result<(), anyhow::Error>>;
}
