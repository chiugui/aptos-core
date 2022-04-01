// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Result};
use aptos_transaction_replay::AptosDebugger;
use aptos_types::{
    account_address::AccountAddress,
    event::EventKey,
    transaction::{TransactionPayload, Version},
};
use difference::Changeset;
use move_core_types::effects::ChangeSet;
use std::{fs, path::PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Path to the local AptosDB file
    #[structopt(long, parse(from_os_str))]
    db: Option<PathBuf>,
    /// Full URL address to connect to - should include port number, if applicable
    #[structopt(short = "u", long)]
    url: Option<String>,
    /// If true, persist the effects of replaying transactions via `cmd` to disk in a format understood by the Move CLI
    #[structopt(short = "s", global = true)]
    save_write_sets: bool,
    #[structopt(subcommand)] // Note that we mark a field as a subcommand
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Replay transactions starting from version `start` to `start + limit`.
    #[structopt(name = "replay-transactions")]
    ReplayTransactions { start: Version, limit: u64 },
    /// Replay the last `txns` committed transactions.
    #[structopt(name = "replay-recent-transactions")]
    ReplayRecentTransactions { txns: u64 },
    /// Replay the `seq`th transaction committed by `account`
    #[structopt(name = "replay-transaction-by-sequence-number")]
    ReplayTransactionBySequence {
        #[structopt(parse(try_from_str))]
        account: AccountAddress,
        seq: u64,
    },
    /// Execute a writeset as if it is signed by the Root and print the result.
    #[structopt(name = "replay-writeset")]
    ReplayWriteSetAtVersion {
        /// Path to a serialized WriteSetPayload. Could be generated by the `aptos-writeset-generator` tool.
        #[structopt(parse(from_os_str))]
        write_set_blob_path: PathBuf,
        version: u64,
    },
    /// Annotate the resources stored under `account` at `version`.
    #[structopt(name = "annotate-account")]
    AnnotateAccount {
        #[structopt(parse(try_from_str))]
        account: AccountAddress,
        version: Option<Version>,
    },
    /// Annotate the resources stored under `aptos_root`, `treasury_compliance` and all validator addresses.
    #[structopt(name = "annotate-key-accounts")]
    AnnotateKeyAccounts { version: Version },
    /// Annotate the events stored under `key` with range `start` to `start+limit`.
    #[structopt(name = "annotate-events")]
    AnnotateEvents { key: String, start: u64, limit: u64 },
    /// Diff between the resources stored under two versions of the same `account`
    #[structopt(name = "diff-account")]
    DiffAccount {
        #[structopt(parse(try_from_str))]
        account: AccountAddress,
        base_version: Version,
        revision: Version,
    },
    /// Get the bytecode for all Framework modules at `version`
    #[structopt(name = "get-modules")]
    GetModules { version: Version },
    #[structopt(name = "bisect-transaction")]
    BisectTransaction {
        #[structopt(parse(from_os_str))]
        script_path: PathBuf,
        #[structopt(parse(try_from_str))]
        sender: AccountAddress,
        begin: Version,
        end: Version,
        #[structopt(long)]
        rebuild_stdlib: bool,
    },
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let debugger = if let Some(p) = opt.db {
        AptosDebugger::db(p)?
    } else {
        panic!("No debugger attached")
    };

    println!("Connection Succeeded");

    match opt.cmd {
        Command::ReplayTransactions { start, limit } => {
            println!(
                "{:#?}",
                debugger.execute_past_transactions(start, limit, opt.save_write_sets)
            );
        }
        Command::ReplayRecentTransactions { txns } => {
            let latest_version = debugger
                .get_latest_version()
                .expect("Failed to get latest version");
            assert!(latest_version >= txns);
            println!(
                "{:#?}",
                debugger.execute_past_transactions(
                    latest_version - txns,
                    txns,
                    opt.save_write_sets
                )
            );
        }
        Command::ReplayTransactionBySequence { account, seq } => {
            let version = debugger
                .get_version_by_account_sequence(account, seq)?
                .expect("Version not found");
            println!(
                "Executing transaction at version: {:?}\n{:#?}",
                version,
                debugger.execute_past_transactions(version, 1, opt.save_write_sets)
            );
        }
        Command::ReplayWriteSetAtVersion {
            write_set_blob_path: path,
            version,
        } => {
            let transaction_payload = bcs::from_bytes(&fs::read(path.as_path())?)?;
            let writeset_payload = if let TransactionPayload::WriteSet(ws) = transaction_payload {
                ws
            } else {
                bail!("Unexpected transaction payload: {:?}", transaction_payload);
            };
            println!(
                "{:?}",
                debugger.execute_writeset_at_version(
                    version,
                    &writeset_payload,
                    opt.save_write_sets
                )?
            );
        }
        Command::AnnotateAccount {
            account,
            version: version_opt,
        } => {
            let version = match version_opt {
                Some(v) => v,
                None => debugger.get_latest_version()?,
            };
            println!(
                "{}",
                debugger
                    .annotate_account_state_at_version(account, version, opt.save_write_sets)?
                    .expect("Account not found")
            )
        }
        Command::AnnotateEvents { key, start, limit } => {
            debugger.pretty_print_events(
                &EventKey::from_bytes(hex::decode(key.as_str())?)?,
                start,
                limit,
            )?;
        }
        Command::AnnotateKeyAccounts { version } => {
            for (addr, state) in
                debugger.annotate_key_accounts_at_version(version, opt.save_write_sets)?
            {
                println!("Account: {}, State: {}", addr, state);
            }
        }
        Command::DiffAccount {
            account,
            base_version,
            revision,
        } => {
            let base_annotation = format!(
                "{}",
                debugger
                    .annotate_account_state_at_version(account, base_version, false)?
                    .expect("Account not found")
            );
            let revision_annotation = format!(
                "{}",
                debugger
                    .annotate_account_state_at_version(account, revision, false)?
                    .expect("Account not found")
            );
            println!(
                "{}",
                Changeset::new(&base_annotation, &revision_annotation, "\n")
            );
        }
        Command::GetModules { version } => {
            let modules =
                debugger.get_aptos_framework_modules_at_version(version, opt.save_write_sets)?;
            println!("Fetched {} modules", modules.len())
        }
        Command::BisectTransaction {
            sender,
            script_path,
            begin,
            end,
            rebuild_stdlib: reload_stdlib,
        } => println!(
            "{:?}",
            debugger.bisect_transactions_by_script(
                script_path.to_str().expect("Expect an str"),
                sender,
                begin,
                end,
                if reload_stdlib {
                    let mut change_set = ChangeSet::new();
                    for module in framework::aptos_modules() {
                        let mut bytes = vec![];
                        module.serialize(&mut bytes)?;
                        change_set.publish_module(module.self_id(), bytes)?;
                    }
                    Some(change_set)
                } else {
                    None
                },
            )
        ),
    }
    Ok(())
}
