use alloy::network::Ethereum;
use revm::{
    database::{AlloyDB, CacheDB},
    database_interface::WrapDatabaseAsync,
};

pub type MyDb<P> = CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, P>>>;
