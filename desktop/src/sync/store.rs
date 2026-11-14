//! Object stores: S3 and the S3-compatible services, Azure Blob Storage and Google Cloud Storage.
//! Each one gets, puts and deletes objects under the remote folder (the prefix).

use std::io::Read;
use std::time::{Duration, Instant};

use super::sign::{self, SigV4, sha256_hex, uri_encode};

/// Shared by the sync's worker threads, so each call takes `&self`.
pub trait Store: Send + Sync {
    /// The object's bytes, or None when it does not exist.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
    fn put(&self, key: &str, body: &[u8]) -> Result<(), String>;
    fn delete(&self, key: &str) -> Result<(), String>;
    /// The bucket or container, for messages.
    fn place(&self) -> String;
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout(Duration::from_secs(120)).build()
}

/// A request that did not get an answer, and whether another try can help.
struct Fail {
    msg: String,
    again: bool,
}

/// Runs `once` up to 5 times. It tries again after 0.5, 1, 2 and 4 seconds when the service
/// is busy (429, 500, 502, 503, 504) or the connection failed. Object stores send these now
/// and then, and one of them must not fail a sync of hundreds of notes.
fn retry(mut once: impl FnMut() -> Result<(u16, Vec<u8>), Fail>) -> Result<(u16, Vec<u8>), String> {
    let waits = [0.5, 1.0, 2.0, 4.0];
    let scale = if cfg!(test) { 0.002 } else { 1.0 };
    let mut i = 0;
    loop {
        let r = once();
        let again = match &r {
            Ok((code, _)) => matches!(code, 429 | 500 | 502 | 503 | 504),
            Err(f) => f.again,
        };
        if !again || i == waits.len() {
            return r.map_err(|f| f.msg);
        }
        std::thread::sleep(Duration::from_secs_f64(waits[i] * scale));
        i += 1;
    }
}

/// One request: status and body. Transport failures become errors with a plain message.
fn send(req: ureq::Request, body: Option<&[u8]>) -> Result<(u16, Vec<u8>), Fail> {
    let res = match body {
        Some(b) => req.send_bytes(b),
        None => req.call(),
    };
    let resp = match res {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(ureq::Error::Transport(t)) => {
            let host = t.url().and_then(|u| u.host_str().map(|h| u.port().map_or(h.to_string(), |p| format!("{h}:{p}")))).unwrap_or_else(|| "the server".into());
            let why = match t.kind() {
                ureq::ErrorKind::Dns => "The host name does not resolve. Check the endpoint and the region.".to_string(),
                ureq::ErrorKind::InvalidUrl | ureq::ErrorKind::UnknownScheme => "The endpoint is not a valid URL.".to_string(),
                ureq::ErrorKind::ConnectionFailed => match t.message() {
                    Some(m) if m.to_lowercase().contains("certificate") || m.to_lowercase().contains("tls") => format!("The TLS check failed: {m}."),
                    _ => "The connection was refused or timed out.".to_string(),
                },
                _ => "The connection broke.".to_string(),
            };
            let again = !matches!(t.kind(), ureq::ErrorKind::Dns | ureq::ErrorKind::InvalidUrl | ureq::ErrorKind::UnknownScheme);
            return Err(Fail { msg: format!("Could not reach {host}. {why}"), again });
        }
    };
    let code = resp.status();
    let mut out = Vec::new();
    resp.into_reader().take(256 * 1024 * 1024).read_to_end(&mut out).map_err(|e| Fail { msg: format!("The connection broke: {e}"), again: true })?;
    Ok((code, out))
}

/// The message for a busy or failing service, the same for every provider.
fn busy(code: u16) -> Option<String> {
    match code {
        429 => Some("The storage service asks Typo to slow down (429). Typo tries again on the next sync.".into()),
        502..=504 => Some(format!("The storage service did not answer in time ({code}). Typo tries again on the next sync.")),
        500..=599 => Some(format!("The storage service had an error ({code}). Typo tries again on the next sync.")),
        _ => None,
    }
}

