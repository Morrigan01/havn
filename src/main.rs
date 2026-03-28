mod api;
mod cli;
mod config;
mod env_file;
mod logs;
mod mcp;
mod rate_limit;
mod registry;
mod scanner;
mod secrets;
mod server;
mod service;
mod ws;

use clap::Parser;

#[tokio::main]
async fn main() {
    let args = cli::Cli::parse();

    config::init_logging(&args);

    match &args.command {
        Some(cli::Command::Status) => cli::status(&args).await,
        Some(cli::Command::Kill { target }) => cli::kill(&args, target).await,
        Some(cli::Command::Add { path }) => cli::add(&args, path).await,
        Some(cli::Command::Remove { target }) => cli::remove(&args, target).await,
        Some(cli::Command::Config { key, value }) => cli::config_cmd(key, value.as_deref()),
        Some(cli::Command::Logs) => cli::logs().await,
        Some(cli::Command::Mcp) => cli::mcp(&args).await,
        Some(cli::Command::InstallService) => cli::install_service(),
        Some(cli::Command::Restart { target }) => cli::restart(&args, target).await,
        Some(cli::Command::SetStartCmd { project, cmd }) => {
            cli::set_start_cmd(&args, project, cmd).await
        }
        Some(cli::Command::Secret { action }) => cli::secret(&args, action).await,
        Some(cli::Command::Update) => cli::update().await,
        Some(cli::Command::Tools) => cli::list_tools(),
        None => {
            cli::check_for_update_notice().await;
            let bind = args.bind.clone();
            let port = args.port;
            server::run(bind, port).await;
        }
    }
}
