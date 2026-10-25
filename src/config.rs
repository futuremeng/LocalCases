use serde::{Deserialize, Serialize};
use std::{fs, sync::LazyLock};

/// Read once, shared by every binary in the process.
pub static CONFIG: LazyLock<Config> = LazyLock::new(Config::from_args);

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// fjall library directory
    pub db: String,
    /// tantivy index directory
    pub index_path: String,
    /// address the local search service listens on
    pub addr: String,
    /// index 全文 as well. Off by default: it multiplies the index size for no
    /// measured query win, since the text is in the library either way.
    #[serde(default)]
    pub index_with_full_text: bool,
    /// directory of imported source files (`*.zip`, each holding CSV members)
    pub raw_data_path: Option<String>,
    /// row cap for `export=true`
    pub export_limit: Option<usize>,
    /// deepest `offset` the service will serve, so a stray page number cannot
    /// make tantivy walk the whole result set
    pub max_results: Option<usize>,
}

impl Config {
    /// `LOCALCASES_CONFIG`, else the first argument, else `config.toml` next to
    /// the binary. The desktop shell sets the variable; the CLI passes an argument.
    fn from_args() -> Config {
        let path = std::env::var("LOCALCASES_CONFIG")
            .ok()
            .filter(|p| !p.trim().is_empty())
            .or_else(|| std::env::args().nth(1))
            .unwrap_or_else(|| "config.toml".to_owned());
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        basic_toml::from_str(&text).unwrap_or_else(|e| panic!("invalid {path}: {e}"))
    }
}
