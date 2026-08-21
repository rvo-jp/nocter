use std::io::Read;
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::CONTENT_LENGTH;

use crate::PackageAcquisitionError;
use crate::policy::{public_https_url, redirect_is_allowed};

pub(crate) const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct HttpsClient {
    client: Client,
}

impl std::fmt::Debug for HttpsClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("HttpsClient")
    }
}

impl HttpsClient {
    pub(crate) fn new() -> Result<Self, PackageAcquisitionError> {
        install_crypto_provider();
        let redirect = reqwest::redirect::Policy::custom(|attempt| {
            if redirect_is_allowed(attempt.url(), attempt.previous().len()) {
                attempt.follow()
            } else {
                attempt.error("redirect violates Nocter public HTTPS policy")
            }
        });
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_mins(5))
            .redirect(redirect)
            .user_agent("nocter-package-acquisition/0.14")
            .build()?;
        Ok(Self { client })
    }

    pub(crate) fn download_archive(
        &self,
        authored: &str,
    ) -> Result<Vec<u8>, PackageAcquisitionError> {
        let url = public_https_url(authored, true)?;
        let mut response = self.client.get(url).send()?.error_for_status()?;
        if response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|length| length > MAX_ARCHIVE_BYTES)
        {
            return Err(PackageAcquisitionError::ResponseTooLarge {
                url: authored.into(),
                limit: MAX_ARCHIVE_BYTES,
            });
        }
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(MAX_ARCHIVE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                PackageAcquisitionError::filesystem("read HTTPS archive response", authored, error)
            })?;
        if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(PackageAcquisitionError::ResponseTooLarge {
                url: authored.into(),
                limit: MAX_ARCHIVE_BYTES,
            });
        }
        Ok(bytes)
    }
}

pub(crate) fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
