/*
    This file is part of Eruption.

    Eruption is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    Eruption is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with Eruption.  If not, see <http://www.gnu.org/licenses/>.
*/

use clap::Clap;
// use colored::*;
use log::*;
use std::{
    env,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use syslog::Facility;

mod constants;
mod util;

// type Result<T> = std::result::Result<T, eyre::Error>;

#[derive(Debug, thiserror::Error)]
pub enum MainError {
    #[error("Unknown error: {description}")]
    UnknownError { description: String },

    #[error("Could not parse syslog log-level")]
    SyslogLevelError {},
}

/// Supported command line arguments
#[derive(Debug, Clap)]
#[clap(
    version = env!("CARGO_PKG_VERSION"),
    author = "X3n0m0rph59 <x3n0m0rph59@gmail.com>",
    about = "A utility used to notify Eruption about device hotplug events",
)]
pub struct Options {
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[clap(short, long, parse(from_occurrences))]
    verbose: u8,

    #[clap(subcommand)]
    command: Subcommands,
}

// Sub-commands
#[derive(Debug, Clap)]
pub enum Subcommands {
    /// Trigger a hotplug event
    Hotplug,

    /// Generate shell completions
    Completions {
        #[clap(subcommand)]
        command: CompletionsSubcommands,
    },
}

/// Subcommands of the "completions" command
#[derive(Debug, Clap)]
pub enum CompletionsSubcommands {
    Bash,

    Elvish,

    Fish,

    PowerShell,

    Zsh,
}

/// Print license information
#[allow(dead_code)]
fn print_header() {
    println!(
        r#"
 Eruption is free software: you can redistribute it and/or modify
 it under the terms of the GNU General Public License as published by
 the Free Software Foundation, either version 3 of the License, or
 (at your option) any later version.

 Eruption is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU General Public License for more details.

 You should have received a copy of the GNU General Public License
 along with Eruption.  If not, see <http://www.gnu.org/licenses/>.
"#
    );
}

