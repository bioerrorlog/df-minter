use anyhow::{
    bail,
    Result,
    Context,
};
use candid::{Principal, Encode, Decode};
use clap::Parser;
use garcon::{Delay, Waiter};
use ic_agent::agent::AgentError;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    process,
    path::PathBuf,
    time::Duration,
};

use df_minter::{
    InterfaceId,
    MetadataPart,
    MetadataPurpose,
    MetadataVal,
    MintReceipt,
    MintError,
    Network,
    get_agent,
};

#[derive(Parser)]
struct Args {
    #[clap(arg_enum)]
    /// The network the canister is running on.
    network: Network,
    /// The DIP-721 compliant NFT container.
    canister: Principal,
    /// The owner of the new NFT.
    #[clap(long)]
    owner: Principal,
    /// The path to the file. Required if you want the file contents sent to
    /// the smart contract.
    #[clap(long)]
    file: PathBuf,  
}

#[tokio::main]
async fn main() {
    if let Err(e) = mint().await {
        eprintln!("{}", e);
        process::exit(1);
    }
}

async fn mint() -> Result<()> {
    let args = Args::parse();
    let canister = args.canister;
    let owner = args.owner;
    let agent = get_agent(args.network).await?;
    let res = agent
        .query(&canister, "supportedInterfaces")
        .with_arg(Encode!()?)
        .call()
        .await;
    let res = if let Err(AgentError::ReplicaError { reject_code: 3, .. }) = &res {
        res.context(format!(
            "canister {canister} does not appear to be a DIP-721 NFT canister"
        ))?
    } else {
        res?
    };
    let interfaces = Decode!(&res, Vec<InterfaceId>)?;
    if !interfaces.contains(&InterfaceId::Mint) {
        bail!("canister {canister} does not support minting");
    }
    let mut metadata = HashMap::new();
    metadata.insert("locationType", MetadataVal::Nat8Content(4));

    let (data, content_type) = {
        let data = fs::read(&args.file)?;
        metadata.insert(
            "contentHash",
            MetadataVal::BlobContent(Vec::from_iter(Sha256::digest(&data))),
        );
        let content_type = mime_guess::from_path(&args.file).first().map(|m| format!("{m}"));
        (data, content_type)
    };
    let content_type = content_type.unwrap_or_else(|| String::from("application/octet-stream"));
    metadata.insert("contentType", MetadataVal::TextContent(content_type));
    let metadata = MetadataPart {
        purpose: MetadataPurpose::Rendered,
        data: &data,
        key_val_data: metadata,
    };
    let waiter = get_waiter();
    let res = agent
        .update(&args.canister, "mint")
        .with_arg(Encode!(&owner, &[metadata], &data)?)
        .call_and_wait(waiter)
        .await;
    let res = if let Err(AgentError::ReplicaError { reject_code: 3, .. }) = &res {
        res.context(format!("canister {canister} does not support minting"))?
    } else {
        res?
    };
    let MintReceipt { token_id, id } = Decode!(&res, Result<MintReceipt, MintError>)??;
    println!("Successfully minted token {token_id} to {owner} (transaction id {id})");
    Ok(())
}

fn get_waiter() -> impl Waiter {
    Delay::builder()
        .throttle(Duration::from_millis(500))
        .timeout(Duration::from_secs(300))
        .build()
}
