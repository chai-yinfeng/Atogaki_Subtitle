use anyhow::{Context, Result, anyhow, bail};
use reqwest::{ClientBuilder, Proxy, Url};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkProxyMode {
    Environment,
    Direct,
    Custom,
}

impl NetworkProxyMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim() {
            "environment" => Ok(Self::Environment),
            "direct" => Ok(Self::Direct),
            "custom" => Ok(Self::Custom),
            value => bail!("unsupported network proxy mode: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Direct => "direct",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkClientConfig {
    proxy_mode: NetworkProxyMode,
    custom_proxy_url: Option<String>,
}

impl NetworkClientConfig {
    pub fn new(proxy_mode: &str, custom_proxy_url: Option<String>) -> Result<Self> {
        let proxy_mode = NetworkProxyMode::parse(proxy_mode)?;
        let custom_proxy_url = normalize_optional_url(custom_proxy_url);
        if let Some(url) = custom_proxy_url.as_deref() {
            validate_proxy_url(url)?;
        }
        if proxy_mode == NetworkProxyMode::Custom && custom_proxy_url.is_none() {
            bail!("custom proxy mode requires an HTTP proxy URL");
        }
        Ok(Self {
            proxy_mode,
            custom_proxy_url,
        })
    }

    pub fn environment() -> Self {
        Self {
            proxy_mode: NetworkProxyMode::Environment,
            custom_proxy_url: None,
        }
    }

    pub fn proxy_mode(&self) -> NetworkProxyMode {
        self.proxy_mode
    }

    pub fn custom_proxy_url(&self) -> Option<&str> {
        self.custom_proxy_url.as_deref()
    }

    pub fn apply(&self, builder: ClientBuilder) -> Result<ClientBuilder> {
        match self.proxy_mode {
            NetworkProxyMode::Environment => Ok(builder),
            NetworkProxyMode::Direct => Ok(builder.no_proxy()),
            NetworkProxyMode::Custom => {
                let url = self
                    .custom_proxy_url
                    .as_deref()
                    .ok_or_else(|| anyhow!("custom proxy URL is missing"))?;
                Ok(builder.proxy(Proxy::all(url).context("invalid custom HTTP proxy")?))
            }
        }
    }
}

pub fn normalize_https_endpoint(value: Option<String>) -> Result<Option<String>> {
    let Some(value) = normalize_optional_url(value) else {
        return Ok(None);
    };
    let parsed = Url::parse(&value).map_err(|error| anyhow!("invalid HTTPS endpoint: {error}"))?;
    if parsed.scheme() != "https" {
        bail!("model mirror must use https://");
    }
    validate_public_url(&parsed, "model mirror")?;
    if parsed.query().is_some() || parsed.fragment().is_some() {
        bail!("model mirror must not contain a query or fragment");
    }
    Ok(Some(value.trim_end_matches('/').to_string()))
}

fn validate_proxy_url(value: &str) -> Result<()> {
    let parsed = Url::parse(value).map_err(|error| anyhow!("invalid HTTP proxy URL: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("custom proxy must use http:// or https://");
    }
    validate_public_url(&parsed, "custom proxy")?;
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        bail!("custom proxy URL must contain only scheme, host and optional port");
    }
    Ok(())
}

fn validate_public_url(url: &Url, label: &str) -> Result<()> {
    if url.host_str().is_none() {
        bail!("{label} URL must include a host");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("{label} URL must not contain credentials");
    }
    Ok(())
}

fn normalize_optional_url(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{NetworkClientConfig, NetworkProxyMode, normalize_https_endpoint};

    #[test]
    fn validates_proxy_modes_without_storing_credentials() {
        assert_eq!(
            NetworkClientConfig::new("environment", None)
                .unwrap()
                .proxy_mode(),
            NetworkProxyMode::Environment
        );
        assert!(NetworkClientConfig::new("custom", None).is_err());
        assert!(
            NetworkClientConfig::new(
                "custom",
                Some("http://user:secret@127.0.0.1:7897".to_string())
            )
            .is_err()
        );
        assert!(
            NetworkClientConfig::new("custom", Some("socks5://127.0.0.1:7897".to_string()))
                .is_err()
        );
    }

    #[test]
    fn normalizes_only_https_model_endpoints() {
        assert_eq!(
            normalize_https_endpoint(Some(" https://mirror.example/hf/ ".to_string())).unwrap(),
            Some("https://mirror.example/hf".to_string())
        );
        assert!(normalize_https_endpoint(Some("http://mirror.example".to_string())).is_err());
        assert!(
            normalize_https_endpoint(Some("https://user:secret@mirror.example".to_string()))
                .is_err()
        );
    }
}
