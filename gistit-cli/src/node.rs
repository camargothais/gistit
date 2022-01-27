//! The host module
use std::env;
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::{Command, Stdio};

use async_trait::async_trait;
use clap::ArgMatches;
use console::style;

use lib_gistit::ipc::{self, Instruction, ServerResponse};

use crate::dispatch::{get_runtime_dir, Dispatch};
use crate::params::Check;
use crate::{prettyln, Result};

#[derive(Debug, Clone)]
pub struct Action {
    pub init: Option<&'static str>,
    pub join: Option<&'static str>,
    pub start: bool,
    pub stop: bool,
    pub status: bool,
    pub host: &'static str,
    pub port: &'static str,
    pub clipboard: bool,
}

impl Action {
    pub fn from_args(
        args: &'static ArgMatches,
    ) -> Result<Box<dyn Dispatch<InnerData = Config> + Send + Sync + 'static>> {
        // merge settings

        Ok(Box::new(Self {
            init: args.value_of("init"),
            join: args.value_of("join"),
            clipboard: args.is_present("clipboard"),
            // SAFETY: Has default values
            host: unsafe { args.value_of("host").unwrap_unchecked() },
            port: unsafe { args.value_of("port").unwrap_unchecked() },
            start: args.is_present("start"),
            stop: args.is_present("stop"),
            status: args.is_present("status"),
        }))
    }
}

pub enum ProcessCommand {
    Init(&'static str),
    Join(&'static str),
    Start,
    Stop,
    Status,
}

pub struct Config {
    command: ProcessCommand,
    host: Ipv4Addr,
    port: u16,
}

#[async_trait]
impl Dispatch for Action {
    type InnerData = Config;

    async fn prepare(&'static self) -> Result<Self::InnerData> {
        <Self as Check>::check(self)?;

        let command = match (self.init, self.join, self.start, self.stop, self.status) {
            (Some(seed), None, false, false, false) => ProcessCommand::Init(seed),
            (None, Some(address), false, false, false) => ProcessCommand::Join(address),
            (None, None, true, false, false) => ProcessCommand::Start,
            (None, None, false, true, false) => ProcessCommand::Stop,
            (None, None, false, false, true) => ProcessCommand::Status,
            (_, _, _, _, _) => unreachable!(),
        };

        // SAFETY: Previously checked in [`Check::check`]
        let (host, port) = unsafe {
            (
                self.host.parse::<Ipv4Addr>().unwrap_unchecked(),
                self.port.parse::<u16>().unwrap_unchecked(),
            )
        };

        let config = Config {
            command,
            host,
            port,
        };

        Ok(config)
    }

    async fn dispatch(&'static self, config: Self::InnerData) -> Result<()> {
        let runtime_dir = get_runtime_dir()?;
        let mut bridge = ipc::client(&runtime_dir)?;

        match config.command {
            ProcessCommand::Init(seed) => {
                if let Some(ipfs_path) = env::var_os("IPFS_PATH") {
                    if let Some(ipfs_config) =
                        std::fs::read_to_string(Path::new(&ipfs_path).join("config"))
                    {
                        let json_config: serde_json::Value = serde_json::from_str(ipfs_config).ok();
                    }
                }
            }
            ProcessCommand::Start => {
                if bridge.alive() {
                    prettyln!("Running..."); // TODO: change this to status msg
                    return Ok(());
                }

                let pid = spawn(&runtime_dir)?;
                prettyln!(
                    "Starting gistit network node process, pid: {}",
                    style(pid).blue()
                );

                bridge.connect_blocking()?;
                bridge
                    .send(Instruction::Listen {
                        host: config.host,
                        port: config.port,
                    })
                    .await?;

                if let Instruction::Response(ServerResponse::PeerId(id)) = bridge.recv().await? {
                    print_success(self.clipboard, id);
                }
            }
            ProcessCommand::Join(address) => {
                if !bridge.alive() {
                    prettyln!("Gistit node must be running to join a peer");
                } else {
                    bridge.connect_blocking()?;
                    bridge
                        .send(Instruction::Dial {
                            peer_id: address.to_owned(),
                        })
                        .await?;
                }
            }
            ProcessCommand::Stop => {
                prettyln!("Stopping gistit network node process...");
                fs::remove_file(runtime_dir.join("gistit.log"))?;
                bridge.connect_blocking()?;
                bridge.send(Instruction::Shutdown).await?;
            }
            ProcessCommand::Status => {
                if bridge.alive() {
                    bridge.connect_blocking()?;
                    bridge.send(Instruction::Status).await?;

                    if let Instruction::Response(ServerResponse::Status(status_str)) =
                        bridge.recv().await?
                    {
                        println!("{}", status_str);
                    }
                } else {
                    prettyln!("Not running");
                }
            }
        };
        Ok(())
    }
}

fn get_node_config() -> Result<String> {
    todo!()
}

fn spawn(runtime_dir: &Path, seed: &str) -> Result<u32> {
    let stdout = fs::File::create(runtime_dir.join("gistit.log"))?;
    let daemon = "/home/fabricio7p/Documents/Projects/gistit/target/debug/gistit-daemon";
    let child = Command::new(daemon)
        .args(["--seed", seed])
        .args(["--runtime-dir", runtime_dir.to_string_lossy().as_ref()])
        .stderr(stdout)
        .stdout(Stdio::null())
        .spawn()?;

    Ok(child.id())
}

fn print_success(has_clipboard: bool, peer_id: String) {
    let clipboard_msg = if has_clipboard {
        "(copied to clipboard)".to_owned()
    } else {
        "".to_owned()
    };
    println!(
        r#"
SUCCESS:
    peer id: {} {}
"#,
        peer_id,
        style(clipboard_msg).italic()
    );
}
