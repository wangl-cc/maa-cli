use super::{normalize_url, return_true, Channel};

use std::env::var_os;

use serde::Deserialize;

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    channel: Channel,
    #[serde(default = "default_api_url")]
    api_url: String,
    #[serde(default = "default_download_url")]
    download_url: String,
    #[serde(default)]
    components: CLIComponents,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            channel: Default::default(),
            api_url: default_api_url(),
            download_url: default_download_url(),
            components: Default::default(),
        }
    }
}

impl Config {
    pub fn channel(&self) -> Channel {
        self.channel
    }

    pub fn set_channel(&mut self, channel: Channel) -> &Self {
        self.channel = channel;
        self
    }

    pub fn api_url(&self) -> String {
        format!("{}{}.json", normalize_url(&self.api_url), self.channel())
    }

    pub fn set_api_url(&mut self, api_url: impl ToString) -> &Self {
        self.api_url = api_url.to_string();
        self
    }

    pub fn download_url(&self, tag: &str, name: &str) -> String {
        format!("{}{}/{}", normalize_url(&self.download_url), tag, name)
    }

    pub fn set_download_url(&mut self, download_url: impl ToString) -> &Self {
        self.download_url = download_url.to_string();
        self
    }

    pub fn components(&self) -> &CLIComponents {
        &self.components
    }
}

fn default_api_url() -> String {
    if let Some(url) = var_os("MAA_CLI_API") {
        url.into_string().unwrap()
    } else {
        "https://github.com/MaaAssistantArknights/maa-cli/raw/version/".to_owned()
    }
}

fn default_download_url() -> String {
    if let Some(url) = var_os("MAA_CLI_DOWNLOAD") {
        url.into_string().unwrap()
    } else {
        "https://github.com/MaaAssistantArknights/maa-cli/releases/download/".to_owned()
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Deserialize, Default, Clone)]
pub struct CLIComponents {
    #[serde(default = "return_true")]
    pub binary: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Config {
        pub fn with_channel(mut self, channel: Channel) -> Self {
            self.channel = channel;
            self
        }

        pub fn with_api_url(mut self, api_url: impl ToString) -> Self {
            self.api_url = api_url.to_string();
            self
        }

        pub fn with_download_url(mut self, download_url: impl ToString) -> Self {
            self.download_url = download_url.to_string();
            self
        }
    }

    mod serde {
        use super::*;

        use serde_test::{assert_de_tokens, Token};

        #[test]
        fn deserialize_cli_components() {
            assert_de_tokens(
                &CLIComponents { binary: true },
                &[Token::Map { len: Some(0) }, Token::MapEnd],
            );
            assert_de_tokens(
                &CLIComponents { binary: false },
                &[
                    Token::Map { len: Some(1) },
                    Token::Str("binary"),
                    Token::Bool(false),
                    Token::MapEnd,
                ],
            );
        }

        #[test]
        fn deserialize_config() {
            assert_de_tokens(
                &Config {
                    channel: Channel::Alpha,
                    api_url: "https://foo.bar/api/".to_owned(),
                    download_url: "https://foo.bar/download/".to_owned(),
                    components: CLIComponents { binary: false },
                },
                &[
                    Token::Map { len: Some(4) },
                    Token::Str("channel"),
                    Channel::Alpha.as_token(),
                    Token::Str("api_url"),
                    Token::Str("https://foo.bar/api/"),
                    Token::Str("download_url"),
                    Token::Str("https://foo.bar/download/"),
                    Token::Str("components"),
                    Token::Map { len: Some(1) },
                    Token::Str("binary"),
                    Token::Bool(false),
                    Token::MapEnd,
                    Token::MapEnd,
                ],
            );

            assert_de_tokens(
                &Config::default(),
                &[Token::Map { len: Some(0) }, Token::MapEnd],
            );
        }
    }

    mod default {
        use super::*;

        use std::env::{remove_var, set_var};

        #[test]
        fn api_url() {
            assert_eq!(
                default_api_url(),
                "https://github.com/MaaAssistantArknights/maa-cli/raw/version/"
            );

            set_var("MAA_CLI_API", "https://foo.bar/cli/");
            assert_eq!(default_api_url(), "https://foo.bar/cli/");
            remove_var("MAA_CLI_API");
        }

        #[test]
        fn download_url() {
            assert_eq!(
                default_download_url(),
                "https://github.com/MaaAssistantArknights/maa-cli/releases/download/",
            );

            set_var("MAA_CLI_DOWNLOAD", "https://foo.bar/download/");
            assert_eq!(default_download_url(), "https://foo.bar/download/");
            remove_var("MAA_CLI_DOWNLOAD");
        }
    }

    mod methods {
        use super::*;

        #[test]
        fn channel() {
            assert_eq!(Config::default().channel(), Default::default());
            assert_eq!(
                Config::default().set_channel(Channel::Alpha).channel(),
                Channel::Alpha,
            );
        }

        #[test]
        fn api_url() {
            assert_eq!(
                Config::default().api_url(),
                "https://github.com/MaaAssistantArknights/maa-cli/raw/version/stable.json",
            );
            assert_eq!(
                Config::default()
                    .with_channel(Channel::Alpha)
                    .with_api_url("https://foo.bar/cli/")
                    .api_url(),
                "https://foo.bar/cli/alpha.json",
            );
        }

        #[test]
        fn download_url() {
            assert_eq!(
                Config::default().download_url("v0.3.12", "maa_cli.zip"),
                "https://github.com/MaaAssistantArknights/maa-cli/releases/download/v0.3.12/maa_cli.zip",
            );
            assert_eq!(
                Config::default()
                    .with_download_url("https://foo.bar/download/")
                    .download_url("v0.3.12", "maa_cli.zip"),
                "https://foo.bar/download/v0.3.12/maa_cli.zip",
            );
        }
    }
}
