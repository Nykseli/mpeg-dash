use serde::Deserialize;
use std::fs;

/// Default ipv4 address
fn def_ipv4_addr() -> String {
    "0.0.0.0".to_string()
}

/// Default ipv4 port
fn def_ipv4_port() -> String {
    "443".to_string()
}

/// Default Access-Control-Allow-Origin http header
fn def_allow_origin() -> String {
    "*".to_string()
}

/// Default structure for network in Config
fn def_network() -> Network {
    Network {
        port: def_ipv4_port(),
        address: def_ipv4_addr(),
        allow_origin: def_allow_origin(),
    }
}

/// Default ThreadPool size
fn def_thread_pool_size() -> usize {
    4
}

/// Default tcp connection timeout in seconds
fn def_tcp_connection_timeout() -> f64 {
    // PHP / Apache seems to use 30 secs so that's probably a good value
    30.0
}

/// Default structure for performance in Config
fn def_performance() -> Performance {
    Performance {
        thread_pool_size: def_thread_pool_size(),
        connection_timeout: def_tcp_connection_timeout(),
    }
}

fn true_value() -> bool {
    true
}

/// Default path for tls certificate file
fn def_ssl_cert_path() -> String {
    "cert.pem".to_string()
}

/// Default path for tls private key file
fn def_ssl_private_key_path() -> String {
    "private.pem".to_string()
}

/// Default structure for security in Config
fn def_security() -> Security {
    Security {
        https: true_value(),
        certificate_file: def_ssl_cert_path(),
        private_key_file: def_ssl_private_key_path(),
    }
}

#[derive(Debug, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct Network {
    /// IPv4 address.
    /// E.g. (0.0.0.0) for all connections, (127.0.0.1) for localhost only.
    /// ## Defaults to "0.0.0.0".
    #[serde(default = "def_ipv4_addr")]
    pub address: String,
    /// What port is the address bound to.
    /// E.g. 443 for the default https port, 80 for default http port.
    /// ## Defaults to "443".
    #[serde(default = "def_ipv4_port")]
    pub port: String,
    /// Defines the Http header "Access-Control-Allow-Origin"
    /// ## Defaults to "*".
    #[serde(default = "def_allow_origin")]
    pub allow_origin: String,
}

#[derive(Debug, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct Performance {
    /// How many threads are handling the connection.
    /// Note that too many threads can overwhelm the system.
    /// ## Defaults to 4.
    #[serde(default = "def_thread_pool_size")]
    pub thread_pool_size: usize,
    /// How long will the server wait for data before closing the connection
    #[serde(default = "def_tcp_connection_timeout")]
    pub connection_timeout: f64,
}

#[derive(Debug, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct Security {
    /// Is https enabled.
    /// # Currently this is required to be on.
    /// ## Defaults to true
    #[serde(default = "true_value")]
    pub https: bool,
    /// Relative or absolute path to certificate file
    /// ## Defaults to "cert.pem"
    #[serde(default = "def_ssl_cert_path")]
    pub certificate_file: String,
    /// Relative or absolute path to private key file
    /// ## Defaults to "private.pem"
    #[serde(default = "def_ssl_private_key_path")]
    pub private_key_file: String,
}

#[derive(Debug, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "def_network")]
    pub network: Network,
    #[serde(default = "def_performance")]
    pub performance: Performance,
    #[serde(default = "def_security")]
    pub security: Security,
}

/// Singleton wrapper for Config
pub struct GlobalConfig {
    configuration: Option<Config>,
}

impl GlobalConfig {
    /// Initialize config.
    /// This should be called in main.rs since the program depends on this.
    /// If config isn't initialized. It my cause run time errors.
    /// # Panics if called twice during the runtime.
    pub fn init(path: &str) {
        // Make sure that this is only called once.
        // Since the reads are unsafe, reinit during runtime might cause issues.
        assert!(!GlobalConfig::is_init());

        let json_data = fs::read_to_string(path).expect("Cannot read the configuration file");
        let conf: Config = serde_json::from_str(&json_data[..]).expect("Json formatting error");
        unsafe {
            GLOBAL_CONFIG = GlobalConfig {
                configuration: Some(conf),
            }
        };
    }

    fn is_init() -> bool {
        match unsafe { &GLOBAL_CONFIG.configuration.as_ref() } {
            Some(_) => true,
            None => false,
        }
    }

    /// Return the initialized config
    /// # Panics if config isn't initilized before this
    pub fn config() -> &'static Config {
        // as_ref gets the configurations reference so rust doesn't
        // try to to create a duplication or copy of the configuration
        unsafe { &GLOBAL_CONFIG.configuration.as_ref().unwrap() }
    }
}

/// GLOBAL_CONFIG should be treated as read only after initialization
static mut GLOBAL_CONFIG: GlobalConfig = GlobalConfig {
    configuration: None,
};

// Rest of the file is tests
#[cfg(test)]
mod config_tests {
    use super::*;
    const CONFIG_FULL: &str = "test_data/config_full.json";
    const INVALID_CONFIG: &str = "test_data/config_invalid_json.json";
    const INVALID_VALUE: &str = "test_data/config_invalid_value.json";
    const EMPTY_OBJECT: &str = "test_data/config_empty_object.json";

    /// call this in every function to make sure config is set to None
    /// This avoids the assert!(!GlobalConfig::is_init()); from erroring out druing tests
    fn test_init_conf() {
        unsafe {
            GLOBAL_CONFIG = GlobalConfig {
                configuration: None,
            };
        }
    }

    #[test]
    #[should_panic]
    fn init_file_not_found() {
        test_init_conf();
        GlobalConfig::init("this_file_doesnt_exist.json");
    }

    #[test]
    #[should_panic]
    fn invalid_json_file() {
        test_init_conf();
        GlobalConfig::init(INVALID_CONFIG);
    }

    #[test]
    #[should_panic]
    fn double_init_panic() {
        test_init_conf();
        GlobalConfig::init(CONFIG_FULL);
        GlobalConfig::init(CONFIG_FULL);
    }

    #[test]
    #[should_panic]
    fn invalid_value_in_json() {
        test_init_conf();
        GlobalConfig::init(INVALID_VALUE);
    }

    #[test]
    fn full_config() {
        test_init_conf();
        GlobalConfig::init(CONFIG_FULL);
        let config = GlobalConfig::config();
        assert_eq!(
            *config,
            Config {
                network: Network {
                    address: "127.0.0.1".to_string(),
                    port: "9443".to_string(),
                    allow_origin: "255.255.255.1".to_string(),
                },
                security: Security {
                    https: false,
                    private_key_file: "private_test_path.pem".to_string(),
                    certificate_file: "cert_test_path.pem".to_string(),
                },
                performance: Performance {
                    thread_pool_size: 123,
                    connection_timeout: 321.4,
                },
            }
        );
    }

    #[test]
    fn empty_object_defaults() {
        test_init_conf();
        GlobalConfig::init(EMPTY_OBJECT);
        let config = GlobalConfig::config();
        assert!(config == config);
        assert_eq!(
            *config,
            Config {
                network: def_network(),
                security: def_security(),
                performance: def_performance(),
            }
        );
    }
}
