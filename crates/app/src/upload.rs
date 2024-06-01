use {
    crate::{has_permission, new_upload_event, AppError, AppResult, CODES, CONFIG},
    grug_types::{hash, Addr, Event, Hash, Storage},
    tracing::{info, warn},
};

pub fn do_upload(
    storage: &mut dyn Storage,
    uploader: &Addr,
    code: Vec<u8>,
) -> AppResult<Vec<Event>> {
    match _do_upload(storage, uploader, code) {
        Ok((events, code_hash)) => {
            info!(code_hash = code_hash.to_string(), "Stored code");
            Ok(events)
        },
        Err(err) => {
            warn!(err = err.to_string(), "Failed to storage code");
            Err(err)
        },
    }
}

// return the hash of the code that is stored, for purpose of tracing/logging
fn _do_upload(
    storage: &mut dyn Storage,
    uploader: &Addr,
    code: Vec<u8>,
) -> AppResult<(Vec<Event>, Hash)> {
    // make sure the user has permission to storage code
    let cfg = CONFIG.load(storage)?;
    if !has_permission(&cfg.permissions.upload, cfg.owner.as_ref(), uploader) {
        return Err(AppError::Unauthorized);
    }

    // make sure that the same code isn't uploaded twice
    let code_hash = hash(&code);
    if CODES.has(storage, &code_hash) {
        return Err(AppError::code_exists(code_hash));
    }

    // TODO: deserialize the code to make sure it's a valid program?

    // storage the code
    CODES.save(storage, &code_hash, &code)?;

    Ok((vec![new_upload_event(&code_hash, uploader)], code_hash))
}
