//! Runtime configuration, loaded once from a `.env` file (with process-env
//! overrides and sensible defaults). Values are read via the accessor functions
//! below; call `config::init()` once at startup before using them.

use crate::log_info;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

pub struct Config {
    pub app_name: String,
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub server_base_url: String,
}

static CFG: OnceLock<Config> = OnceLock::new();

pub fn app_name() -> &'static str {
    &cfg().app_name
}

pub fn database_url() -> &'static str {
    &cfg().database_url
}

pub fn server_host() -> &'static str {
    &cfg().server_host
}

pub fn server_port() -> u16 {
    cfg().server_port
}

pub fn bind_address() -> String {
    format!("{}:{}", server_host(), server_port())
}

/// Public base URL used when building absolute links to stored files
/// (e.g. uploaded images). Configurable via `SERVER_BASE_URL`.
pub fn server_base_url() -> &'static str {
    &cfg().server_base_url
}

fn cfg() -> &'static Config {
    CFG.get()
        .expect("config not initialized; call config::init() first")
}

/// Robust lookup to find `.env` by searching upwards from the current directory
/// and the executable path.
fn find_env_file() -> Option<PathBuf> {
    // 1. Try current working directory and search upwards
    if let Ok(mut dir) = std::env::current_dir() {
        loop {
            let env_path = dir.join(".env");
            if env_path.is_file() {
                return Some(env_path);
            }
            if !dir.pop() {
                break;
            }
        }
    }
    // 2. Try relative to the executable path and search upwards
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(mut dir) = exe_path.parent().map(|p| p.to_path_buf())
    {
        loop {
            let env_path = dir.join(".env");
            if env_path.is_file() {
                return Some(env_path);
            }
            if !dir.pop() {
                break;
            }
        }
    }
    None
}

/// Parse the `.env` file. If not found, prints a warning and falls back.
fn parse_env_file() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(path) = find_env_file() else {
        println!(
            "[\x1b[33mWARN\x1b[0m] .env file not found in current directory or parent directories. Using defaults."
        );
        return map;
    };

    log_info!("Loading environment configuration from: {}", path.display());
    let Ok(content) = std::fs::read_to_string(path) else {
        return map;
    };

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim().to_string();
            let mut val = v.trim();
            if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
                || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
            {
                val = &val[1..val.len() - 1];
            }
            map.insert(key, val.to_string());
        }
    }
    map
}

fn lookup(env: &HashMap<String, String>, key: &str) -> Option<String> {
    std::env::var(key).ok().or_else(|| env.get(key).cloned())
}

fn get_str(env: &HashMap<String, String>, key: &str, default: &str) -> String {
    lookup(env, key).unwrap_or_else(|| default.to_string())
}

fn get_u16(env: &HashMap<String, String>, key: &str, default: u16) -> u16 {
    lookup(env, key)
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub fn init() {
    let env = parse_env_file();

    let app_name = get_str(&env, "APP_NAME", "api_sapaai_crm");
    let server_host = get_str(&env, "APP_SERVER_HOST", "0.0.0.0");
    let server_port = get_u16(&env, "APP_SERVER_PORT", 5790);

    let database_url = lookup(&env, "DATABASE_URL").unwrap_or_else(|| {
        let host = get_str(&env, "DATABASE_HOST", "127.0.0.1");
        let port = get_str(&env, "DATABASE_PORT", "3306");
        let db = get_str(&env, "DATABASE_NAME", "crm_sapaai");
        let user = get_str(&env, "DATABASE_USERNAME", "root");
        let pass = get_str(&env, "DATABASE_PASSWORD", "");
        format!(
            "mysql://{}:{}@{}:{}/{}",
            url_encode(&user),
            url_encode(&pass),
            host,
            port,
            db
        )
    });

    let server_base_url = get_str(&env, "SERVER_BASE_URL", "http://localhost:5790");

    let config = Config {
        app_name,
        database_url,
        server_host,
        server_port,
        server_base_url,
    };
    let _ = CFG.set(config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello"), "hello");
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("admin@123"), "admin%40123");
        assert_eq!(url_encode("p@ssw0rd!"), "p%40ssw0rd%21");
    }

    #[test]
    fn test_lookup_precedence() {
        let mut mock_env = HashMap::new();
        mock_env.insert("TEST_KEY".to_string(), "env_file_value".to_string());

        // 1. Verify lookup falls back to env file when process env is not set
        unsafe {
            std::env::remove_var("TEST_KEY");
        }
        assert_eq!(
            lookup(&mock_env, "TEST_KEY"),
            Some("env_file_value".to_string())
        );

        // 2. Verify process env overrides env file
        unsafe {
            std::env::set_var("TEST_KEY", "process_env_override");
        }
        assert_eq!(
            lookup(&mock_env, "TEST_KEY"),
            Some("process_env_override".to_string())
        );

        // Clean up
        unsafe {
            std::env::remove_var("TEST_KEY");
        }
    }

    #[test]
    fn test_get_str_fallback() {
        let mock_env = HashMap::new();
        unsafe {
            std::env::remove_var("NON_EXISTENT_KEY");
        }
        assert_eq!(
            get_str(&mock_env, "NON_EXISTENT_KEY", "default_val"),
            "default_val"
        );

        let mut mock_env2 = HashMap::new();
        mock_env2.insert("EXISTS_IN_FILE".to_string(), "file_val".to_string());
        assert_eq!(
            get_str(&mock_env2, "EXISTS_IN_FILE", "default_val"),
            "file_val"
        );
    }

    #[test]
    fn test_get_u16_parsing() {
        let mut env = HashMap::new();
        env.insert("PORT".to_string(), "8080".to_string());
        assert_eq!(get_u16(&env, "PORT", 3000), 8080);

        let bad = HashMap::new();
        assert_eq!(get_u16(&bad, "PORT", 3000), 3000);
    }
}
