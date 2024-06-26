use {
    crate::{Borsh, Bound, Encoding, MapKey, PathBuf, Prefix, Proto},
    borsh::{BorshDeserialize, BorshSerialize},
    grug_types::{Order, StdError, StdResult, Storage},
    prost::Message,
    std::marker::PhantomData,
};

pub struct Map<'a, K, T, E: Encoding = Borsh> {
    namespace: &'a [u8],
    key: PhantomData<K>,
    data: PhantomData<T>,
    encoding: PhantomData<E>,
}

impl<'a, K, T> Map<'a, K, T> {
    pub const fn new(namespace: &'a str) -> Self {
        // TODO: add a maximum length for namespace
        // see comments of increment_last_byte function for rationale
        Self {
            namespace: namespace.as_bytes(),
            key: PhantomData,
            data: PhantomData,
            encoding: PhantomData,
        }
    }
}

impl<'a, K, T, E> Map<'a, K, T, E>
where
    K: MapKey,
    E: Encoding,
{
    fn path(&self, key: K) -> PathBuf<T, E> {
        let mut raw_keys = key.raw_keys();
        let last_raw_key = raw_keys.pop();
        PathBuf::new(self.namespace, &raw_keys, last_raw_key.as_ref())
    }

    fn no_prefix(&self) -> Prefix<K, T, E> {
        Prefix::new(self.namespace, &[])
    }

    pub fn prefix(&self, prefix: K::Prefix) -> Prefix<K::Suffix, T, E> {
        Prefix::new(self.namespace, &prefix.raw_keys())
    }

    pub fn is_empty(&self, storage: &dyn Storage) -> bool {
        self.keys_raw(storage, None, None, Order::Ascending)
            .next()
            .is_none()
    }

    pub fn has(&self, storage: &dyn Storage, k: K) -> bool {
        self.path(k).as_path().exists(storage)
    }

    pub fn remove(&self, storage: &mut dyn Storage, k: K) {
        self.path(k).as_path().remove(storage)
    }

    #[allow(clippy::type_complexity)]
    pub fn range_raw<'b>(
        &self,
        storage: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = (Vec<u8>, Vec<u8>)> + 'b> {
        self.no_prefix().range_raw(storage, min, max, order)
    }

    pub fn keys_raw<'b>(
        &self,
        storage: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = Vec<u8>> + 'b> {
        self.no_prefix().keys_raw(storage, min, max, order)
    }

    pub fn keys<'b>(
        &self,
        storage: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<K::Output>> + 'b> {
        self.no_prefix().keys(storage, min, max, order)
    }

    pub fn clear(
        &self,
        storage: &mut dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        limit: Option<usize>,
    ) {
        self.no_prefix().clear(storage, min, max, limit)
    }
}

// ----------------------------------- borsh -----------------------------------

impl<'a, K, T> Map<'a, K, T, Borsh>
where
    K: MapKey,
    T: BorshSerialize,
{
    pub fn save(&self, storage: &mut dyn Storage, k: K, data: &T) -> StdResult<()> {
        self.path(k).as_path().save(storage, data)
    }
}

impl<'a, K, T> Map<'a, K, T, Borsh>
where
    K: MapKey,
    T: BorshDeserialize,
{
    pub fn may_load(&self, storage: &dyn Storage, k: K) -> StdResult<Option<T>> {
        self.path(k).as_path().may_load(storage)
    }

    pub fn load(&self, storage: &dyn Storage, k: K) -> StdResult<T> {
        self.path(k).as_path().load(storage)
    }

    #[allow(clippy::type_complexity)]
    pub fn range<'b>(
        &self,
        storage: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'b> {
        self.no_prefix().range(storage, min, max, order)
    }
}

impl<'a, K, T> Map<'a, K, T, Borsh>
where
    K: MapKey,
    T: BorshSerialize + BorshDeserialize,
{
    pub fn update<A, E>(&self, storage: &mut dyn Storage, k: K, action: A) -> Result<Option<T>, E>
    where
        A: FnOnce(Option<T>) -> Result<Option<T>, E>,
        E: From<StdError>,
    {
        self.path(k).as_path().update(storage, action)
    }
}

// ----------------------------------- proto -----------------------------------

impl<'a, K, T> Map<'a, K, T, Proto>
where
    K: MapKey,
    T: Message,
{
    pub fn save(&self, storage: &mut dyn Storage, k: K, data: &T) {
        self.path(k).as_path().save(storage, data)
    }
}

impl<'a, K, T> Map<'a, K, T, Proto>
where
    K: MapKey,
    T: Message + Default,
{
    pub fn may_load(&self, storage: &dyn Storage, k: K) -> StdResult<Option<T>> {
        self.path(k).as_path().may_load(storage)
    }

    pub fn load(&self, storage: &dyn Storage, k: K) -> StdResult<T> {
        self.path(k).as_path().load(storage)
    }

    pub fn update<A, E>(&self, storage: &mut dyn Storage, k: K, action: A) -> Result<Option<T>, E>
    where
        A: FnOnce(Option<T>) -> Result<Option<T>, E>,
        E: From<StdError>,
    {
        self.path(k).as_path().update(storage, action)
    }

    #[allow(clippy::type_complexity)]
    pub fn range<'b>(
        &self,
        storage: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'b> {
        self.no_prefix().range(storage, min, max, order)
    }
}
