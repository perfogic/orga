use serde::{Deserialize, Serialize};

use super::sdk_compat::{sdk::Tx as SdkTx, ConvertSdkTx};
use super::Paid;
use crate::call::Call;
use crate::coins::{Coin, Symbol};
use crate::context::{Context, GetContext};
use crate::describe::Describe;
use crate::encoding::{Decode, Encode};
use crate::migrate::{MigrateFrom, MigrateInto};
use crate::query::Query;
use crate::state::State;
use crate::{Error, Result};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

pub const MIN_FEE: u64 = 10_000;

#[derive(Encode, Decode, Default, Serialize, Deserialize, State)]
#[state(transparent)]
pub struct FeePlugin<S, T> {
    #[state(skip)]
    _symbol: PhantomData<S>,
    pub inner: T,
}

impl<S1, S2, T1, T2> MigrateFrom<FeePlugin<S1, T1>> for FeePlugin<S2, T2>
where
    T1: MigrateInto<T2>,
{
    fn migrate_from(other: FeePlugin<S1, T1>) -> Result<Self> {
        Ok(Self {
            _symbol: other._symbol.migrate_into()?,
            inner: other.inner.migrate_into()?,
        })
    }
}

impl<S, T> Describe for FeePlugin<S, T>
where
    S: Symbol,
    T: State + Describe + 'static,
{
    fn describe() -> crate::describe::Descriptor {
        T::describe()
    }
}

impl<S, T: Query> Query for FeePlugin<S, T> {
    type Query = T::Query;

    fn query(&self, query: Self::Query) -> Result<()> {
        self.inner.query(query)
    }
}

impl<S: Symbol, T: Call + State> Call for FeePlugin<S, T> {
    type Call = T::Call;

    fn call(&mut self, call: Self::Call) -> Result<()> {
        let paid = self
            .context::<Paid>()
            .ok_or_else(|| Error::Coins("Minimum fee not paid".into()))?;

        if !paid.running_payer && !paid.fee_disabled {
            let fee_payment: Coin<S> = paid.take(MIN_FEE)?;
            fee_payment.burn();
        }

        self.inner.call(call)
    }
}

pub fn disable_fee() {
    if let Some(paid_ctx) = Context::resolve::<Paid>() {
        paid_ctx.fee_disabled = true;
    }
}

impl<S, T: ConvertSdkTx> ConvertSdkTx for FeePlugin<S, T> {
    type Output = T::Output;

    fn convert(&self, sdk_tx: &SdkTx) -> Result<T::Output> {
        self.inner.convert(sdk_tx)
    }
}

impl<S, T> Deref for FeePlugin<S, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<S, T> DerefMut for FeePlugin<S, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// TODO: Remove dependency on ABCI for this otherwise-pure plugin.
#[cfg(feature = "abci")]
mod abci {
    use super::super::{BeginBlockCtx, EndBlockCtx, InitChainCtx};
    use super::*;
    use crate::abci::{BeginBlock, EndBlock, InitChain};

    impl<S, T> BeginBlock for FeePlugin<S, T>
    where
        S: Symbol,
        T: BeginBlock + State,
    {
        fn begin_block(&mut self, ctx: &BeginBlockCtx) -> Result<()> {
            self.inner.begin_block(ctx)
        }
    }

    impl<S, T> EndBlock for FeePlugin<S, T>
    where
        S: Symbol,
        T: EndBlock + State,
    {
        fn end_block(&mut self, ctx: &EndBlockCtx) -> Result<()> {
            self.inner.end_block(ctx)
        }
    }

    impl<S, T> InitChain for FeePlugin<S, T>
    where
        S: Symbol,
        T: InitChain + State + Call,
    {
        fn init_chain(&mut self, ctx: &InitChainCtx) -> Result<()> {
            self.inner.init_chain(ctx)
        }
    }

    impl<S, T> crate::abci::AbciQuery for FeePlugin<S, T>
    where
        S: Symbol,
        T: crate::abci::AbciQuery + State + Call,
    {
        fn abci_query(
            &self,
            request: &tendermint_proto::abci::RequestQuery,
        ) -> Result<tendermint_proto::abci::ResponseQuery> {
            self.inner.abci_query(request)
        }
    }
}