/// A message for a failed request, from its status and the service's XML or JSON error body.
fn failure(code: u16, body: &[u8], secret_label: &str) -> String {
    let text = String::from_utf8_lossy(body);
    let tag = |t: &str| text.split(&format!("<{t}>")).nth(1).and_then(|r| r.split(&format!("</{t}>")).next()).map(str::to_string);
    let code_name = tag("Code").unwrap_or_default();
    if let Some(msg) = busy(code) {
        return msg;
    }
    match (code, code_name.as_str()) {
        (_, "NoSuchBucket") | (_, "ContainerNotFound") => "The bucket does not exist. Check the bucket name and the region.".into(),
        (403, _) | (401, _) | (_, "SignatureDoesNotMatch") | (_, "InvalidAccessKeyId") | (_, "AuthenticationFailed") => {
            format!("Access denied ({code}). Check the {secret_label}.")
        }
        (301, _) | (_, "PermanentRedirect") | (_, "AuthorizationHeaderMalformed") => "The bucket is in another region. Pick the region it was made in.".into(),
        _ => {
            let msg = tag("Message").unwrap_or_else(|| text.chars().take(160).collect());
            format!("The server answered {code}. {msg}")
        }
    }
}

pub struct S3 {
    /// `https://host[:port]`, no trailing slash.
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access: String,
    pub secret: String,
    pub path_style: bool,
    pub prefix: String,
    /// The field name of the secret, for messages ("secret access key", "application key").
    pub secret_label: String,
    agent: ureq::Agent,
}

impl S3 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(endpoint: &str, bucket: &str, region: &str, access: &str, secret: &str, path_style: bool, prefix: &str, secret_label: &str) -> S3 {
        S3 {
            endpoint: endpoint.trim().trim_end_matches('/').to_string(),
            bucket: bucket.trim().to_string(),
            region: if region.trim().is_empty() { "us-east-1".into() } else { region.trim().to_string() },
            access: access.trim().to_string(),
            secret: secret.trim().to_string(),
            path_style,
            prefix: prefix.to_string(),
            secret_label: secret_label.to_string(),
            agent: agent(),
        }
    }

    fn request(&self, method: &str, key: &str, body: Option<&[u8]>) -> Result<(u16, Vec<u8>), String> {
        let (scheme, host) = match self.endpoint.split_once("://") {
            Some((s, h)) => (s, h.trim_end_matches('/')),
            None => ("https", self.endpoint.as_str()),
        };
        let object = uri_encode(&format!("{}{key}", self.prefix), true);
        let (host, uri) = if self.path_style {
            (host.to_string(), format!("/{}/{object}", uri_encode(&self.bucket, false)))
        } else {
            (format!("{}.{host}", self.bucket), format!("/{object}"))
        };
        let payload = sha256_hex(body.unwrap_or(&[]));
        retry(|| {
        let (amz_date, date) = sign::amz_dates(sign::now_secs());
        let headers = [("host", host.clone()), ("x-amz-content-sha256", payload.clone()), ("x-amz-date", amz_date.clone())];
        let s = SigV4 { access: &self.access, secret: &self.secret, region: &self.region, service: "s3" };
        let auth = s.authorization(method, &uri, "", &headers, &payload, &amz_date, &date);
        let req = self
            .agent
            .request(method, &format!("{scheme}://{host}{uri}"))
            .set("x-amz-content-sha256", &payload)
            .set("x-amz-date", &amz_date)
            .set("Authorization", &auth);
        send(req, body)
        })
    }
}

impl Store for S3 {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let (code, body) = self.request("GET", key, None)?;
        match code {
            200 => Ok(Some(body)),
            404 if !String::from_utf8_lossy(&body).contains("NoSuchBucket") => Ok(None),
            _ => Err(failure(code, &body, &self.secret_label)),
        }
    }
    fn put(&self, key: &str, body: &[u8]) -> Result<(), String> {
        let (code, resp) = self.request("PUT", key, Some(body))?;
        if (200..300).contains(&code) { Ok(()) } else { Err(failure(code, &resp, &self.secret_label)) }
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        let (code, resp) = self.request("DELETE", key, None)?;
        if (200..300).contains(&code) || code == 404 { Ok(()) } else { Err(failure(code, &resp, &self.secret_label)) }
    }
    fn place(&self) -> String {
        self.bucket.clone()
    }
}

