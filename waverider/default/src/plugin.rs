use async_std::os::unix::net::UnixDatagram;
use hyper::body::Buf;
use postgrest::Postgrest;
use pyth_sdk_solana::{self, load_price_feed_from_account_info};
use serde::Deserialize;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
use solana_geyser_plugin_interface::geyser_plugin_interface::{
    GeyserPlugin, ReplicaAccountInfoVersions, Result as PluginResult,
};

use solana_geyser_plugin_interface::geyser_plugin_interface::{
    GeyserPluginError, ReplicaAccountInfo,
};

use solana_account_decoder::UiAccountEncoding;

use solana_client::rpc_client::RpcClient;
use solana_program::account_info::AccountInfo;
use solana_program::pubkey::Pubkey;
use solana_program::{borsh, pubkey};
use solana_sdk::client;
use solana_sdk::commitment_config::CommitmentConfig;

use std::{
    error::Error,
    fmt::{self, Debug},
    fs::OpenOptions,
    io::Read,
};
use tokio::runtime::Runtime;

pub struct SupabasePlugin {
    postgres_client: Option<Postgrest>,
    configuration: Option<Configuration>,
    accounts: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub supabase_url: String,
    pub supabase_key: String,
}

impl Configuration {
    pub fn load(config_path: &str) -> Result<Self, Box<dyn Error>> {
        let mut file = OpenOptions::new().read(true).open(config_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        Ok(serde_json::from_str::<Configuration>(&contents)?)
    }
}

impl Default for SupabasePlugin {
    fn default() -> Self {
        SupabasePlugin {
            postgres_client: None,
            configuration: None,
            accounts: Vec::new(),
        }
    }
}

impl GeyserPlugin for SupabasePlugin {
    fn name(&self) -> &'static str {
        "supabase-geyser"
    }

    fn on_load(&mut self, config_file: &str) -> PluginResult<()> {
        // todo: get trigger account bytes
        // let TRIGGER_ACCOUNT_DISCRIMINATOR: Vec<u8> = vec![77, 155, 35, 144, 38, 14, 106, 88];

        // let connection = RpcClient::new_with_commitment(
        //     "https://global.rpc.hellomoon.io/5d77d2b5-8b58-4131-95d7-ba9c59499a1c".to_string(),
        //     CommitmentConfig::confirmed(),
        // );

        // let filters = Some(vec![RpcFilterType::Memcmp(Memcmp {
        //     offset: 0,
        //     bytes: MemcmpEncodedBytes::Bytes(TRIGGER_ACCOUNT_DISCRIMINATOR),
        //     encoding: None,
        // })]);

        // let accounts = connection
        //     .get_program_accounts_with_config(
        //         &pubkey!("41NuR2mieT98yDQpXmwDzBZ24sz9UMAieorCr8Mw9C8Q"),
        //         RpcProgramAccountsConfig {
        //             with_context: Some(false),
        //             filters,
        //             account_config: RpcAccountInfoConfig {
        //                 commitment: Some(connection.commitment()),
        //                 encoding: Some(UiAccountEncoding::Base64),
        //                 ..RpcAccountInfoConfig::default()
        //             },
        //         },
        //     )
        //     .unwrap();

        // println!("TriggrAccounts: {:#?}", accounts[0]);

        println!("config file: {}", config_file);
        let mut config = match Configuration::load(config_file) {
            Ok(c) => c,
            Err(_e) => {
                return Err(GeyserPluginError::ConfigFileReadError {
                    msg: String::from("Error opening, or reading config file"),
                });
            }
        };

        println!("Your supabase url: {:#?} ", &config.supabase_url);
        self.postgres_client = Some(
            Postgrest::new(&config.supabase_url).insert_header("apikey", &config.supabase_key),
        );

        // self.accounts = accounts
        //     .iter()
        //     .map(|account| account.0.to_bytes())
        //     .collect();

        self.configuration = Some(config);
        Ok(())
    }

    fn on_unload(&mut self) {}

    fn update_account(
        &self,
        account: ReplicaAccountInfoVersions,
        _slot: u64,
        is_startup: bool,
    ) -> PluginResult<()> {
        let account_info = match account {
            ReplicaAccountInfoVersions::V0_0_1(_) => {
                return Err(GeyserPluginError::AccountsUpdateError {
                    msg: "V1 not supported, please upgrade your Solana CLI Version".to_string(),
                });
            }
            ReplicaAccountInfoVersions::V0_0_2(_) => {
                return Err(GeyserPluginError::AccountsUpdateError {
                    msg: "V1 not supported, please upgrade your Solana CLI Version".to_string(),
                });
            }
            ReplicaAccountInfoVersions::V0_0_3(account_info) => account_info,
        };
        let pyth_program_devnet = pubkey!("gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s");

        if pyth_program_devnet.to_bytes() == account_info.owner {
            let account_pubkey = Pubkey::try_from(account_info.pubkey).unwrap();
            let owner_pubkey = Pubkey::try_from(account_info.owner).unwrap();

            let data = &mut account_info.data.to_vec();
            let mut lamports = account_info.lamports;
            let acc_info = AccountInfo::new(
                &account_pubkey,
                false,
                false,
                &mut lamports,
                data,
                &owner_pubkey,
                account_info.executable,
                account_info.rent_epoch,
            );
            let price_feed = load_price_feed_from_account_info(&acc_info).map_err(|_| {
                GeyserPluginError::AccountsUpdateError {
                    msg: "Failed to parse Pyth price account".into(),
                }
            })?;

            println!("Price feed: {}", price_feed.get_price_unchecked().price);
        };

        // self.accounts.iter().for_each(|account| {

        //     if account == account_info.owner {
        //         let account_pubkey = bs58::encode(account_info.pubkey).into_string();
        //         let account_owner = bs58::encode(account_info.owner).into_string();
        //         let account_data = account_info.data;
        //         let _account_lamports = account_info.lamports;
        //         let account_executable = account_info.executable;
        //         let _account_rent_epoch = account_info.rent_epoch;

        //         let rt = Runtime::new().unwrap();
        //         let result = rt.block_on(
        //             self.postgres_client
        //                 .as_mut()
        //                 .unwrap()
        //                 .from("accounts")
        //                 .upsert(
        //                     serde_json::to_string(
        //                         &serde_json::json!([{ "account": account_pubkey, "owner": account_owner, "data": account_data, "executable": account_executable }]),
        //                     )
        //                     .unwrap(),
        //                 )
        //                 .execute(),
        //         );
        //         println!("result: {:#?}, startup: {:#?}", result, is_startup);
        //     } else {
        //     }
        // });

        Ok(())
    }

    fn notify_end_of_startup(&self) -> PluginResult<()> {
        Ok(())
    }

    fn account_data_notifications_enabled(&self) -> bool {
        true
    }

    fn transaction_notifications_enabled(&self) -> bool {
        false
    }
}

impl Debug for SupabasePlugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupabasePlugin")
            .field("postgres_client", &self.postgres_client.is_some())
            .finish()
    }
}
