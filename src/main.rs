//! Optimism-specific constants, types, and helpers.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod evm;
mod trace_inspector;
mod types;

use std::cell::RefCell;
use std::rc::Rc;

use alloy::consensus::Transaction;
use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::providers::{Provider, ProviderBuilder, network::primitives::BlockTransactions};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing::get};
use revm::primitives::TxKind;

use evm::Helper;
use evm::create_evm;
use trace_inspector::CallInfo;

///////////////////////////////////////////////////////////////////////////////////////////////////////

// TODO: remove this hardcode eventually
static URL: &str = "https://eth-mainnet.g.alchemy.com/v2/_mSXeLBO_vTQ9KMdPQ0LkRryUY0sdvtM";
static CHAIN_ID: u64 = 1;

// TODO: better logging
async fn trace_block_impl(
    url: &str,
    block_number: u64,
    chain_id: u64,
) -> anyhow::Result<Vec<CallInfo>> {
    // Set up the HTTP transport which is consumed by the RPC client.
    let rpc_url = url.parse()?;

    // Create a provider
    let client = ProviderBuilder::new().on_http(rpc_url);

    // Fetch the transaction-rich block
    let block = match client
        .get_block_by_number(BlockNumberOrTag::Number(block_number))
        .full()
        .await
    {
        Ok(Some(block)) => block,
        Ok(None) => anyhow::bail!("Block not found"),
        Err(error) => anyhow::bail!("Error: {:?}", error),
    };
    println!("Fetched block number: {}", block.header.number);
    let previous_block_number = block_number - 1;

    let state_block_id: BlockId = previous_block_number.into();

    let traces = Rc::new(RefCell::new(Vec::new()));
    let mut evm = create_evm(
        client,
        state_block_id,
        block.clone(),
        chain_id,
        traces.clone(),
    );

    let txs: usize = block.transactions.len();
    println!("Processing {} transactions", txs);

    let BlockTransactions::Full(transactions) = block.transactions else {
        panic!("Wrong transaction type")
    };

    for tx in transactions {
        let tx_hash = tx.as_ref().hash();
        println!("processing tx = {}", tx_hash);

        evm.modify_tx(
            |etx| {
                etx.caller = tx.inner.signer();
                etx.gas_limit = tx.gas_limit();
                etx.gas_price = tx.gas_price().unwrap_or(tx.inner.max_fee_per_gas());
                etx.value = tx.value();
                etx.data = tx.input().to_owned();
                etx.gas_priority_fee = tx.max_priority_fee_per_gas();
                etx.chain_id = Some(chain_id);
                etx.nonce = tx.nonce();
                if let Some(access_list) = tx.access_list() {
                    etx.access_list = access_list.clone()
                } else {
                    etx.access_list = Default::default();
                }

                etx.kind = match tx.to() {
                    Some(to_address) => TxKind::Call(to_address),
                    None => TxKind::Create,
                };
            },
            *tx_hash,
        );

        // Inspect and commit the transaction to the EVM
        let res = evm.my_inspect();
        if let Err(s) = res {
            println!("{}", s);
        }
    }

    println!("done!");

    let result = (*traces.borrow()).clone();
    Ok(result)
}

struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

async fn trace_block(Path(block_number): Path<u64>) -> Result<Json<Vec<CallInfo>>, AppError> {
    let result = trace_block_impl(URL, block_number, CHAIN_ID).await;

    match result {
        Ok(traces) => Ok(Json(traces)),
        Err(e) => Err(AppError(e)),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Router::new().route("/trace_block/{block_number}", get(trace_block));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
    Ok(())
}