pub struct Azure {
    pub account: String,
    pub container: String,
    /// The decoded account key, or None with a SAS token.
    pub key: Option<Vec<u8>>,
    pub sas: Option<String>,
    pub prefix: String,
    /// `https://<account>.blob.core.windows.net`, or another base for an emulator.
    pub base: String,
    agent: ureq::Agent,
}

impl Azure {
    pub fn new(account: &str, container: &str, auth: &str, secret: &str, prefix: &str) -> Result<Azure, String> {
        let account = account.trim().to_string();
        let (key, sas) = if auth == "sas" {
            (None, Some(secret.trim().trim_start_matches('?').to_string()))
        } else {
            use base64::Engine;
            let k = base64::engine::general_purpose::STANDARD.decode(secret.trim()).map_err(|_| "The account key is not valid base64. Copy it again from the Azure portal.".to_string())?;
            (Some(k), None)
        };
        let base = std::env::var("TYPO_AZURE_BASE").unwrap_or_else(|_| format!("https://{account}.blob.core.windows.net"));
        Ok(Azure { account, container: container.trim().to_string(), key, sas, prefix: prefix.to_string(), base, agent: agent() })
    }

    fn request(&self, method: &str, key: &str, body: Option<&[u8]>) -> Result<(u16, Vec<u8>), String> {
        let blob = uri_encode(&format!("{}{key}", self.prefix), true);
        let base_path = self.base.split_once("://").map_or("", |(_, rest)| rest.find('/').map_or("", |i| &rest[i..]));
        let path = format!("{}/{}/{blob}", base_path.trim_end_matches('/'), uri_encode(&self.container, false));
        let mut url = format!("{}/{}/{blob}", self.base.trim_end_matches('/'), uri_encode(&self.container, false));
        if let Some(s) = &self.sas {
            url.push('?');
            url.push_str(s);
        }
        retry(|| {
        let date = sign::rfc1123(sign::now_secs());
        let mut x_ms = vec![("x-ms-date", date), ("x-ms-version", "2021-08-06".to_string())];
        if method == "PUT" {
            x_ms.push(("x-ms-blob-type", "BlockBlob".to_string()));
        }
        let mut req = self.agent.request(method, &url);
        for (k, v) in &x_ms {
            req = req.set(k, v);
        }
        if let Some(k) = &self.key {
            let len = body.map_or(String::new(), |b| if b.is_empty() { String::new() } else { b.len().to_string() });
            let auth = sign::azure_shared_key(k, &self.account, method, &len, &x_ms, &format!("/{}{path}", self.account));
            req = req.set("Authorization", &auth);
        }
        send(req, body)
        })
    }
}

impl Store for Azure {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let (code, body) = self.request("GET", key, None)?;
        match code {
            200 => Ok(Some(body)),
            404 if !String::from_utf8_lossy(&body).contains("ContainerNotFound") => Ok(None),
            _ => Err(failure(code, &body, if self.sas.is_some() { "SAS token" } else { "account key" })),
        }
    }
    fn put(&self, key: &str, body: &[u8]) -> Result<(), String> {
        let (code, resp) = self.request("PUT", key, Some(body))?;
        if (200..300).contains(&code) { Ok(()) } else { Err(failure(code, &resp, if self.sas.is_some() { "SAS token" } else { "account key" })) }
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        let (code, resp) = self.request("DELETE", key, None)?;
        if (200..300).contains(&code) || code == 404 { Ok(()) } else { Err(failure(code, &resp, "account key")) }
    }
    fn place(&self) -> String {
        self.container.clone()
    }
}

/// Google Cloud Storage with a service account key: a signed JWT buys an OAuth token, and the
/// JSON API moves the objects.
pub struct Gcs {
    pub bucket: String,
    pub prefix: String,
    email: String,
    key: rsa::RsaPrivateKey,
    token_uri: String,
    token: std::sync::Mutex<Option<(String, Instant)>>,
    agent: ureq::Agent,
}

