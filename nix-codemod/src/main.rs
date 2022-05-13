
mod walkers;
mod edit;
mod commands;

use std::error::Error;

use clap::Parser;
use clap::Subcommand;

use commands::*;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Command
}

#[derive(Subcommand)]
enum Command {
    ListSystemdServices {
        module: String,
        #[clap(short, long)]
        verbose: bool,
    },
    PrintSystemdServiceConfig {
        module: String,
        service: String,
        #[clap(short, long)]
        verbose: bool,
    },
    EditSystemdService {
        module: String,
        service: String,
        options: String,
        #[clap(short, long)]
        verbose: bool,
    },
    InsertSystemdHooks {
        module: String,
        service: String,
        option_names: String,
    },
    FindAllTests {
        all_tests: String,
    },
    IsTestWellFormed {
        test: String,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::ListSystemdServices { module, verbose } =>
            list_systemd_services(&module, verbose)?,
        Command::PrintSystemdServiceConfig { module, service, verbose } =>
            print_systemd_service_config(&module, &service, verbose)?,
        Command::EditSystemdService { module, service, options, verbose } =>
            edit_systemd_service(&module, &service, &options, verbose)?,
        Command::InsertSystemdHooks { module, service, option_names } =>
            insert_systemd_hooks(&module, &service, &option_names)?,
        Command::FindAllTests { all_tests } =>
            find_all_tests(&all_tests)?,
        Command::IsTestWellFormed { test } =>
            is_test_well_formed(&test)?,
    }

    Ok(())
}

