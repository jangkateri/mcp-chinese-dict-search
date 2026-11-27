//! Loopback-only Ollama HTTP client and the real embedder.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::embedder::Embedder;
use super::profiles::EmbeddingProfile;

const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
const BUILD_TIMEOUT: Duration = Duration::from_secs(300);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const KEEP_ALIVE: &str = "30m";
const PROBE_TEXT: &str = "dimension probe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTag {
    pub name: String,
    pub digest: String,
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TagsEnvelope {
    models: Vec<TagsModel>,
}

#[derive(Debug, Deserialize)]
struct TagsModel {
    name: String,
    digest: String,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Debug, Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
    truncate: bool,
    keep_alive: &'a str,
    options: EmbedOptions,
}

#[derive(Debug, Serialize)]
struct EmbedOptions {
    num_ctx: u32,
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    total_duration: Option<u64>,
}

/// A blocking client for a loopback Ollama server.
#[derive(Debug, Clone)]
pub struct OllamaClient {
    base_url: String,
    agent: ureq::Agent,
    timeout: Duration,
}

impl OllamaClient {
    pub fn query(base_url: &str) -> Result<Self, String> {
        Self::new(base_url, QUERY_TIMEOUT)
    }

    pub fn build(base_url: &str) -> Result<Self, String> {
        Self::new(base_url, BUILD_TIMEOUT)
    }

