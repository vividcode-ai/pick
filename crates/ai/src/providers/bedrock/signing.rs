use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

const AWS_ALGORITHM: &str = "AWS4-HMAC-SHA256";
const AWS_SERVICE: &str = "bedrock";
const AWS_REQUEST_TYPE: &str = "aws4_request";

fn iso8601_date(time: &chrono::DateTime<chrono::Utc>) -> String {
    time.format("%Y%m%dT%H%M%SZ").to_string()
}

fn short_date(time: &chrono::DateTime<chrono::Utc>) -> String {
    time.format("%Y%m%d").to_string()
}

pub(crate) fn sha256_hex(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn derive_signing_key(secret_key: &str, date_stamp: &str, region: &str) -> Vec<u8> {
    let k_secret = format!("AWS4{}", secret_key);
    let k_date = hmac_sha256(k_secret.as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, AWS_SERVICE.as_bytes());
    hmac_sha256(&k_service, AWS_REQUEST_TYPE.as_bytes())
}

pub(crate) fn build_aws_auth_header(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    payload: &[u8],
    access_key: &str,
    secret_key: &str,
    session_token: Option<&str>,
    region: &str,
) -> String {
    let now = chrono::Utc::now();
    let amz_date = iso8601_date(&now);
    let date_stamp = short_date(&now);

    let parsed_url = url::Url::parse(url).expect("Invalid URL");
    let canonical_uri = parsed_url.path().to_string();
    let canonical_query = parsed_url.query().unwrap_or_default();

    let payload_hash = sha256_hex(payload);

    let mut canonical_headers = Vec::new();
    let mut signed_headers = Vec::new();

    let host = parsed_url.host_str().unwrap_or("");
    canonical_headers.push(("host".to_string(), host.to_string()));
    signed_headers.push("host".to_string());

    canonical_headers.push(("x-amz-date".to_string(), amz_date.clone()));
    signed_headers.push("x-amz-date".to_string());

    canonical_headers.push(("x-amz-content-sha256".to_string(), payload_hash.clone()));
    signed_headers.push("x-amz-content-sha256".to_string());

    if let Some(token) = session_token {
        canonical_headers.push(("x-amz-security-token".to_string(), token.to_string()));
        signed_headers.push("x-amz-security-token".to_string());
    }

    for (key, value) in headers {
        let lower_key = key.to_lowercase();
        if lower_key == "content-type" || lower_key == "authorization" {
            continue;
        }
        canonical_headers.push((lower_key.clone(), value.clone()));
        signed_headers.push(lower_key);
    }

    canonical_headers.sort_by(|a, b| a.0.cmp(&b.0));
    signed_headers.sort();

    let canonical_header_str: String = canonical_headers
        .iter()
        .map(|(k, v)| format!("{}:{}", k, v.trim()))
        .collect::<Vec<_>>()
        .join("\n");

    let signed_header_str = signed_headers.join(";");

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method,
        canonical_uri,
        canonical_query,
        canonical_header_str,
        signed_header_str,
        payload_hash,
    );

    let canonical_request_hash = sha256_hex(canonical_request.as_bytes());

    let credential_scope = format!("{}/{}/{}/{}", date_stamp, region, AWS_SERVICE, AWS_REQUEST_TYPE);

    let string_to_sign = format!(
        "{}\n{}\n{}\n{}",
        AWS_ALGORITHM, amz_date, credential_scope, canonical_request_hash,
    );

    let signing_key = derive_signing_key(secret_key, &date_stamp, region);
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));

    let credential = format!("{}/{}", access_key, credential_scope);
    format!(
        "{} Credential={}, SignedHeaders={}, Signature={}",
        AWS_ALGORITHM, credential, signed_header_str, signature,
    )
}
