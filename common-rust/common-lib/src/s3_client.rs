use chrono::Utc;
use golem_rust::Schema;
use golem_wasi_http::{Client, Method};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// S3 Document Source Types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct S3Config {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
    pub bucket: String,
    pub endpoint_url: Option<String>, // Custom S3-compatible endpoint
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct S3DocumentSource {
    pub bucket: String,
    pub key: String,
    pub size_bytes: u64,
    pub last_modified: String,
    pub content_type: String,
    pub namespace: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct S3ListResponse {
    pub objects: Vec<S3DocumentSource>,
    pub next_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct S3ObjectMetadata {
    pub size_bytes: u64,
    pub last_modified: String,
    pub content_type: String,
    pub bucket: String,
    pub key: String,
}

// Error types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum S3Error {
    NetworkError(String),
    AuthenticationError(String),
    NotFound(String),
    InvalidRequest(String),
    InternalError(String),
}

impl std::fmt::Display for S3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            S3Error::NetworkError(msg) => write!(f, "Network error: {}", msg),
            S3Error::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            S3Error::NotFound(msg) => write!(f, "Not found: {}", msg),
            S3Error::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            S3Error::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for S3Error {}

pub type S3Result<T> = Result<T, S3Error>;

// S3 Client Implementation
#[derive(Clone, Debug)]
pub struct S3Client {
    access_key_id: String,
    secret_access_key: String,
    region: String,
    endpoint_url: String, // Configurable S3 endpoint
    client: Client,
}

impl S3Client {
    pub fn new(
        access_key_id: String,
        secret_access_key: String,
        region: String,
        endpoint_url: Option<String>,
    ) -> S3Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| S3Error::NetworkError(format!("Failed to create HTTP client: {:?}", e)))?;

        // Use custom endpoint or default AWS S3 endpoint
        let endpoint_url =
            endpoint_url.unwrap_or_else(|| format!("https://s3.{}.amazonaws.com", region));

        Ok(Self {
            access_key_id,
            secret_access_key,
            region,
            endpoint_url,
            client,
        })
    }

    fn build_endpoint_url(&self, bucket: &str) -> String {
        if self.endpoint_url.contains("localhost") || !self.endpoint_url.contains("amazonaws.com") {
            format!("{}/{}", self.endpoint_url, bucket)
        } else {
            format!("{}.{}.amazonaws.com", bucket, self.region)
        }
    }

    pub fn list_objects(&self, bucket: &str, prefix: Option<&str>) -> S3Result<S3ListResponse> {
        let endpoint = self.build_endpoint_url(bucket);
        let path = "/";

        let mut query_params = Vec::new();
        // Match working AWS CLI format exactly: delimiter=%2F&encoding-type=url&list-type=2&prefix=
        query_params.push(("delimiter", "/"));
        query_params.push(("encoding-type", "url"));
        query_params.push(("list-type", "2"));
        // Always include prefix parameter (empty string for empty prefix)
        if let Some(p) = prefix {
            query_params.push(("prefix", p));
        } else {
            query_params.push(("prefix", ""));
        }

        let query_string = if query_params.is_empty() {
            String::new()
        } else {
            let mut sorted_params = query_params.clone();
            sorted_params.sort_by(|a, b| a.0.cmp(b.0));
            sorted_params
                .iter()
                .map(|(k, v)| format!("{}={}", k, self.url_encode(v)))
                .collect::<Vec<_>>()
                .join("&")
        };

        let url = if query_string.is_empty() {
            format!("{}{}", endpoint, path)
        } else {
            format!("{}{}?{}", endpoint, path, query_string)
        };

        let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let payload_hash = self.sha256_hex("".as_bytes());

        // Include query string in the path for signature calculation
        // For custom endpoints, include bucket in path; for AWS, use just the path
        let path_for_signature = if query_string.is_empty() {
            if self.endpoint_url.contains("localhost")
                || !self.endpoint_url.contains("amazonaws.com")
            {
                format!("/{}{}", bucket, path)
            } else {
                path.to_string()
            }
        } else {
            if self.endpoint_url.contains("localhost")
                || !self.endpoint_url.contains("amazonaws.com")
            {
                format!("/{}{}?{}", bucket, path, query_string)
            } else {
                format!("{}?{}", path, query_string)
            }
        };

        let authorization = self.create_s3_auth_header(
            "GET",
            &path_for_signature,
            &timestamp,
            &payload_hash,
            &endpoint,
        );

        log::debug!("S3 Request - URL: {}, Method: GET", url);
        log::debug!("S3 Request - Path for signature: {}", path_for_signature);
        log::debug!("S3 Request - Timestamp: {}", timestamp);
        log::debug!("S3 Request - Authorization: {}", authorization);

        let response = self
            .client
            .request(Method::GET, &url)
            .header("Authorization", authorization)
            .header("X-Amz-Date", timestamp)
            .header("X-Amz-Content-Sha256", payload_hash)
            .send()
            .map_err(|e| S3Error::NetworkError(format!("Failed to send request: {:?}", e)))?;

        if response.status().is_success() {
            let body = response
                .text()
                .map_err(|e| S3Error::NetworkError(format!("Failed to read response: {:?}", e)))?;

            self.parse_s3_list_response(&body)
        } else {
            Err(S3Error::NetworkError(format!(
                "S3 request failed with status: {}",
                response.status()
            )))
        }
    }

    pub fn get_object(&self, bucket: &str, key: &str) -> S3Result<Vec<u8>> {
        let endpoint = self.build_endpoint_url(bucket);
        let path = format!("/{}", key);

        let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let payload_hash = self.sha256_hex("".as_bytes());
        let authorization =
            self.create_s3_auth_header("GET", &path, &timestamp, &payload_hash, &endpoint);

        let url = format!("{}{}", endpoint, path);

        let response = self
            .client
            .request(Method::GET, &url)
            .header("Authorization", authorization)
            .header("X-Amz-Date", timestamp)
            .header("X-Amz-Content-Sha256", payload_hash)
            .send()
            .map_err(|e| S3Error::NetworkError(format!("Failed to send request: {:?}", e)))?;

        if response.status().is_success() {
            response
                .bytes()
                .map_err(|e| S3Error::NetworkError(format!("Failed to read object data: {:?}", e)))
                .map(|bytes| bytes.to_vec())
        } else if response.status().as_u16() == 404 {
            Err(S3Error::NotFound(format!("Object not found: {}", key)))
        } else {
            Err(S3Error::NetworkError(format!(
                "S3 request failed with status: {}",
                response.status()
            )))
        }
    }

    pub fn get_object_metadata(&self, bucket: &str, key: &str) -> S3Result<S3ObjectMetadata> {
        let endpoint = self.build_endpoint_url(bucket);
        let path = format!("/{}", key);

        let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let payload_hash = self.sha256_hex("".as_bytes());
        let authorization =
            self.create_s3_auth_header("HEAD", &path, &timestamp, &payload_hash, &endpoint);

        let url = format!("{}{}", endpoint, path);

        let response = self
            .client
            .request(Method::HEAD, &url)
            .header("Authorization", authorization)
            .header("X-Amz-Date", timestamp)
            .header("X-Amz-Content-Sha256", payload_hash)
            .send()
            .map_err(|e| S3Error::NetworkError(format!("Failed to send request: {:?}", e)))?;

        if response.status().is_success() {
            let content_length = response
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);

            let last_modified = response
                .headers()
                .get("last-modified")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .unwrap_or_default();

            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .unwrap_or_default();

            Ok(S3ObjectMetadata {
                size_bytes: content_length,
                last_modified,
                content_type,
                bucket: bucket.to_string(),
                key: key.to_string(),
            })
        } else if response.status().as_u16() == 404 {
            Err(S3Error::NotFound(format!("Object not found: {}", key)))
        } else {
            Err(S3Error::NetworkError(format!(
                "S3 request failed with status: {}",
                response.status()
            )))
        }
    }

    fn create_s3_auth_header(
        &self,
        method: &str,
        path: &str,
        timestamp: &str,
        payload_hash: &str,
        endpoint: &str,
    ) -> String {
        let date = &timestamp[0..8];

        // Extract host from endpoint (exclude bucket name for custom endpoints)
        let host = if self.endpoint_url.contains("localhost")
            || !self.endpoint_url.contains("amazonaws.com")
        {
            // For custom endpoints, host is just the base URL
            self.endpoint_url
                .replace("https://", "")
                .replace("http://", "")
        } else {
            // For AWS, use the endpoint as-is
            endpoint.replace("https://", "").replace("http://", "")
        };

        // Split path and query string for canonical request
        let (canonical_path, canonical_query_string) = if let Some(query_pos) = path.find('?') {
            (&path[..query_pos], &path[query_pos + 1..])
        } else {
            (path, "")
        };

        let canonical_headers = format!(
            "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}",
            host, payload_hash, timestamp
        );
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method,
            canonical_path,
            canonical_query_string,
            canonical_headers,
            signed_headers,
            payload_hash
        );

        log::debug!("S3 Canonical Request:\n{}", canonical_request);

        let canonical_request_hash = self.sha256_hex(canonical_request.as_bytes());

        let credential_scope = format!("{}/{}/s3/aws4_request", date, self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            timestamp, credential_scope, canonical_request_hash
        );

        log::debug!("S3 String to Sign:\n{}", string_to_sign);

        let signature = self.calculate_s3_signature(&string_to_sign, date);

        format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.access_key_id, credential_scope, signed_headers, signature
        )
    }

    fn calculate_s3_signature(&self, string_to_sign: &str, date: &str) -> String {
        // AWS CLI HMAC key derivation - exact match
        let k_date = hmac_sha256(
            format!("AWS4{}", self.secret_access_key).as_bytes(),
            date.as_bytes(),
        );
        let k_region = hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = hmac_sha256(&k_region, b"s3");
        let k_signing = hmac_sha256(&k_service, b"aws4_request");

        let signature = hmac_sha256(&k_signing, string_to_sign.as_bytes());
        hex::encode(signature)
    }
    
    fn sha256_hex(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    fn url_encode(&self, input: &str) -> String {
        input
            .chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                _ => format!("%{:02X}", c as u8),
            })
            .collect()
    }

    fn parse_s3_list_response(&self, body: &str) -> S3Result<S3ListResponse> {
        // Simple XML parsing for S3 ListObjectsV2 response
        let mut objects = Vec::new();
        let mut next_token = None;

        // This is a simplified parser - in production, use proper XML parsing
        if body.contains("<Key>") {
            for line in body.lines() {
                if line.trim().contains("<Key>") && line.trim().contains("</Key>") {
                    let key = line
                        .split("<Key>")
                        .nth(1)
                        .and_then(|s| s.split("</Key>").next())
                        .unwrap_or("")
                        .to_string();

                    if !key.is_empty() {
                        objects.push(S3DocumentSource {
                            bucket: String::new(), // Will be set by caller
                            key,
                            size_bytes: 0,
                            last_modified: String::new(),
                            content_type: String::new(),
                            namespace: String::new(), // Will be set by caller
                        });
                    }
                }
            }
        }

        if body.contains("<NextContinuationToken>") {
            for line in body.lines() {
                if line.trim().contains("<NextContinuationToken>")
                    && line.trim().contains("</NextContinuationToken>")
                {
                    next_token = Some(
                        line.split("<NextContinuationToken>")
                            .nth(1)
                            .and_then(|s| s.split("</NextContinuationToken>").next())
                            .unwrap_or("")
                            .to_string(),
                    );
                    break;
                }
            }
        }

        Ok(S3ListResponse {
            objects,
            next_token,
        })
    }
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}