    pub fn new(base_url: &str, timeout: Duration) -> Result<Self, String> {
        let base_url = base_url.trim().trim_end_matches('/').to_owned();
        if !is_loopback_url(&base_url) {
            return Err(format!(
                "the Ollama URL {base_url:?} is not loopback; only 127.0.0.1, localhost, or ::1 \
                 is accepted"
            ));
        }
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .build();
        Ok(Self {
            base_url,
            agent: config.into(),
            timeout,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// `GET /api/tags`.
    pub fn tags(&self) -> Result<Vec<ModelTag>, String> {
        let mut response = self
            .agent
            .get(format!("{}/api/tags", self.base_url))
            .call()
            .map_err(|error| format!("GET /api/tags failed: {error}"))?;
        let envelope: TagsEnvelope = response
            .body_mut()
            .read_json()
            .map_err(|error| format!("GET /api/tags returned invalid JSON: {error}"))?;
        Ok(envelope
            .models
            .into_iter()
            .map(|model| ModelTag {
                name: model.name,
                digest: model.digest,
                size: model.size,
            })
            .collect())
    }

    /// Resolve one configured model to a stable digest.
    pub fn model_tag(&self, model: &str) -> Result<ModelTag, String> {
        let tags = self.tags()?;
        let matches: Vec<&ModelTag> = tags.iter().filter(|tag| tag.name == model).collect();
        match matches.as_slice() {
            [] => Err(format!(
                "the Ollama model {model:?} is not installed; run `ollama pull {model}`"
            )),
            [tag] => {
                if tag.digest.trim().is_empty() {
                    Err(format!("the Ollama model {model:?} has an empty digest"))
                } else {
                    Ok((*tag).clone())
                }
            }
            _ => {
                let first = matches[0];
                if matches.iter().all(|tag| tag.digest == first.digest) {
                    Ok(first.clone())
                } else {
                    Err(format!(
                        "the Ollama server reports duplicate entries for {model:?} with different \
                         digests; remove the stale tag and pull it again"
                    ))
                }
            }
        }
    }

    /// `POST /api/show`, used only for diagnostics.
    pub fn show(&self, model: &str) -> Result<Value, String> {
        let mut response = self
            .agent
            .post(format!("{}/api/show", self.base_url))
            .send_json(serde_json::json!({ "model": model }))
            .map_err(|error| format!("POST /api/show failed: {error}"))?;
        response
            .body_mut()
            .read_json()
            .map_err(|error| format!("POST /api/show returned invalid JSON: {error}"))
    }

    /// `POST /api/embed` for one batch. Inputs are already formatted.
    pub fn embed(
        &self,
        model: &str,
        inputs: &[String],
        profile: &EmbeddingProfile,
        keep_alive: &str,
    ) -> Result<Vec<Vec<f32>>, String> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let request = EmbedRequest {
            model,
            input: inputs,
            truncate: true,
            keep_alive,
            options: EmbedOptions {
                num_ctx: profile.num_ctx,
            },
        };
        let mut response = self
            .agent
            .post(format!("{}/api/embed", self.base_url))
            .send_json(request)
            .map_err(|error| format!("POST /api/embed failed: {error}"))?;
        let response: EmbedResponse = response
            .body_mut()
            .read_json()
            .map_err(|error| format!("POST /api/embed returned invalid JSON: {error}"))?;
        if response.embeddings.len() != inputs.len() {
            return Err(format!(
                "POST /api/embed returned {} vectors for {} inputs",
                response.embeddings.len(),
                inputs.len()
            ));
        }
        let mut dimensions = None;
        for vector in &response.embeddings {
            if vector.is_empty() {
                return Err("POST /api/embed returned an empty vector".to_owned());
            }
            if !vector.iter().all(|value| value.is_finite()) {
                return Err("POST /api/embed returned NaN or infinity".to_owned());
            }
            if let Some(expected) = dimensions {
                if vector.len() != expected {
                    return Err("POST /api/embed returned inconsistent dimensions".to_owned());
                }
            } else {
                dimensions = Some(vector.len());
            }
            let norm = vector
                .iter()
                .map(|value| f64::from(*value) * f64::from(*value))
                .sum::<f64>()
                .sqrt();
            if (norm - 1.0).abs() > 0.02 {
                return Err(format!(
                    "POST /api/embed returned a vector with norm {norm}, not close to 1"
                ));
            }
        }
        let _ = (
            &response.model,
            response.prompt_eval_count,
            response.total_duration,
        );
        Ok(response.embeddings)
    }

    pub fn dimensions(&self, model: &str, profile: &EmbeddingProfile) -> Result<usize, String> {
        let vectors = self.embed(
            model,
            &[profile.document_text(PROBE_TEXT)],
            profile,
            KEEP_ALIVE,
        )?;
        vectors
            .first()
            .map(Vec::len)
            .ok_or_else(|| "the dimension probe returned no vector".to_owned())
    }
}

/// The real embedder. It probes dimensions once and reuses them.
#[derive(Debug, Clone)]
pub struct OllamaEmbedder {
    client: OllamaClient,
    model: String,
    profile: EmbeddingProfile,
    dimensions: usize,
    keep_alive: String,
}

impl OllamaEmbedder {
    pub fn new(
        client: OllamaClient,
        model: impl Into<String>,
        profile: EmbeddingProfile,
        dimensions: usize,
    ) -> Self {
        Self {
            client,
            model: model.into(),
            profile,
            dimensions,
            keep_alive: KEEP_ALIVE.to_owned(),
        }
    }

    pub fn connect(
        client: OllamaClient,
        model: impl Into<String>,
        profile: EmbeddingProfile,
    ) -> Result<Self, String> {
        let model = model.into();
        let dimensions = client.dimensions(&model, &profile)?;
        Ok(Self::new(client, model, profile, dimensions))
    }

    pub fn client(&self) -> &OllamaClient {
        &self.client
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn profile(&self) -> &EmbeddingProfile {
        &self.profile
    }
}

impl Embedder for OllamaEmbedder {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let inputs: Vec<String> = texts
            .iter()
            .map(|text| self.profile.document_text(text))
            .collect();
        self.client
            .embed(&self.model, &inputs, &self.profile, &self.keep_alive)
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        let input = self.profile.query_text(text);
        self.client
            .embed(&self.model, &[input], &self.profile, &self.keep_alive)?
            .into_iter()
            .next()
            .ok_or_else(|| "the query embedding returned no vector".to_owned())
    }
}

pub fn is_loopback_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    let Some(rest) = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))
    else {
        return false;
    };
    if rest.starts_with("[::1]") {
        return true;
    }
    let host = rest.split([':', '/']).next().unwrap_or("");
    matches!(host, "127.0.0.1" | "localhost")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;

    struct MockServer {
        url: String,
        request: Arc<Mutex<String>>,
        handle: JoinHandle<()>,
    }

    impl MockServer {
        fn start(status: &'static str, body: &'static str) -> Self {
            Self::start_delayed(status, body, Duration::ZERO)
        }

        fn start_delayed(status: &'static str, body: &'static str, delay: Duration) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let request = Arc::new(Mutex::new(String::new()));
            let captured = Arc::clone(&request);
            let handle = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0u8; 8192];
                let read = stream.read(&mut buffer).unwrap();
                let text = String::from_utf8_lossy(&buffer[..read]).into_owned();
                let length = header_value(&text, "content-length")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                let header_end = text.find("\r\n\r\n").map(|index| index + 4).unwrap_or(read);
                let header_text = text[..header_end].to_owned();
                let mut body_text = text[header_end..].to_owned();
                while body_text.len() < length {
                    let read = stream.read(&mut buffer).unwrap();
                    if read == 0 {
                        break;
                    }
                    body_text.push_str(&String::from_utf8_lossy(&buffer[..read]));
                }
                *captured.lock().unwrap() = format!("{header_text}{body_text}");
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: \
                     {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            });
            Self {
                url,
                request,
                handle,
            }
        }

        fn join(self) -> String {
            self.handle.join().unwrap();
            self.request.lock().unwrap().clone()
        }
    }

    fn header_value<'a>(request: &'a str, name: &str) -> Option<&'a str> {
        request.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim()
                .eq_ignore_ascii_case(name)
                .then_some(value.trim())
        })
    }

    #[test]
    fn tags_are_parsed_and_a_missing_model_is_actionable() {
        let server = MockServer::start(
            "200 OK",
            r#"{"models":[{"name":"qwen3-embedding:0.6b","digest":"abc123","size":42}]}"#,
        );
        let client = OllamaClient::query(&server.url).unwrap();
        let tags = client.tags().unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].digest, "abc123");
        server.join();

        let server = MockServer::start("200 OK", r#"{"models":[]}"#);
        let client = OllamaClient::query(&server.url).unwrap();
        assert!(
            client
                .model_tag("missing")
                .unwrap_err()
                .contains("ollama pull")
        );
        server.join();
    }

    #[test]
    fn embed_sends_the_documented_request_and_parses_vectors() {
        let server = MockServer::start(
            "200 OK",
            r#"{"embeddings":[[1.0,0.0],[0.0,1.0]],"model":"test","prompt_eval_count":2}"#,
        );
        let client = OllamaClient::build(&server.url).unwrap();
        let profile = EmbeddingProfile::for_model("qwen3-embedding:0.6b");
        let vectors = client
            .embed(
                "test",
                &["alpha".to_owned(), "beta".to_owned()],
                &profile,
                "30m",
            )
            .unwrap();
        assert_eq!(vectors.len(), 2);
        let request = server.join();
        assert!(request.starts_with("POST /api/embed"));
        assert!(request.contains("\"truncate\": true"));
        assert!(request.contains("\"keep_alive\": \"30m\""));
        assert!(request.contains("\"num_ctx\": 4096"));
    }

    #[test]
    fn malformed_json_and_http_errors_are_reported() {
        let server = MockServer::start("200 OK", "not json");
        let client = OllamaClient::query(&server.url).unwrap();
        assert!(client.tags().unwrap_err().contains("invalid JSON"));
        server.join();

        let server = MockServer::start("500 Internal Server Error", r#"{"error":"boom"}"#);
        let client = OllamaClient::query(&server.url).unwrap();
        assert!(client.tags().is_err());
        server.join();
    }

    #[test]
    fn duplicate_models_with_different_digests_are_rejected() {
        let server = MockServer::start(
            "200 OK",
            r#"{"models":[{"name":"m","digest":"a"},{"name":"m","digest":"b"}]}"#,
        );
        let client = OllamaClient::query(&server.url).unwrap();
        assert!(client.model_tag("m").unwrap_err().contains("duplicate"));
        server.join();
    }

    #[test]
    fn a_hung_server_is_bounded_by_the_client_timeout() {
        let server =
            MockServer::start_delayed("200 OK", r#"{"models":[]}"#, Duration::from_millis(250));
        let client = OllamaClient::new(&server.url, Duration::from_millis(50)).unwrap();
        let started = std::time::Instant::now();
        assert!(client.tags().is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        server.join();
    }

    #[test]
    fn remote_urls_are_rejected() {
        assert!(OllamaClient::query("https://ollama.com").is_err());
        assert!(OllamaClient::query("http://192.168.1.10:11434").is_err());
        assert!(OllamaClient::query("http://127.0.0.1:11434").is_ok());
    }
}
