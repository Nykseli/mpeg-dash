use std::env;

mod config;
mod server;

fn main() {
    let args: Vec<String> = env::args().collect();
    let conf_path = if args.len() < 2 {
        "config.json"
    } else {
        &args[1][..]
    };

    // Config needs to be initialized here. See the init function for more information
    config::GlobalConfig::init(conf_path);
    let server = server::DashServer::new();
    server.start_server();
}
