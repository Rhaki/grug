use {
    crate::{Borsh, Bound, Encoding, MapKey, Proto, RawBound, RawKey},
    borsh::BorshDeserialize,
    grug_types::{
        concat, extend_one_byte, from_borsh_slice, from_proto_slice, increment_last_byte,
        nested_namespaces_with_key, trim, Order, StdResult, Storage,
    },
    prost::Message,
    std::marker::PhantomData,
};

pub struct Prefix<K, T, E: Encoding = Borsh> {
    prefix: Vec<u8>,
    suffix: PhantomData<K>,
    data: PhantomData<T>,
    encoding: PhantomData<E>,
}

impl<K, T, E> Prefix<K, T, E>
where
    E: Encoding,
{
    pub fn new(namespace: &[u8], prefixes: &[RawKey]) -> Self {
        Self {
            prefix: nested_namespaces_with_key(Some(namespace), prefixes, <Option<&RawKey>>::None),
            suffix: PhantomData,
            data: PhantomData,
            encoding: PhantomData,
        }
    }
}

impl<K, T, E> Prefix<K, T, E>
where
    K: MapKey,
    E: Encoding,
{
    #[allow(clippy::type_complexity)]
    pub fn range_raw<'a>(
        &self,
        storage: &'a dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = (Vec<u8>, Vec<u8>)> + 'a> {
        // compute start and end bounds
        // note that the store considers the start bounds as inclusive, and end
        // bound as exclusive (see the Storage trait)
        let (min, max) = range_bounds(&self.prefix, min, max);

        // need to make a clone of self.prefix and move it into the closure,
        // so that the iterator can live longer than &self.
        let prefix = self.prefix.clone();
        let iter = storage
            .scan(Some(&min), Some(&max), order)
            .map(move |(k, v)| {
                debug_assert_eq!(&k[0..prefix.len()], prefix, "prefix mispatch");
                (trim(&prefix, &k), v)
            });

        Box::new(iter)
    }

    pub fn keys_raw<'a>(
        &self,
        storage: &'a dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = Vec<u8>> + 'a> {
        let (min, max) = range_bounds(&self.prefix, min, max);
        let prefix = self.prefix.clone();
        // TODO: this is really inefficient because the host needs to load both
        // the key and value into Wasm memory. we should create a `scan_keys`
        // import that only loads the keys.
        let iter = storage
            .scan(Some(&min), Some(&max), order)
            .map(move |(k, _)| {
                debug_assert_eq!(&k[0..prefix.len()], prefix, "prefix mispatch");
                trim(&prefix, &k)
            });

        Box::new(iter)
    }

    pub fn keys<'a>(
        &self,
        storage: &'a dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<K::Output>> + 'a> {
        let iter = self
            .keys_raw(storage, min, max, order)
            .map(|key_raw| K::deserialize(&key_raw));

        Box::new(iter)
    }

    pub fn clear(
        &self,
        _storage: &mut dyn Storage,
        _min: Option<Bound<K>>,
        _max: Option<Bound<K>>,
        _limit: Option<usize>,
    ) {
        todo!() // TODO: implement this after we're added a `remove_range` method to Storage
    }
}

impl<K, T> Prefix<K, T, Borsh>
where
    K: MapKey,
    T: BorshDeserialize,
{
    #[allow(clippy::type_complexity)]
    pub fn range<'a>(
        &self,
        storage: &'a dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'a> {
        let iter = self
            .range_raw(storage, min, max, order)
            .map(|(key_raw, value_raw)| {
                let key = K::deserialize(&key_raw)?;
                let value = from_borsh_slice(value_raw)?;
                Ok((key, value))
            });

        Box::new(iter)
    }
}

impl<K, T> Prefix<K, T, Proto>
where
    K: MapKey,
    T: Message + Default,
{
    #[allow(clippy::type_complexity)]
    pub fn range<'a>(
        &self,
        storage: &'a dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'a> {
        let iter = self
            .range_raw(storage, min, max, order)
            .map(|(key_raw, value_raw)| {
                let key = K::deserialize(&key_raw)?;
                let value = from_proto_slice(value_raw)?;
                Ok((key, value))
            });

        Box::new(iter)
    }
}

fn range_bounds<K: MapKey>(
    prefix: &[u8],
    min: Option<Bound<K>>,
    max: Option<Bound<K>>,
) -> (Vec<u8>, Vec<u8>) {
    let min = match min.map(RawBound::from) {
        None => prefix.to_vec(),
        Some(RawBound::Inclusive(k)) => concat(prefix, &k),
        Some(RawBound::Exclusive(k)) => extend_one_byte(concat(prefix, &k)),
    };
    let max = match max.map(RawBound::from) {
        None => increment_last_byte(prefix.to_vec()),
        Some(RawBound::Inclusive(k)) => extend_one_byte(concat(prefix, &k)),
        Some(RawBound::Exclusive(k)) => concat(prefix, &k),
    };

    (min, max)
}
