use futures::TryStreamExt;
use image::DynamicImage;
use s3::{creds::Credentials, error::S3Error, Bucket, Region};
use url::Url;

pub trait FileStorage {
    async fn save_file(&self, url: &str, filename: &str) -> Result<String, anyhow::Error>;
    async fn load_file(&self, url: &str) -> Result<DynamicImage, anyhow::Error>;
    async fn remove_file(&self, url: &str) -> Result<(), anyhow::Error>;
}

/*
pub struct LocalFileStorage {}

impl FileStorage for LocalFileStorage {
    fn save_file() {
        todo!()
    }

    fn load_file(url: &str) {

    }
}
*/

pub struct S3FileStorage {
    endpoint: String,
    access_key: String,
    secret_key: String,
    bucket_name: String,
}

fn get_bucket(
    endpoint: &str,
    bucket_name: &str,
    access_key: &str,
    secret_key: &str,
) -> Result<Box<Bucket>, S3Error> {
    let region = Region::Custom {
        region: "eu-central-1".to_owned(),
        endpoint: endpoint.to_owned(),
    };

    let credentials = Credentials::new(
        // Some("GxETY0nixTfpRyXvpsdJ"),
        // Some("CnjH4H6hEFIqsDBTyis0x9WFA6N9y7UPEMHJZBuQ"),
        Some(access_key),
        Some(secret_key),
        None,
        None,
        None,
    )?;

    let bucket = Bucket::new(bucket_name, region, credentials)?.with_path_style();
    Ok(bucket)
}

impl S3FileStorage {
    pub fn new(s3_endpoint: &str, bucket_name: &str, access_key: &str, secret_key: &str) -> Self {
        //"http://localhost:9000"
        Self {
            access_key: access_key.to_owned(),
            secret_key: secret_key.to_owned(),
            endpoint: s3_endpoint.to_owned(),
            bucket_name: bucket_name.to_owned(),
        }
    }
}

impl FileStorage for S3FileStorage {
    #[tracing::instrument(
        skip(self)
    )]
    async fn save_file(&self, url: &str, filename: &str) -> Result<String, anyhow::Error> {
        let bucket = get_bucket(
            &self.endpoint,
            &self.bucket_name,
            &self.access_key,
            &self.secret_key,
        )
        .map_err(|e| anyhow::format_err!("Failed to open bucket: {}", e))?;

        let _ = upload_url_to_bucket(bucket, url, filename).await?;
        //TODO: return s3 uri
        //TODO: extract hostname as endpoint
        Ok(format!("s3://{}/{}", &self.bucket_name, filename))
    }

    async fn load_file(&self, url: &str) -> Result<DynamicImage, anyhow::Error> {
        //TODO: parse uri
        let s3_url = Url::parse(url)?;

        if s3_url.scheme() != "s3" {
            return Err(anyhow::format_err!("This is not s3 url"));
        }

        let bucket_name = s3_url
            .host_str()
            .ok_or(anyhow::format_err!("Failed to parse bucket"))?; //bucket
        let filename = s3_url.path(); // filename

        let bucket = get_bucket(
            &self.endpoint,
            bucket_name,
            &self.access_key,
            &self.secret_key,
        )
        .map_err(|e| anyhow::format_err!("Failed to open bucket: {}", e))?;

        let bucket_result = bucket
            .get_object(filename)
            .await
            .map_err(|e| anyhow::format_err!("Failed to read file from bucket: {}", e))?;

        let image_result = image::load_from_memory(bucket_result.bytes())
            .map_err(|e| anyhow::format_err!("Failed to load image: {}", e))?;
        //TODO: return image
        Ok(image_result)
    }


    async fn remove_file(&self, url: &str) -> Result<(), anyhow::Error> {
        let s3_url = Url::parse(url)?;

        if s3_url.scheme() != "s3" {
            return Err(anyhow::format_err!("This is not s3 url"));
        }

        let bucket_name = s3_url
            .host_str()
            .ok_or(anyhow::format_err!("Failed to parse bucket"))?; //bucket
        let filename = s3_url.path(); // filename

        let bucket = get_bucket(
            &self.endpoint,
            bucket_name,
            &self.access_key,
            &self.secret_key,
        )
        .map_err(|e| anyhow::format_err!("Failed to open bucket: {}", e))?;

        bucket.delete_object(filename).await.map_err(|e| anyhow::format_err!("Failed to remove file: {}", e))?;

        Ok(())
    }
}

async fn upload_url_to_bucket(
    bucket: Box<Bucket>,
    url: &str,
    filename: &str,
) -> Result<s3::utils::PutStreamResponse, anyhow::Error> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| anyhow::format_err!("Failed to download url: {}", e))?;
    let stream = response.bytes_stream().map_err(std::io::Error::other);
    let mut stream = tokio_util::io::StreamReader::new(stream);

    let result = bucket
        .put_object_stream(&mut stream, filename)
        .await
        .map_err(|e| anyhow::format_err!("Failed to upload file: {}", e))?;
    Ok(result)
}
