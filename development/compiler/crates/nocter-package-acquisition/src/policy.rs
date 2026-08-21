use reqwest::Url;

use crate::PackageAcquisitionError;

pub(crate) fn public_https_url(
    authored: &str,
    archive: bool,
) -> Result<Url, PackageAcquisitionError> {
    let url = Url::parse(authored)
        .map_err(|error| PackageAcquisitionError::invalid_url(authored, error.to_string()))?;
    if url.scheme() != "https" {
        return Err(PackageAcquisitionError::invalid_url(
            authored,
            "v0.14.0 supports public HTTPS sources only",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(PackageAcquisitionError::invalid_url(
            authored,
            "credentials are not allowed",
        ));
    }
    if url.host_str().is_none() {
        return Err(PackageAcquisitionError::invalid_url(
            authored,
            "a host is required",
        ));
    }
    if url.fragment().is_some() {
        return Err(PackageAcquisitionError::invalid_url(
            authored,
            "fragments are not acquisition input",
        ));
    }
    if archive && !url.path().ends_with(".tar.gz") {
        return Err(PackageAcquisitionError::invalid_url(
            authored,
            "archive sources must end in .tar.gz",
        ));
    }
    Ok(url)
}

pub(crate) fn redirect_is_allowed(url: &Url, previous: usize) -> bool {
    previous < 5
        && url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_credential_free_https_sources() {
        assert!(public_https_url("https://example.test/pkg.git", false).is_ok());
        assert!(public_https_url("https://example.test/pkg.tar.gz", true).is_ok());
        assert!(public_https_url("http://example.test/pkg.git", false).is_err());
        assert!(public_https_url("ssh://example.test/pkg.git", false).is_err());
        assert!(public_https_url("https://user@example.test/pkg.git", false).is_err());
        assert!(public_https_url("https://example.test/pkg.zip", true).is_err());
    }

    #[test]
    fn redirect_policy_is_https_credential_free_and_bounded() {
        let safe = Url::parse("https://mirror.example.test/pkg.tar.gz").unwrap();
        let downgrade = Url::parse("http://mirror.example.test/pkg.tar.gz").unwrap();
        let credentials = Url::parse("https://user@mirror.example.test/pkg.tar.gz").unwrap();
        assert!(redirect_is_allowed(&safe, 4));
        assert!(!redirect_is_allowed(&safe, 5));
        assert!(!redirect_is_allowed(&downgrade, 0));
        assert!(!redirect_is_allowed(&credentials, 0));
    }
}
