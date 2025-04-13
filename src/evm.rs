use std::cell::RefCell;
use std::rc::Rc;

use alloy::eips::BlockId;
use alloy::rpc::types::Block;
use revm::context::{Cfg, Evm, Transaction, TxEnv};
use revm::handler::instructions::{EthInstructions, InstructionProvider};
use revm::handler::{EthPrecompiles, PrecompileProvider};
use revm::interpreter::InterpreterResult;
use revm::interpreter::interpreter::EthInterpreter;
use revm::primitives::B256;
use revm::{
    Context, MainBuilder, MainContext,
    database::{AlloyDB, CacheDB, StateBuilder},
    database_interface::WrapDatabaseAsync,
};

use revm::context::Block as RevmBlock;

use revm::{Database, DatabaseCommit, InspectCommitEvm, Inspector};

use crate::trace_inspector::{CallInfo, InspectorHelper, TraceInspector};

use alloy::providers::Provider;

pub trait Helper<TX>: InspectCommitEvm {
    fn modify_tx<F>(&mut self, f: F, hash: B256)
    where
        F: FnOnce(&mut TX);

    fn my_inspect(&mut self) -> Result<(), String>;
}

impl<TX, INSP, I, P, BLOCK, CFG, DB> Helper<TX> for Evm<Context<BLOCK, TX, CFG, DB>, INSP, I, P>
where
    TX: Transaction + Default,
    BLOCK: RevmBlock + Default,
    CFG: Cfg + Default,
    DB: Database + DatabaseCommit,
    P: PrecompileProvider<Context<BLOCK, TX, CFG, DB>, Output = InterpreterResult>,
    I: InstructionProvider<
            Context = Context<BLOCK, TX, CFG, DB>,
            InterpreterTypes = EthInterpreter,
        >,
    INSP: Inspector<Context<BLOCK, TX, CFG, DB>> + InspectorHelper,
{
    fn modify_tx<F>(&mut self, f: F, hash: B256)
    where
        F: FnOnce(&mut TX),
    {
        f(&mut self.ctx.tx);
        self.inspector.set_tx(hash);
    }

    fn my_inspect(&mut self) -> Result<(), String> {
        let result = self.inspect_replay_commit();
        if result.is_ok() {
            return Ok(());
        }

        Err("fuck".into())
    }
}

pub fn create_evm<P>(
    client: P,
    state_block_id: BlockId,
    block: Block,
    chain_id: u64,
    traces: Rc<RefCell<Vec<CallInfo>>>,
) -> impl Helper<TxEnv>
where
    P: Provider,
{
    let state_db = WrapDatabaseAsync::new(AlloyDB::new(client, state_block_id)).unwrap();
    let cache_db: CacheDB<_> = CacheDB::new(state_db);
    let state = StateBuilder::new_with_database(cache_db).build();
    let ctx = Context::mainnet()
        .with_db(state)
        .modify_block_chained(|b| {
            b.number = block.header.number;
            b.beneficiary = block.header.beneficiary;
            b.timestamp = block.header.timestamp;

            b.difficulty = block.header.difficulty;
            b.gas_limit = block.header.gas_limit;
            b.basefee = block.header.base_fee_per_gas.unwrap_or_default();
        })
        .modify_cfg_chained(|c| {
            c.chain_id = chain_id;
        });

    let mut inspector = TraceInspector::<_, _, _, EthInstructions<_, _>, EthPrecompiles>::default();
    inspector.traces = traces.clone();

    ctx.build_mainnet_with_inspector(inspector)
}