impl Gcs {
    pub fn new(bucket: &str, json: &str, prefix: &str) -> Result<Gcs, String> {
        let v: serde_json::Value = serde_json::from_str(json).map_err(|_| "The service account key is not valid JSON. Paste the whole key file.".to_string())?;
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
        let email = s("client_email").ok_or("The key file has no client_email.")?;
        let pem = s("private_key").ok_or("The key file has no private_key.")?;
        use rsa::pkcs8::DecodePrivateKey;
        let key = rsa::RsaPrivateKey::from_pkcs8_pem(&pem).map_err(|_| "The private key in the key file could not be read.".to_string())?;
        Ok(Gcs {
            bucket: bucket.trim().to_string(),
            prefix: prefix.to_string(),
            email,
            key,
            token_uri: s("token_uri").unwrap_or_else(|| "https://oauth2.googleapis.com/token".into()),
            token: Default::default(),
            agent: agent(),
        })
    }

    fn token(&self) -> Result<String, String> {
        let mut slot = self.token.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((t, until)) = &*slot
            && Instant::now() < *until
        {
            return Ok(t.clone());
        }
        use base64::Engine;
        let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
        let now = sign::now_secs();
        let header = b64(br#"{"alg":"RS256","typ":"JWT"}"#);
        let claims = serde_json::json!({
            "iss": self.email, "scope": "https://www.googleapis.com/auth/devstorage.read_write",
            "aud": self.token_uri, "iat": now, "exp": now + 3600,
        });
        let body = format!("{header}.{}", b64(claims.to_string().as_bytes()));
        use rsa::signature::{SignatureEncoding, Signer};
        let signer = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(self.key.clone());
        let sig = signer.sign(body.as_bytes()).to_vec();
        let jwt = format!("{body}.{}", b64(&sig));
        let form = format!("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={jwt}");
        let (code, resp) = retry(|| send(self.agent.post(&self.token_uri).set("Content-Type", "application/x-www-form-urlencoded"), Some(form.as_bytes())))?;
        if code != 200 {
            return Err(format!("Google refused the service account key ({code}). Check that the key is current."));
        }
        let v: serde_json::Value = serde_json::from_slice(&resp).map_err(|e| e.to_string())?;
        let t = v.get("access_token").and_then(|x| x.as_str()).ok_or("Google gave no access token.")?.to_string();
        let ttl = v.get("expires_in").and_then(|x| x.as_u64()).unwrap_or(3600).saturating_sub(120);
        *slot = Some((t.clone(), Instant::now() + Duration::from_secs(ttl)));
        Ok(t)
    }

    fn object_url(&self, key: &str) -> String {
        format!("https://storage.googleapis.com/storage/v1/b/{}/o/{}", uri_encode(&self.bucket, false), uri_encode(&format!("{}{key}", self.prefix), false))
    }
}

impl Store for Gcs {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let t = self.token()?;
        let (code, body) = retry(|| send(self.agent.get(&format!("{}?alt=media", self.object_url(key))).set("Authorization", &format!("Bearer {t}")), None))?;
        if let Some(msg) = busy(code) {
            return Err(msg);
        }
        match code {
            200 => Ok(Some(body)),
            404 if !String::from_utf8_lossy(&body).contains("bucket does not exist") => Ok(None),
            404 => Err("The bucket does not exist. Check the bucket name.".into()),
            401 | 403 => Err(format!("Access denied ({code}). Give the service account the Storage Object Admin role on the bucket.")),
            _ => Err(format!("Google Cloud answered {code}.")),
        }
    }
    fn put(&self, key: &str, body: &[u8]) -> Result<(), String> {
        let t = self.token()?;
        let url = format!(
            "https://storage.googleapis.com/upload/storage/v1/b/{}/o?uploadType=media&name={}",
            uri_encode(&self.bucket, false),
            uri_encode(&format!("{}{key}", self.prefix), false)
        );
        let (code, _) = retry(|| send(self.agent.post(&url).set("Authorization", &format!("Bearer {t}")).set("Content-Type", "application/octet-stream"), Some(body)))?;
        if (200..300).contains(&code) { Ok(()) } else { Err(busy(code).unwrap_or_else(|| format!("Google Cloud refused the upload ({code})."))) }
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        let t = self.token()?;
        let (code, _) = retry(|| send(self.agent.delete(&self.object_url(key)).set("Authorization", &format!("Bearer {t}")), None))?;
        if (200..300).contains(&code) || code == 404 { Ok(()) } else { Err(busy(code).unwrap_or_else(|| format!("Google Cloud refused the delete ({code})."))) }
    }
    fn place(&self) -> String {
        self.bucket.clone()
    }
}
