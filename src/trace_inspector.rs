use std::cell::RefCell;
use std::rc::Rc;

use alloy::providers::Provider;
use revm::Context;
use revm::Inspector;
use revm::context::{Block, Cfg, ContextTr, CreateScheme, JournalTr, Transaction};
use revm::database::State;
use revm::handler::PrecompileProvider;
use revm::handler::instructions::InstructionProvider;
use revm::interpreter::interpreter::EthInterpreter;
use revm::interpreter::{
    CallInputs, CallOutcome, CallScheme, CallValue, CreateInputs, CreateOutcome, InterpreterResult,
};
use revm::primitives::{Address, B256, U256};
use serde::Serialize;

use hex;

use crate::types::MyDb;

#[derive(Serialize, Clone)]
pub struct CallInfo {
    tx_hash: B256,
    from_addr: Address,
    to_addr: Address,
    storage_addr: Address,
    value: U256,
    action: String,
    calldata: String,
}

#[derive(Default)]
pub struct TraceInspector<BLOCK, TX, CFG, I, P> {
    pub traces: Rc<RefCell<Vec<CallInfo>>>,
    cur_tx_hash: B256,
    phantom: core::marker::PhantomData<(BLOCK, TX, CFG, I, P)>,
}

pub trait InspectorHelper {
    fn set_tx(&mut self, hash: B256);
}

impl<BLOCK, TX, CFG, I, P> InspectorHelper for TraceInspector<BLOCK, TX, CFG, I, P> {
    fn set_tx(&mut self, hash: B256) {
        self.cur_tx_hash = hash;
    }
}

fn call_scheme_to_string(scheme: CallScheme) -> String {
    match scheme {
        CallScheme::Call => "call",
        CallScheme::CallCode => "call_code",
        CallScheme::DelegateCall => "delegate_call",
        CallScheme::StaticCall => "static_call",
        CallScheme::ExtCall => "ext_call",
        CallScheme::ExtDelegateCall => "ext_delegate_call",
        CallScheme::ExtStaticCall => "ext_static_call",
    }
    .into()
}

fn create_scheme_to_string(scheme: CreateScheme) -> String {
    match scheme {
        CreateScheme::Create => "create",
        CreateScheme::Create2 { .. } => "create2",
    }
    .into()
}

impl<PR, BLOCK, TX, CFG, I, P> Inspector<Context<BLOCK, TX, CFG, State<MyDb<PR>>>>
    for TraceInspector<BLOCK, TX, CFG, I, P>
where
    PR: Provider,
    BLOCK: Block + Clone,
    TX: Transaction + Clone,
    CFG: Cfg + Clone,
    I: InstructionProvider<
            Context = Context<BLOCK, TX, CFG, State<MyDb<PR>>>,
            InterpreterTypes = EthInterpreter,
        >,
    P: PrecompileProvider<Context<BLOCK, TX, CFG, State<MyDb<PR>>>, Output = InterpreterResult>,
{
    fn call(
        &mut self,
        _: &mut Context<BLOCK, TX, CFG, State<MyDb<PR>>>,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        let value = match inputs.value {
            CallValue::Transfer(v) => v,
            CallValue::Apparent(v) => v,
        };

        let trace = CallInfo {
            tx_hash: self.cur_tx_hash,
            from_addr: inputs.caller,
            storage_addr: inputs.target_address,
            to_addr: inputs.bytecode_address,
            action: call_scheme_to_string(inputs.scheme),
            value,
            calldata: hex::encode(inputs.input.as_ref()),
        };

        self.traces.borrow_mut().push(trace);
        None
    }
    fn create(
        &mut self,
        context: &mut Context<BLOCK, TX, CFG, State<MyDb<PR>>>,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        let r = context.journal().load_account(inputs.caller);
        let nonce: u64;
        if let Ok(acc) = r {
            nonce = acc.data.info.nonce;
        } else {
            panic!("err while loading account, idk what to do with it now (TODO!)");
        }

        let contract_addr = inputs.created_address(nonce);
        let trace = CallInfo {
            tx_hash: self.cur_tx_hash,
            from_addr: inputs.caller,
            to_addr: contract_addr,
            storage_addr: contract_addr,
            action: create_scheme_to_string(inputs.scheme),
            value: inputs.value,
            calldata: "".into(),
        };

        self.traces.borrow_mut().push(trace);

        None
    }
}
