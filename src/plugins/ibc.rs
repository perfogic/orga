use crate::call::Call as CallTrait;
use crate::client::{AsyncCall, AsyncQuery, Client};
use crate::coins::{Address, Symbol};
use crate::encoding::{Decode, Encode};
use crate::ibc::Ibc;
use crate::query::Query as QueryTrait;
use crate::state::State;
use crate::Result;
use ibc::core::ics26_routing::msgs::Ics26Envelope;
use ibc_proto::google::protobuf::Any;
use prost::Message;
use std::convert::TryFrom;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

pub struct IbcPlugin<T> {
    ibc: Ibc,
    inner: T,
}

impl<T> Deref for IbcPlugin<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for IbcPlugin<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: State> State for IbcPlugin<T> {
    type Encoding = (<Ibc as State>::Encoding, T::Encoding);

    fn create(store: crate::store::Store, data: Self::Encoding) -> Result<Self> {
        Ok(Self {
            ibc: Ibc::create(store.sub(&[0]), data.0)?,
            inner: T::create(store.sub(&[1]), data.1)?,
        })
    }

    fn flush(self) -> Result<Self::Encoding> {
        Ok((self.ibc.flush()?, self.inner.flush()?))
    }
}

impl<T: State> From<IbcPlugin<T>> for (<Ibc as State>::Encoding, T::Encoding) {
    fn from(plugin: IbcPlugin<T>) -> Self {
        (plugin.ibc.into(), plugin.inner.into())
    }
}

pub enum Call<T> {
    Inner(T),
    Ics26(Any),
}

unsafe impl<T> Send for Call<T> {}

impl<T: Encode> Encode for Call<T> {
    fn encoding_length(&self) -> ed::Result<usize> {
        match self {
            Call::Inner(inner) => inner.encoding_length(),
            Call::Ics26(envelope) => Ok(envelope.encoded_len()),
        }
    }

    fn encode_into<W: std::io::Write>(&self, dest: &mut W) -> ed::Result<()> {
        match self {
            Call::Inner(inner) => inner.encode_into(dest),
            Call::Ics26(envelope) => {
                let bytes = envelope.encode_to_vec();
                dest.write_all(bytes.as_slice())?;

                Ok(())
            }
        }
    }
}

impl<T: Decode> Decode for Call<T> {
    fn decode<R: std::io::Read>(mut reader: R) -> ed::Result<Self> {
        let mut bytes = vec![];
        reader.read_to_end(&mut bytes)?;

        let maybe_any = Any::decode(bytes.clone().as_slice());
        if let Ok(any) = maybe_any {
            if Ics26Envelope::try_from(any.clone()).is_ok() {
                Ok(Call::Ics26(any))
            } else {
                let native = T::decode(bytes.as_slice())?;
                Ok(Call::Inner(native))
            }
        } else {
            let native = T::decode(bytes.as_slice())?;
            Ok(Call::Inner(native))
        }
    }
}

impl<T> CallTrait for IbcPlugin<T>
where
    T: CallTrait + State,
    T::Call: Encode + 'static,
{
    type Call = Call<T::Call>;

    fn call(&mut self, call: Self::Call) -> Result<()> {
        match call {
            Call::Inner(native) => self.inner.call(native),
            Call::Ics26(envelope) => {
                todo!()
            }
        }
    }
}

#[derive(Encode, Decode)]
pub enum Query<T: QueryTrait> {
    Inner(T::Query),
    Ibc(<Ibc as QueryTrait>::Query),
}

impl<T: QueryTrait> QueryTrait for IbcPlugin<T> {
    type Query = Query<T>;

    fn query(&self, query: Self::Query) -> Result<()> {
        match query {
            Query::Inner(inner_query) => self.inner.query(inner_query),
            Query::Ibc(ibc_query) => todo!(),
        }
    }
}

pub struct InnerAdapter<T, U: Clone> {
    parent: U,
    marker: PhantomData<fn() -> T>,
}

impl<T, U: Clone> Clone for InnerAdapter<T, U> {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            marker: PhantomData,
        }
    }
}

unsafe impl<T, U: Send + Clone> Send for InnerAdapter<T, U> {}

