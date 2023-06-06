use crate::{
    abci::App,
    call::Call,
    client::Client,
    encoding::Encode,
    merk::{BackingStore, ProofStore},
    plugins::{ABCICall, ABCIPlugin},
    query::Query,
    state::State,
    store::{Shared, Store},
    Error, Result,
};
use tendermint_rpc::{self as tm, Client as _};

pub struct HttpClient {
    client: tm::HttpClient,
}

impl HttpClient {
    pub fn new(url: &str) -> Result<Self> {
        Ok(Self {
            client: tm::HttpClient::new(url)?,
        })
    }
}

impl<T: App + Call + Query + State + Default> Client<ABCIPlugin<T>> for HttpClient {
    async fn call(&self, call: <ABCIPlugin<T> as Call>::Call) -> Result<()> {
        // TODO: shouldn't need to deal with ABCIPlugin at this level
        let call = match call {
            ABCICall::DeliverTx(call) => call,
            _ => return Err(Error::Client("Unexpected call type".into())),
        };
        let call_bytes = call.encode()?;
        let res = self.client.broadcast_tx_commit(call_bytes).await?;

        if let tendermint::abci::Code::Err(code) = res.check_tx.code {
            let msg = format!("code {}: {}", code, res.check_tx.log);
            return Err(Error::Call(msg));
        }

        Ok(())
    }

    async fn query(&self, query: T::Query) -> Result<Store> {
        let query_bytes = query.encode()?;
        let res = self
            .client
            .abci_query(None, query_bytes, None, true)
            .await?;

        if let tendermint::abci::Code::Err(code) = res.code {
            let msg = format!("code {}: {}", code, res.log);
            return Err(Error::Query(msg));
        }

        // TODO: we shouldn't need to include the root hash in the result, it
        // should come from a trusted source
        let root_hash = match res.value[0..32].try_into() {
            Ok(inner) => inner,
            _ => {
                return Err(Error::Tendermint(
                    "Cannot convert result to fixed size array".into(),
                ));
            }
        };
        let proof_bytes = &res.value[32..];

        let map = merk::proofs::query::verify(proof_bytes, root_hash)?;

        let store: Shared<ProofStore> = Shared::new(ProofStore(map));
        let store = Store::new(BackingStore::ProofMap(store));

        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        abci::InitChain,
        client::{wallet::DerivedKey, AppClient},
        coins::{Accounts, Symbol},
        collections::Map,
        context::Context,
        plugins::{ChainId, ConvertSdkTx, DefaultPlugins, PaidCall},
    };

    use super::*;
    use orga::orga;
    use orga_macros::build_call;

    #[orga]
    #[derive(Debug, Clone, Copy)]
    pub struct FooCoin();

    impl Symbol for FooCoin {
        const INDEX: u8 = 123;
    }

    #[orga]
    pub struct App {
        pub foo: u32,
        pub bar: u32,
        pub map: Map<u32, u32>,
        #[call]
        pub accounts: Accounts<FooCoin>,
    }

    #[orga]
    impl App {
        #[call]
        pub fn increment_foo(&mut self) -> orga::Result<()> {
            self.foo += 1;
            Ok(())
        }
    }

    impl InitChain for App {
        fn init_chain(&mut self, _ctx: &crate::plugins::InitChainCtx) -> Result<()> {
            self.accounts
                .deposit(DerivedKey::address_for(b"alice").unwrap(), 100_000.into())
        }
    }

    pub fn spawn_node() {
        pretty_env_logger::init();

        std::thread::spawn(move || {
            // TODO: find available ports

            Context::add(ChainId("foo".to_string()));

            let home = tempdir::TempDir::new("orga-node").unwrap();
            let node = orga::abci::Node::<DefaultPlugins<FooCoin, App>>::new(
                home.path(),
                orga::abci::DefaultConfig {
                    seeds: None,
                    timeout_commit: None,
                },
            );
            node.run().unwrap();
            home.close().unwrap();
        });

        // TODO: wait for node to be ready

        // TODO: return type which kills node after drop
        // TODO: return client which talks to the node (or just RPC address)
    }

    impl ConvertSdkTx for App {
        type Output = PaidCall<<App as Call>::Call>;

        fn convert(
            &self,
            _msg: &crate::plugins::sdk_compat::sdk::Tx,
        ) -> orga::Result<Self::Output> {
            todo!()
        }
    }

    #[ignore]
    #[tokio::test]
    #[serial_test::serial]
    async fn basic() -> Result<()> {
        spawn_node();

        let client = HttpClient::new("http://localhost:26657").unwrap();
        let client =
            AppClient::<App, App, _, FooCoin, _>::new(client, DerivedKey::new(b"alice").unwrap());

        // TODO: node spawn should return future which waits for node to be ready
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;

        let res = client.query(|app| Ok(app.bar)).await.unwrap();
        assert_eq!(res, 0);

        let res = client
            .query(|app| {
                app.accounts
                    .balance(DerivedKey::address_for(b"alice").unwrap())
            })
            .await
            .unwrap();
        assert_eq!(res.value, 100_000);

        client
            .call(
                |app| build_call!(app.accounts.take_as_funding(50_000.into())),
                |app| build_call!(app.increment_foo()),
            )
            .await
            .unwrap();

        Ok(())
    }
}
