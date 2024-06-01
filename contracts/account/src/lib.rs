#[cfg(not(feature = "library"))]
use grug::grug_export;
use {
    anyhow::bail,
    grug::{
        grug_derive, split_one_key, to_json_value, to_json_vec, Addr, AuthCtx, Binary,
        ImmutableCtx, Incrementor, Item, Json, MapKey, Message, MutableCtx, RawKey, Response,
        StdError, StdResult, Tx,
    },
    sha2::{Digest, Sha256},
};

const PUBLIC_KEY: Item<PublicKey> = Item::new("pk");
const SEQUENCE: Incrementor<u32> = Incrementor::new("seq");

#[grug_derive(serde)]
pub struct InstantiateMsg {
    pub public_key: PublicKey,
}

#[grug_derive(serde)]
pub enum ExecuteMsg {
    // not execute method is available with this contract.
    //
    // ideally we want to allow the account to update its public key. however
    // adding this capability breaks the account factory contract, which
    // maintains a registry of accounts indexed by (public_key, serial).
    // if the account updates its public key, it needs to report it to the
    // factory to get its record in the registry updated. this is doable but
    // adds quite some complexity. also, maybe not that many users will need to
    // rotate keys after all... so for now we don't have plan to support key
    // rotation.
}

#[grug_derive(serde)]
pub enum QueryMsg {
    /// Query the state of the account, including its public key and sequence.
    /// Returns: StateResponse
    State {},
}

#[grug_derive(serde)]
pub struct StateResponse {
    pub public_key: PublicKey,
    pub sequence: u32,
}

#[grug_derive(serde, borsh)]
#[derive(Hash)]
pub enum PublicKey {
    Secp256k1(Binary),
    Secp256r1(Binary),
}

// implement MapKey trait, so that in account factory it can use the public key
// as a map key.
impl<'a> MapKey for &'a PublicKey {
    type Output = PublicKey;
    type Prefix = ();
    type Suffix = ();

    fn raw_keys(&self) -> Vec<RawKey> {
        let (ty, bytes) = match self {
            PublicKey::Secp256k1(bytes) => ("secp256k1", bytes),
            PublicKey::Secp256r1(bytes) => ("secp256r1", bytes),
        };
        vec![RawKey::Ref(ty.as_bytes()), RawKey::Ref(bytes)]
    }

    fn deserialize(bytes: &[u8]) -> StdResult<Self::Output> {
        let (ty_bytes, bytes) = split_one_key(bytes);
        match ty_bytes {
            b"secp256k1" => {
                if bytes.len() != 33 {
                    return Err(StdError::deserialize::<PublicKey>(
                        "incorrect secp256k1 public key length",
                    ));
                }
                Ok(PublicKey::Secp256k1(bytes.to_vec().into()))
            },
            b"secp256r1" => {
                if bytes.len() != 33 {
                    return Err(StdError::deserialize::<PublicKey>(
                        "incorrect secp256r1 public key length",
                    ));
                }
                Ok(PublicKey::Secp256r1(bytes.to_vec().into()))
            },
            _ => Err(StdError::deserialize::<PublicKey>(format!(
                "unknown public key type: {ty_bytes:?}"
            ))),
        }
    }
}

/// Given details of a transaction, produce the bytes that the sender needs to
/// sign (hashed).
///
/// The bytes are defined as:
///
/// ```plain
/// bytes := blake3(json(msgs) | sender_addr | chain_id | sequence)
/// ```
///
/// where:
/// - `sender_addr` is a 32 bytes address of the sender;
/// - `chain_id` is the chain ID in UTF-8 encoding;
/// - `sequence` is the sender account's sequence in 32-bit big endian encoding.
///
/// TODO: json here is ambiguous, i.e. what padding and linebreak character to
/// use, the order of fields... elaborate it.
///
/// TODO: is it efficient to do hashing in the contract? maybe move this to the
/// host??
pub fn sign_bytes(
    msgs: &[Message],
    sender: &Addr,
    chain_id: &str,
    sequence: u32,
) -> anyhow::Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    hasher.update(&to_json_vec(&msgs)?);
    hasher.update(sender);
    hasher.update(chain_id.as_bytes());
    hasher.update(sequence.to_be_bytes());
    Ok(hasher.finalize().into())
}

#[cfg_attr(not(feature = "library"), grug_export)]
pub fn instantiate(ctx: MutableCtx, msg: InstantiateMsg) -> anyhow::Result<Response> {
    // Save the public key in contract store
    PUBLIC_KEY.save(ctx.storage, &msg.public_key)?;

    // Initialize the sequence number to zero
    SEQUENCE.initialize(ctx.storage)?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), grug_export)]
pub fn receive(ctx: MutableCtx) -> anyhow::Result<Response> {
    // do nothing, accept all transfers. log the receipt to events
    Ok(Response::new()
        .add_attribute("method", "receive")
        .add_attribute("sender", ctx.sender)
        .add_attribute("funds", ctx.funds.to_string()))
}

#[cfg_attr(not(feature = "library"), grug_export)]
pub fn before_tx(ctx: AuthCtx, tx: Tx) -> anyhow::Result<Response> {
    let public_key = PUBLIC_KEY.load(ctx.storage)?;
    let sequence = SEQUENCE.load(ctx.storage)?;

    // prepare the hash that is expected to have been signed
    let msg_hash = sign_bytes(&tx.msgs, &tx.sender, &ctx.chain_id, sequence)?;

    // verify the signature
    // skip if we are in simulate mode
    if !ctx.simulate {
        match &public_key {
            PublicKey::Secp256k1(bytes) => {
                ctx.secp256k1_verify(&msg_hash, &tx.credential, bytes)?;
            },
            PublicKey::Secp256r1(bytes) => {
                ctx.secp256r1_verify(&msg_hash, &tx.credential, bytes)?;
            },
        }
    }

    // increment the sequence number
    SEQUENCE.increment(ctx.storage)?;

    Ok(Response::new()
        .add_attribute("method", "before_tx")
        .add_attribute("next_sequence", sequence.to_string()))
}

#[cfg_attr(not(feature = "library"), grug_export)]
pub fn after_tx(_ctx: AuthCtx, _tx: Tx) -> anyhow::Result<Response> {
    // nothing to do
    Ok(Response::new().add_attribute("method", "after_tx"))
}

#[cfg_attr(not(feature = "library"), grug_export)]
pub fn execute(_ctx: MutableCtx, _msg: ExecuteMsg) -> anyhow::Result<Response> {
    bail!("no execute method is available for this contract");
}

#[cfg_attr(not(feature = "library"), grug_export)]
pub fn query(ctx: ImmutableCtx, msg: QueryMsg) -> StdResult<Json> {
    match msg {
        QueryMsg::State {} => to_json_value(&query_state(ctx)?),
    }
}

pub fn query_state(ctx: ImmutableCtx) -> StdResult<StateResponse> {
    Ok(StateResponse {
        public_key: PUBLIC_KEY.load(ctx.storage)?,
        sequence: SEQUENCE.load(ctx.storage)?,
    })
}
