use {
    crate::{
        create_vm_instance, handle_submessages, load_program, new_after_tx_event,
        new_before_tx_event, AppError, AppResult, Vm, ACCOUNTS, CHAIN_ID,
    },
    grug_types::{BlockInfo, Context, Event, Storage, Tx},
    tracing::{debug, warn},
};

// --------------------------------- before tx ---------------------------------

pub fn do_before_tx<VM>(
    storage: Box<dyn Storage>,
    block: &BlockInfo,
    tx: &Tx,
) -> AppResult<Vec<Event>>
where
    VM: Vm,
    AppError: From<VM::Error>,
{
    match _do_before_tx::<VM>(storage, block, tx) {
        Ok(events) => {
            // TODO: add txhash here?
            debug!(
                sender = tx.sender.to_string(),
                "Called before transaction hook"
            );
            Ok(events)
        },
        Err(err) => {
            warn!(
                err = err.to_string(),
                "Failed to call before transaction hook"
            );
            Err(err)
        },
    }
}

fn _do_before_tx<VM>(storage: Box<dyn Storage>, block: &BlockInfo, tx: &Tx) -> AppResult<Vec<Event>>
where
    VM: Vm,
    AppError: From<VM::Error>,
{
    let chain_id = CHAIN_ID.load(&storage)?;
    let account = ACCOUNTS.load(&storage, &tx.sender)?;

    let program = load_program::<VM>(&storage, &account.code_hash)?;
    let instance = create_vm_instance::<VM>(storage.clone(), block.clone(), &tx.sender, program)?;

    // call `before_tx` entry point
    let ctx = Context {
        chain_id,
        block_height: block.height,
        block_timestamp: block.timestamp,
        block_hash: block.hash.clone(),
        contract: tx.sender.clone(),
        sender: None,
        funds: None,
        simulate: Some(false),
    };
    let resp = instance.call_before_tx(&ctx, tx)?.into_std_result()?;

    // handle submessages
    let mut events = vec![new_before_tx_event(&ctx.contract, resp.attributes)];
    events.extend(handle_submessages::<VM>(
        storage,
        block,
        &ctx.contract,
        resp.submsgs,
    )?);

    Ok(events)
}

// --------------------------------- after tx ----------------------------------

pub fn do_after_tx<VM>(
    storage: Box<dyn Storage>,
    block: &BlockInfo,
    tx: &Tx,
) -> AppResult<Vec<Event>>
where
    VM: Vm,
    AppError: From<VM::Error>,
{
    match _do_after_tx::<VM>(storage, block, tx) {
        Ok(events) => {
            // TODO: add txhash here?
            debug!(
                sender = tx.sender.to_string(),
                "Called after transaction hook"
            );
            Ok(events)
        },
        Err(err) => {
            warn!(
                err = err.to_string(),
                "Failed to call after transaction hook"
            );
            Err(err)
        },
    }
}

fn _do_after_tx<VM>(storage: Box<dyn Storage>, block: &BlockInfo, tx: &Tx) -> AppResult<Vec<Event>>
where
    VM: Vm,
    AppError: From<VM::Error>,
{
    let chain_id = CHAIN_ID.load(&storage)?;
    let account = ACCOUNTS.load(&storage, &tx.sender)?;

    let program = load_program::<VM>(&storage, &account.code_hash)?;
    let instance = create_vm_instance::<VM>(storage.clone(), block.clone(), &tx.sender, program)?;

    // call `after_tx` entry point
    let ctx = Context {
        chain_id,
        block_height: block.height,
        block_timestamp: block.timestamp,
        block_hash: block.hash.clone(),
        contract: tx.sender.clone(),
        sender: None,
        funds: None,
        simulate: Some(false),
    };
    let resp = instance.call_after_tx(&ctx, tx)?.into_std_result()?;

    // handle submessages
    let mut events = vec![new_after_tx_event(&ctx.contract, resp.attributes)];
    events.extend(handle_submessages::<VM>(
        storage,
        block,
        &ctx.contract,
        resp.submsgs,
    )?);

    Ok(events)
}
