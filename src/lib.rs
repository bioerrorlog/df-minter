use anyhow::{Result, Context};
use candid::{
    Deserialize,
    CandidType,
};
use clap::ArgEnum;
use ic_agent::{
    agent::{
        Agent,
        http_transport::ReqwestHttpReplicaV2Transport,
    },
    identity::BasicIdentity,
};
use std::{
    collections::HashMap,
    env,
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Clone, ArgEnum)]
pub enum Network {
    /// The mainnet at <https://ic0.app/>.
    Ic,
    /// The local replica at <http://localhost:8000/>.
    Local,
}

#[derive(CandidType, Deserialize, PartialEq)]
pub enum InterfaceId {
    Approval,
    TransactionHistory,
    Mint,
    Burn,
    TransferNotification,
}

#[derive(CandidType)]
#[allow(dead_code)]
pub enum MetadataVal {
    TextContent(String),
    BlobContent(Vec<u8>),
    NatContent(u128),
    Nat8Content(u8),
    Nat16Content(u16),
    Nat32Content(u32),
    Nat64Content(u64),
}

#[derive(CandidType)]
pub struct MetadataPart<'a> {
    pub purpose: MetadataPurpose,
    pub key_val_data: HashMap<&'static str, MetadataVal>,
    pub data: &'a [u8],
}

#[derive(CandidType)]
#[allow(dead_code)]
pub enum MetadataPurpose {
    Preview,
    Rendered,
}

#[derive(Deserialize)]
pub struct DefaultIdentity {
    default: String,
}

#[derive(CandidType, Deserialize)]
pub struct MintReceipt {
    pub id: u128,
    pub token_id: u64,
}

#[derive(CandidType, Deserialize, thiserror::Error, Debug)]
pub enum MintError {
    #[error("You aren't authorized as a custodian of that canister.")]
    Unauthorized,
}

pub async fn get_agent(network: Network) -> Result<Agent> {
    let url = match network {
        Network::Local => "http://localhost:8000",
        Network::Ic => "https://ic0.app",
    };
    let user_home = env::var_os("HOME").unwrap();
    let file = File::open(Path::new(&user_home).join(".config/dfx/identity.json"))
        .context("Configure an identity in `dfx` or provide an --identity flag")?;
    let default: DefaultIdentity = serde_json::from_reader(file)?;
    let pemfile = PathBuf::from_iter([
        &*user_home,
        ".config/dfx/identity/".as_ref(),
        default.default.as_ref(),
        "identity.pem".as_ref(),
    ]);
    let identity = BasicIdentity::from_pem_file(pemfile)?;
    let agent = Agent::builder()
        .with_transport(ReqwestHttpReplicaV2Transport::create(url)?)
        .with_identity(identity)
        .build()?;
    if let Network::Local = network {
        agent.fetch_root_key().await?;
    }
    Ok(agent)
}