#[tokio::main(flavor = "multi_thread")]
pub async fn main() -> std::result::Result<(), eyre::Error> {
    cfg_if::cfg_if! {
        if #[cfg(debug_assertions)] {
            color_eyre::config::HookBuilder::default()
            .panic_section("Please consider reporting a bug at https://github.com/X3n0m0rph59/eruption")
            .install()?;
        } else {
            color_eyre::config::HookBuilder::default()
            .panic_section("Please consider reporting a bug at https://github.com/X3n0m0rph59/eruption")
            .display_env_section(false)
            .install()?;
        }
    }

    if unsafe { libc::isatty(0) != 0 } {
        // initialize logging on console
        if env::var("RUST_LOG").is_err() {
            env::set_var("RUST_LOG_OVERRIDE", "info");
            pretty_env_logger::init_custom_env("RUST_LOG_OVERRIDE");
        } else {
            pretty_env_logger::init();
        }
    } else {
        // initialize logging to syslog
        let mut errors_present = false;

        let level_filter = match env::var("RUST_LOG")
            .unwrap_or_else(|_| "info".to_string())
            .to_lowercase()
            .as_str()
        {
            "off" => log::LevelFilter::Off,
            "error" => log::LevelFilter::Error,
            "warn" => log::LevelFilter::Warn,
            "info" => log::LevelFilter::Info,
            "debug" => log::LevelFilter::Debug,
            "trace" => log::LevelFilter::Trace,

            _ => {
                errors_present = true;
                log::LevelFilter::Info
            }
        };

        syslog::init(
            Facility::LOG_DAEMON,
            level_filter,
            Some(env!("CARGO_PKG_NAME")),
        )
        .map_err(|_e| MainError::SyslogLevelError {})?;

        if errors_present {
            log::error!("Could not parse syslog log-level");
        }
    }

    let opts = Options::parse();
    match opts.command {
        Subcommands::Hotplug => {
            // place a lockfile, so we don't run into loops
            match lockfile::Lockfile::create("/run/lock/eruption-hotplug-helper.lock") {
                Ok(lock_file) => {
                    if Path::new("/run/lock/eruption-sleep.lock").exists() {
                        log::info!("Waking up from system sleep...");

                        // sleep until udev has settled
                        log::info!("Waiting for the devices to settle...");

                        // udevadm settle may/will deadlock since eruption adds (virtual) devices to udev
                        let status = Command::new("/usr/bin/udevadm")
                            .arg("settle")
                            .stdout(Stdio::null())
                            .status();

                        if let Err(e) = status {
                            // udev-settle has failed, sleep a while and let the devices settle

                            log::error!("udevadm settle has failed: {}", e);

                            thread::sleep(Duration::from_millis(2500));
                        } else {
                            // sleep a while just to be safe
                            thread::sleep(Duration::from_millis(500));

                            log::info!("Done, all devices have settled");
                        }

                        log::info!("Now starting the eruption.service...");

                        // TODO: Implement a D-Bus based notification interface,
                        //       simply restart the eruption.service for now
                        let status = Command::new("/usr/bin/systemctl")
                            .arg("start")
                            .arg("eruption.service")
                            .stdout(Stdio::null())
                            .status()?;

                        if status.success() {
                            // wait for the eruption.service to be fully operational...
                            log::info!("Waiting for Eruption to be fully operational...");

                            let mut retry_counter = 0;

                            'WAIT_START_LOOP: loop {
                                let result = Command::new("/usr/bin/systemctl")
                                    .arg("is-active")
                                    .arg("eruption.service")
                                    .stdout(Stdio::null())
                                    .status();

                                match result {
                                    Ok(status) => {
                                        if status.success() {
                                            log::info!(
                                                "Eruption has been started successfully, exiting now"
                                            );

                                            break 'WAIT_START_LOOP;
                                        } else {
                                            thread::sleep(Duration::from_millis(1000));

                                            if retry_counter >= 5 {
                                                log::error!(
                                                    "Timeout while starting eruption.service"
                                                );

                                                break 'WAIT_START_LOOP;
                                            } else {
                                                retry_counter += 1;
                                            }
                                        }
                                    }

                                    Err(e) => {
                                        log::error!(
                                            "Error while waiting for Eruption to start: {}",
                                            e
                                        );

                                        break 'WAIT_START_LOOP;
                                    }
                                }
                            }
                        } else {
                            log::error!("Could not start Eruption, an error occurred");
                        }
                    } else {
                        log::info!(
                            "A hotplug event has been triggered, notifying the Eruption daemon..."
                        );

                        log::debug!("Checking whether the system is fully booted...");

                        let result = Command::new("/usr/bin/systemctl")
                            .arg("is-system-running")
                            .stdout(Stdio::null())
                            .status();

                        match result {
                            Ok(status) if status.success() => {
                                // sleep until udev has settled
                                log::info!("Waiting for the devices to settle...");

                                // udevadm settle may/will deadlock since eruption adds (virtual) devices to udev
                                let status = Command::new("/usr/bin/udevadm")
                                    .arg("settle")
                                    .stdout(Stdio::null())
                                    .status();

                                if let Err(e) = status {
                                    // udev-settle has failed, sleep a while and let the devices settle

                                    log::error!("udevadm settle has failed: {}", e);

                                    thread::sleep(Duration::from_millis(2500));
                                } else {
                                    // sleep a while just to be safe
                                    thread::sleep(Duration::from_millis(500));

                                    log::info!("Done, all devices have settled");
                                }

                                log::info!("Now restarting the eruption.service...");

                                // TODO: Implement a D-Bus based notification interface,
                                //       simply restart the eruption.service for now
                                let status = Command::new("/usr/bin/systemctl")
                                    .arg("restart")
                                    .arg("eruption.service")
                                    .stdout(Stdio::null())
                                    .status()?;

                                if status.success() {
                                    // wait for the eruption.service to be fully operational...
                                    log::info!("Waiting for Eruption to be fully operational...");

                                    let mut retry_counter = 0;

                                    'WAIT_RESTART_LOOP: loop {
                                        let result = Command::new("/usr/bin/systemctl")
                                            .arg("is-active")
                                            .arg("eruption.service")
                                            .stdout(Stdio::null())
                                            .status();

                                        match result {
                                            Ok(status) => {
                                                if status.success() {
                                                    log::info!(
                                                    "Notification sent successfully, exiting now"
                                                );

                                                    break 'WAIT_RESTART_LOOP;
                                                } else {
                                                    thread::sleep(Duration::from_millis(1000));

                                                    if retry_counter >= 5 {
                                                        log::error!(
                                                        "Timeout while restarting eruption.service"
                                                    );

                                                        break 'WAIT_RESTART_LOOP;
                                                    } else {
                                                        retry_counter += 1;
                                                    }
                                                }
                                            }

                                            Err(e) => {
                                                log::error!(
                                                    "Error while waiting for Eruption to start: {}",
                                                    e
                                                );

                                                break 'WAIT_RESTART_LOOP;
                                            }
                                        }
                                    }
                                } else {
                                    log::error!("Could not notify Eruption, an error occurred");
                                }
                            }

                            Err(e) => {
                                log::error!(
                                    "Could not determine whether the system is still booting: {}",
                                    e
                                );
                            }

                            _ => {
                                log::info!("System is still booting, skipping restart of Eruption");
                            }
                        }
                    }

                    // thread::sleep(Duration::from_millis(2000));

                    lock_file.release()?;
                }

                Err(lockfile::Error::LockTaken) => {
                    log::warn!("We have been invoked while holding a global lock, exiting now");
                }

                Err(lockfile::Error::Io(e)) => {
                    log::error!("An error occurred while creating the lock file: {}", e);
                }

                Err(_) => {
                    log::error!("An unknown error occurred while creating the lock file");
                }
            }
        }

        Subcommands::Completions { command } => {
            use clap::IntoApp;
            use clap_generate::{generate, generators::*};

            const BIN_NAME: &str = env!("CARGO_PKG_NAME");

            let mut app = Options::into_app();
            let mut fd = std::io::stdout();

            match command {
                CompletionsSubcommands::Bash => {
                    generate::<Bash, _>(&mut app, BIN_NAME, &mut fd);
                }

                CompletionsSubcommands::Elvish => {
                    generate::<Elvish, _>(&mut app, BIN_NAME, &mut fd);
                }

                CompletionsSubcommands::Fish => {
                    generate::<Fish, _>(&mut app, BIN_NAME, &mut fd);
                }

                CompletionsSubcommands::PowerShell => {
                    generate::<PowerShell, _>(&mut app, BIN_NAME, &mut fd);
                }

                CompletionsSubcommands::Zsh => {
                    generate::<Zsh, _>(&mut app, BIN_NAME, &mut fd);
                }
            }
        }
    }

    Ok(())
}