#[async_trait::async_trait(?Send)]
impl<T: CallTrait, U: AsyncCall<Call = Call<T::Call>> + Clone> AsyncCall for InnerAdapter<T, U>
where
    T::Call: Send,
    U: Send,
{
    type Call = T::Call;

    async fn call(&mut self, call: Self::Call) -> Result<()> {
        self.parent.call(Call::Inner(call)).await
    }
}

#[async_trait::async_trait(?Send)]
impl<T: QueryTrait + State, U: AsyncQuery<Query = Query<T>, Response = IbcPlugin<T>> + Clone>
    AsyncQuery for InnerAdapter<T, U>
{
    type Query = T::Query;
    type Response = T;

    async fn query<F, R>(&self, query: Self::Query, mut check: F) -> Result<R>
    where
        F: FnMut(Self::Response) -> Result<R>,
    {
        self.parent
            .query(Query::Inner(query), |plugin| check(plugin.inner))
            .await
    }
}

pub struct IbcAdapter<T, U: Clone> {
    parent: U,
    marker: PhantomData<fn() -> T>,
}

impl<T, U: Clone> Clone for IbcAdapter<T, U> {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            marker: PhantomData,
        }
    }
}

unsafe impl<T, U: Send + Clone> Send for IbcAdapter<T, U> {}

#[async_trait::async_trait(?Send)]
impl<T: CallTrait, U: AsyncCall<Call = Call<T::Call>> + Clone> AsyncCall for IbcAdapter<T, U>
where
    T::Call: Send,
    U: Send,
{
    type Call = <Ibc as CallTrait>::Call;

    async fn call(&mut self, call: Self::Call) -> Result<()> {
        todo!()
        // self.parent.call(Call::Ics26())
    }
}

#[async_trait::async_trait(?Send)]
impl<T: QueryTrait + State, U: AsyncQuery<Query = Query<T>, Response = IbcPlugin<T>> + Clone>
    AsyncQuery for IbcAdapter<T, U>
{
    type Query = <Ibc as QueryTrait>::Query;
    type Response = Ibc;

    async fn query<F, R>(&self, query: Self::Query, mut check: F) -> Result<R>
    where
        F: FnMut(Self::Response) -> Result<R>,
    {
        self.parent
            .query(Query::Ibc(query), |plugin| check(plugin.ibc))
            .await
    }
}

pub struct IbcPluginClient<T: Client<InnerAdapter<T, U>> + State, U: Clone> {
    inner: T::Client,
    parent: U,
}

impl<T: Client<InnerAdapter<T, U>> + State, U: Clone> Deref for IbcPluginClient<T, U> {
    type Target = T::Client;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: Client<InnerAdapter<T, U>> + State, U: Clone> DerefMut for IbcPluginClient<T, U> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: Client<InnerAdapter<T, U>> + State, U: Clone> Client<U> for IbcPlugin<T> {
    type Client = IbcPluginClient<T, U>;

    fn create_client(parent: U) -> Self::Client {
        IbcPluginClient {
            inner: T::create_client(InnerAdapter {
                parent: parent.clone(),
                marker: PhantomData,
            }),
            parent,
        }
    }
}

impl<T: Client<InnerAdapter<T, U>> + State, U: Clone + Send> IbcPluginClient<T, U> {
    pub fn ibc(&self) -> <Ibc as Client<IbcAdapter<T, U>>>::Client {
        Ibc::create_client(IbcAdapter {
            parent: self.parent.clone(),
            marker: PhantomData,
        })
    }
}

#[cfg(feature = "abci")]
mod abci {
    use super::super::{BeginBlockCtx, EndBlockCtx, InitChainCtx};
    use super::*;
    use crate::abci::{BeginBlock, EndBlock, InitChain};

    impl<T> BeginBlock for IbcPlugin<T>
    where
        T: BeginBlock + State,
    {
        fn begin_block(&mut self, ctx: &BeginBlockCtx) -> Result<()> {
            self.inner.begin_block(ctx)
        }
    }

    impl<T> EndBlock for IbcPlugin<T>
    where
        T: EndBlock + State,
    {
        fn end_block(&mut self, ctx: &EndBlockCtx) -> Result<()> {
            self.inner.end_block(ctx)
        }
    }

    impl<T> InitChain for IbcPlugin<T>
    where
        T: InitChain + State,
    {
        fn init_chain(&mut self, ctx: &InitChainCtx) -> Result<()> {
            self.inner.init_chain(ctx)
        }
    }
}
