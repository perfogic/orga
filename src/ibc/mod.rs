use ed::Terminated;
use ibc::applications::transfer::send_transfer;
use ibc::clients::ics07_tendermint::{
    client_state::ClientState as TmClientState, consensus_state::ConsensusState as TmConsensusState,
};
use ibc::core::dispatch;
use ibc::core::ics02_client::client_type::ClientType;
use ibc::core::ics02_client::consensus_state::ConsensusState as ConsensusStateTrait;
use ibc::core::ics02_client::height::Height;
use ibc::core::ics03_connection::connection::ConnectionEnd as IbcConnectionEnd;
use ibc::core::ics04_channel::channel::ChannelEnd as IbcChannelEnd;
use ibc::core::ics04_channel::packet::Sequence;
use ibc::core::ics24_host::identifier::{
    ChannelId, ClientId as IbcClientId, ConnectionId as IbcConnectionId, PortId,
};
use ibc::core::ics24_host::path::{
    AckPath, ChannelEndPath, ClientConnectionPath, CommitmentPath, ConnectionPath, ReceiptPath,
    SeqAckPath, SeqRecvPath, SeqSendPath,
};
use ibc::Signer as IbcSigner;
use ibc_proto::google::protobuf::Any;
use ibc_proto::ibc::applications::transfer::v1::MsgTransfer as RawMsgTransfer;
use ibc_proto::ibc::core::{
    channel::v1::Channel as RawChannelEnd, connection::v1::ConnectionEnd as RawConnectionEnd,
};

use ibc_proto::protobuf::Protobuf;
use ibc_rs::applications::transfer::msgs::transfer::MsgTransfer;
use serde::Serialize;
use tendermint_proto::Protobuf as TmProtobuf;

use crate::coins::Address;
use crate::collections::{Deque, Map};
use crate::context::GetContext;
use crate::describe::{Describe, Descriptor};
use crate::encoding::{
    Adapter, ByteTerminatedString, Decode, Encode, EofTerminatedString, FixedString,
};
use crate::migrate::{Migrate, MigrateInto};
use crate::plugins::Signer;
use crate::state::State;
use crate::store::Store;
use crate::{orga, Error, Result as OrgaResult};
pub use ibc as ibc_rs;
use ibc::core::timestamp::Timestamp as IbcTimestamp;

mod impls;
pub mod transfer;
use transfer::{Transfer, TransferInfo};
#[cfg(feature = "abci")]
mod service;
#[cfg(feature = "abci")]
pub use service::{start_grpc, GrpcOpts};

pub use self::messages::{IbcMessage, IbcTx, RawIbcTx};
mod client_contexts;
mod messages;
mod migration;
mod query;
mod router;
// #[cfg(test)]
// mod tests2;
pub const IBC_QUERY_PATH: &str = "store/ibc/key";

#[orga(version = 1)]
pub struct Ibc {
    height: u64,
    host_consensus_states: Deque<ConsensusState>,
    channel_counter: u64,
    connection_counter: u64,
    client_counter: u64,
    pub transfer: Transfer,

    #[state(absolute_prefix(b"clients/"))]
    clients: Map<ClientId, Client>,

    #[state(absolute_prefix(b"connections/"))]
    connections: Map<ConnectionId, ConnectionEnd>,

    #[state(absolute_prefix(b"channelEnds/"))]
    channel_ends: Map<PortChannel, ChannelEnd>,

    #[state(absolute_prefix(b"nextSequenceSend/"))]
    next_sequence_send: Map<PortChannel, Number>,

    #[state(absolute_prefix(b"nextSequenceRecv/"))]
    next_sequence_recv: Map<PortChannel, Number>,

    #[state(absolute_prefix(b"nextSequenceAck/"))]
    next_sequence_ack: Map<PortChannel, Number>,

    #[state(absolute_prefix(b"commitments/"))]
    commitments: Map<PortChannelSequence, Vec<u8>>,

    #[state(absolute_prefix(b"receipts/"))]
    receipts: Map<PortChannelSequence, ()>,

    #[state(absolute_prefix(b"acks/"))]
    acks: Map<PortChannelSequence, Vec<u8>>,

    #[state(absolute_prefix(b""))]
    store: Store,

    #[state(skip)]
    #[serde(skip)]
    incoming_transfer: Option<transfer::TransferInfo>,
}

#[orga]
impl Ibc {
    pub fn deliver(&mut self, messages: RawIbcTx) -> crate::Result<Vec<TransferInfo>> {
        let messages: IbcTx = messages.try_into()?;
        let mut incoming_transfers = vec![];
        for message in messages.0 {
            if let Some(incoming_transfer) = self.deliver_message(message)? {
                incoming_transfers.push(incoming_transfer);
            }
        }
        Ok(incoming_transfers)
    }

    #[call]
    pub fn raw_transfer(&mut self, message: TransferMessage) -> crate::Result<()> {
        let message: MsgTransfer = message.inner;
        let sender_addr: Address = message.packet_data.sender.clone().try_into()?;

        if self.signer()? != sender_addr {
            return Err(crate::Error::Ibc(
                "Transfers must be signed by the sender".into(),
            ));
        }

        self.deliver_message(IbcMessage::Ics20(message))?;

        Ok(())
    }

    pub fn deliver_message(&mut self, message: IbcMessage) -> crate::Result<Option<TransferInfo>> {
        use IbcMessage::*;
        match message {
            Ics26(msg) => dispatch(self, msg).map_err(|e| Error::Ibc(e.to_string()))?,
            Ics20(msg) => send_transfer(self, msg).map_err(|e| Error::Ibc(e.to_string()))?,
        };
        Ok(self.incoming_transfer.take())
    }

    fn signer(&mut self) -> crate::Result<Address> {
        self.context::<Signer>()
            .ok_or_else(|| Error::Coins("No Signer context available".into()))?
            .signer
            .ok_or_else(|| Error::Coins("Call must be signed".into()))
    }
}

impl std::fmt::Debug for Ibc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ibc").finish()
    }
}

#[orga]
pub struct Client {
    #[state(prefix(b"updates/"))]
    updates: Map<EpochHeight, (Timestamp, BinaryHeight)>,

    #[state(prefix(b"clientState"))]
    client_state: Map<(), ClientState>,

    #[state(prefix(b"consensusStates/"))]
    consensus_states: Map<EpochHeight, ConsensusState>,

    #[state(prefix(b"connections/"))]
    connections: Map<ConnectionId, ()>,

    client_type: EofTerminatedString,
}

pub type SlashTerminatedString<T> = ByteTerminatedString<b'/', T>;

pub type ClientId = SlashTerminatedString<IbcClientId>;
pub type ConnectionId = EofTerminatedString<IbcConnectionId>;
pub type Number = EofTerminatedString<u64>;
pub type EpochHeight = EofTerminatedString;

#[orga(simple, skip(Migrate))]
#[derive(Debug)]
pub struct Timestamp {
    inner: IbcTimestamp,
}

// TODO: replace when macro can specify min version
impl Migrate for Timestamp {
    fn migrate(_src: Store, _dest: Store, bytes: &mut &[u8]) -> OrgaResult<Self> {
        if bytes[0] != 1 {
            return Err(crate::Error::Ibc(format!(
                "Invalid timestamp version: {}",
                bytes[0]
            )));
        }
        *bytes = &bytes[1..];

        Timestamp::load(Store::default(), bytes)
    }
}

impl Describe for Timestamp {
    fn describe() -> Descriptor {
        crate::describe::Builder::new::<Self>().build()
    }
}

impl Encode for Adapter<Timestamp> {
    fn encode_into<W: std::io::Write>(&self, dest: &mut W) -> ed::Result<()> {
        self.0.inner.nanoseconds().encode_into(dest)
    }

    fn encoding_length(&self) -> ed::Result<usize> {
        self.0.inner.nanoseconds().encoding_length()
    }
}

impl Decode for Adapter<Timestamp> {
    fn decode<R: std::io::Read>(input: R) -> ed::Result<Self> {
        Ok(Self(Timestamp {
            inner: IbcTimestamp::from_nanoseconds(u64::decode(input)?)
                .map_err(|_| ed::Error::UnexpectedByte(40))?,
        }))
    }
}

impl From<Timestamp> for IbcTimestamp {
    fn from(timestamp: Timestamp) -> Self {
        timestamp.inner
    }
}

impl From<IbcTimestamp> for Timestamp {
    fn from(timestamp: IbcTimestamp) -> Self {
        Self { inner: timestamp }
    }
}

impl Encode for Adapter<IbcSigner> {
    fn encode_into<W: std::io::Write>(&self, dest: &mut W) -> ed::Result<()> {
        borsh::BorshSerialize::serialize(&self.0, dest).map_err(|_| ed::Error::UnexpectedByte(40))
    }

    fn encoding_length(&self) -> ed::Result<usize> {
        let mut buf = vec![];
        borsh::BorshSerialize::serialize(&self.0, &mut buf)
            .map_err(|_| ed::Error::UnexpectedByte(40))?;
        Ok(buf.len())
    }
}

impl Decode for Adapter<IbcSigner> {
    fn decode<R: std::io::Read>(input: R) -> ed::Result<Self> {
        let bytes = input
            .bytes()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ed::Error::UnexpectedByte(40))?;
        Ok(Self(
            borsh::BorshDeserialize::deserialize(&mut bytes.as_slice())
                .map_err(|_| ed::Error::UnexpectedByte(40))?,
        ))
    }
}

impl Terminated for Adapter<IbcSigner> {}

#[orga]
#[derive(Clone, Debug)]
pub struct BinaryHeight {
    epoch: u64,
    height: u64,
}

impl From<Height> for BinaryHeight {
    fn from(height: Height) -> Self {
        Self {
            epoch: height.revision_number(),
            height: height.revision_height(),
        }
    }
}

impl TryFrom<BinaryHeight> for Height {
    type Error = ();

    fn try_from(height: BinaryHeight) -> Result<Self, Self::Error> {
        Height::new(height.epoch, height.height).map_err(|_| ())
    }
}

impl Terminated for Adapter<Timestamp> {}

impl From<ConnectionPath> for ConnectionId {
    fn from(path: ConnectionPath) -> Self {
        Self(path.0)
    }
}

impl From<ClientConnectionPath> for ClientId {
    fn from(path: ClientConnectionPath) -> Self {
        Self(path.0)
    }
}

impl From<Sequence> for Number {
    fn from(sequence: Sequence) -> Self {
        Self(sequence.into())
    }
}

impl From<Height> for EpochHeight {
    fn from(height: Height) -> Self {
        Self(format!(
            "{}-{}",
            height.revision_number(),
            height.revision_height()
        ))
    }
}

impl TryFrom<EpochHeight> for Height {
    type Error = Error;

    fn try_from(epoch_height: EpochHeight) -> Result<Self, Self::Error> {
        let mut parts = epoch_height.0.split('-');
        let revision_number = parts
            .next()
            .ok_or(Error::Ibc("Invalid revision number".to_string()))?
            .parse()
            .map_err(|_| Error::Ibc("Invalid revision number".to_string()))?;
        let revision_height = parts
            .next()
            .ok_or(Error::Ibc("Invalid revision height".to_string()))?
            .parse()
            .map_err(|_| Error::Ibc("Invalid revision height".to_string()))?;
        Height::new(revision_number, revision_height)
            .map_err(|_| Error::Ibc("Failed to parse height".to_string()))
    }
}

impl From<ClientType> for EofTerminatedString {
    fn from(client_type: ClientType) -> Self {
        Self(client_type.as_str().to_string())
    }
}

impl From<EofTerminatedString> for ClientType {
    fn from(client_type: EofTerminatedString) -> Self {
        ClientType::new(client_type.0.as_str()).unwrap()
    }
}

#[derive(State, Encode, Decode, Serialize, Clone, Debug)]
pub struct PortChannel(
    #[serde(skip)] FixedString<"ports/">,
    SlashTerminatedString<PortId>,
    #[serde(skip)] FixedString<"channels/">,
    EofTerminatedString<ChannelId>,
);

impl Migrate for PortChannel {
    fn migrate(_src: Store, _dest: Store, bytes: &mut &[u8]) -> OrgaResult<Self> {
        Ok(Decode::decode(bytes)?)
    }
}

impl Describe for PortChannel {
    fn describe() -> Descriptor {
        crate::describe::Builder::new::<Self>().build()
    }
}

impl PortChannel {
    pub fn new(port_id: PortId, channel_id: ChannelId) -> Self {
        Self(
            FixedString,
            ByteTerminatedString(port_id),
            FixedString,
            EofTerminatedString(channel_id),
        )
    }

    pub fn port_id(&self) -> crate::Result<PortId> {
        self.1
            .clone()
            .to_string()
            .parse()
            .map_err(|_| Error::Ibc("Invalid port ID".to_string()))
    }

    pub fn channel_id(&self) -> crate::Result<ChannelId> {
        self.3
            .clone()
            .to_string()
            .parse()
            .map_err(|_| Error::Ibc("Invalid channel ID".to_string()))
    }

    pub fn with_sequence(self, sequence: Sequence) -> crate::Result<PortChannelSequence> {
        Ok(PortChannelSequence::new(
            self.port_id()?,
            self.channel_id()?,
            sequence,
        ))
    }
}

macro_rules! port_channel_from_impl {
    ($ty:ty) => {
        impl From<$ty> for PortChannel {
            fn from(path: $ty) -> Self {
                Self(
                    FixedString,
                    ByteTerminatedString(path.0),
                    FixedString,
                    EofTerminatedString(path.1),
                )
            }
        }
    };
}

port_channel_from_impl!(ChannelEndPath);
port_channel_from_impl!(SeqSendPath);
port_channel_from_impl!(SeqRecvPath);
port_channel_from_impl!(SeqAckPath);

#[derive(State, Encode, Decode, Serialize, Clone, Debug)]
pub struct PortChannelSequence(
    #[serde(skip)] FixedString<"ports/">,
    SlashTerminatedString<PortId>,
    #[serde(skip)] FixedString<"channels/">,
    SlashTerminatedString<ChannelId>,
    #[serde(skip)] FixedString<"sequences/">,
    EofTerminatedString<Sequence>,
);

impl Migrate for PortChannelSequence {
    fn migrate(_src: Store, _dest: Store, bytes: &mut &[u8]) -> OrgaResult<Self> {
        Ok(Decode::decode(bytes)?)
    }
}

impl Describe for PortChannelSequence {
    fn describe() -> Descriptor {
        crate::describe::Builder::new::<Self>().build()
    }
}

impl PortChannelSequence {
    pub fn new(port_id: PortId, channel_id: ChannelId, sequence: Sequence) -> Self {
        Self(
            FixedString,
            ByteTerminatedString(port_id),
            FixedString,
            ByteTerminatedString(channel_id),
            FixedString,
            EofTerminatedString(sequence),
        )
    }

    pub fn port_id(&self) -> crate::Result<PortId> {
        self.1
            .clone()
            .to_string()
            .parse()
            .map_err(|_| Error::Ibc("Invalid port ID".to_string()))
    }

    pub fn channel_id(&self) -> crate::Result<ChannelId> {
        self.3
            .clone()
            .to_string()
            .parse()
            .map_err(|_| Error::Ibc("Invalid channel ID".to_string()))
    }

    pub fn sequence(&self) -> crate::Result<Sequence> {
        self.5
            .clone()
            .to_string()
            .parse()
            .map_err(|_| Error::Ibc("Invalid sequence".to_string()))
    }
}

macro_rules! port_channel_sequence_from_impl {
    ($ty:ty) => {
        impl From<$ty> for PortChannelSequence {
            fn from(path: $ty) -> Self {
                Self(
                    FixedString,
                    ByteTerminatedString(path.port_id),
                    FixedString,
                    ByteTerminatedString(path.channel_id),
                    FixedString,
                    EofTerminatedString(path.sequence),
                )
            }
        }
    };
}

port_channel_sequence_from_impl!(CommitmentPath);
port_channel_sequence_from_impl!(AckPath);
port_channel_sequence_from_impl!(ReceiptPath);

macro_rules! protobuf_newtype {
    ($newtype:tt, $inner:ty, $raw:ty, $proto:tt, $prev:ty) => {
        #[derive(Serialize, Clone, Debug)]
        pub struct $newtype {
            inner: $inner,
        }

        impl Encode for $newtype {
            fn encode_into<W: std::io::Write>(&self, dest: &mut W) -> ed::Result<()> {
                let mut buf = vec![];
                $proto::<$raw>::encode(&self.inner, &mut buf)
                    .map_err(|_| ed::Error::UnexpectedByte(10))?;
                dest.write_all(&buf)?;
                Ok(())
            }

            fn encoding_length(&self) -> ed::Result<usize> {
                let mut buf = vec![];
                $proto::<$raw>::encode(&self.inner, &mut buf)
                    .map_err(|_| ed::Error::UnexpectedByte(10))?;
                Ok(buf.len())
            }
        }

        impl Decode for $newtype {
            fn decode<R: std::io::Read>(mut input: R) -> ed::Result<Self> {
                let mut buf = vec![];
                input.read_to_end(&mut buf)?;
                let inner = $proto::<$raw>::decode(buf.as_slice())
                    .map_err(|_| ed::Error::UnexpectedByte(10))?;
                Ok(Self { inner })
            }
        }

        impl crate::state::State for $newtype {
            fn attach(&mut self, _store: crate::store::Store) -> crate::Result<()> {
                Ok(())
            }

            fn flush<W: std::io::Write>(self, out: &mut W) -> crate::Result<()> {
                self.encode_into(out)?;
                Ok(())
            }

            fn load(_store: crate::store::Store, bytes: &mut &[u8]) -> crate::Result<Self> {
                Ok(Self::decode(bytes)?)
            }
        }

        impl From<$inner> for $newtype {
            fn from(inner: $inner) -> Self {
                Self { inner }
            }
        }

        impl From<$newtype> for $inner {
            fn from(outer: $newtype) -> Self {
                outer.inner
            }
        }

        #[allow(trivial_bounds)]
        impl From<$newtype> for $raw
        where
            $inner: Into<$raw>,
        {
            fn from(outer: $newtype) -> Self {
                outer.inner.into()
            }
        }

        impl Migrate for $newtype {
            fn migrate(_src: Store, _dest: Store, bytes: &mut &[u8]) -> OrgaResult<Self> {
                let prev = <$prev>::load(Store::default(), bytes)?;
                prev.migrate_into()
            }
        }

        impl Describe for $newtype {
            fn describe() -> Descriptor {
                crate::describe::Builder::new::<Self>().build()
            }
        }
    };
}

protobuf_newtype!(ClientState, TmClientState, Any, Protobuf, ClientState);

protobuf_newtype!(
    ConsensusState,
    TmConsensusState,
    Any,
    TmProtobuf,
    ConsensusState
);

protobuf_newtype!(
    ConnectionEnd,
    IbcConnectionEnd,
    RawConnectionEnd,
    Protobuf,
    ConnectionEnd
);
protobuf_newtype!(
    ChannelEnd,
    IbcChannelEnd,
    RawChannelEnd,
    Protobuf,
    ChannelEnd
);

impl ConsensusStateTrait for ConsensusState {
    fn root(&self) -> &ibc_rs::core::ics23_commitment::commitment::CommitmentRoot {
        self.inner.root()
    }

    fn timestamp(&self) -> IbcTimestamp {
        self.inner.timestamp()
    }

    fn encode_vec(&self) -> Result<Vec<u8>, tendermint_proto::Error> {
        ConsensusStateTrait::encode_vec(&self.inner)
    }
}

#[derive(Debug, Clone)]
pub struct TransferMessage {
    inner: MsgTransfer,
}

impl From<MsgTransfer> for TransferMessage {
    fn from(inner: MsgTransfer) -> Self {
        Self { inner }
    }
}

impl Encode for TransferMessage {
    fn encode_into<W: std::io::Write>(&self, dest: &mut W) -> ed::Result<()> {
        let mut buf = vec![];
        Protobuf::<RawMsgTransfer>::encode(&self.inner, &mut buf)
            .map_err(|_| ed::Error::UnexpectedByte(10))?;
        dest.write_all(&buf)?;
        Ok(())
    }

    fn encoding_length(&self) -> ed::Result<usize> {
        let mut buf = vec![];
        Protobuf::<RawMsgTransfer>::encode(&self.inner, &mut buf)
            .map_err(|_| ed::Error::UnexpectedByte(10))?;
        Ok(buf.len())
    }
}

impl Decode for TransferMessage {
    fn decode<R: std::io::Read>(mut input: R) -> ed::Result<Self> {
        let mut buf = vec![];
        input.read_to_end(&mut buf)?;
        let inner = Protobuf::<RawMsgTransfer>::decode(buf.as_slice())
            .map_err(|_| ed::Error::UnexpectedByte(10))?;
        Ok(Self { inner })
    }
}

impl TryFrom<IbcSigner> for Address {
    type Error = crate::Error;

    fn try_from(signer: IbcSigner) -> crate::Result<Self> {
        signer
            .as_ref()
            .parse()
            .map_err(|_| crate::Error::Ibc("Invalid signer".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ibc::{
        clients::ics07_tendermint::{client_state::AllowUpdate, trust_threshold::TrustThreshold},
        core::{
            ics02_client::{client_type::ClientType, height::Height},
            ics23_commitment::{commitment::CommitmentRoot, specs::ProofSpecs},
            ics24_host::identifier::ChainId,
        },
    };
    use tendermint::{Hash, Time};

    use super::*;
    use crate::{
        state::State,
        store::{BackingStore, MapStore, Shared, Store},
    };

    #[orga]
    pub struct App {
        ibc: Ibc,
    }

    #[test]
    fn state_structure() {
        let store = Store::new(BackingStore::MapStore(Shared::new(MapStore::new())));

        let mut app = App::default();
        app.attach(store.clone()).unwrap();
        let ibc = &mut app.ibc;

        ibc.channel_counter = 123;
        ibc.connection_counter = 456;
        ibc.client_counter = 789;

        let mut client = Client::default();
        let client_state = TmClientState::new(
            ChainId::new("foo", 0),
            TrustThreshold::default(),
            Duration::from_secs(60 * 60 * 24 * 7),
            Duration::from_secs(60 * 60 * 24 * 14),
            Duration::from_secs(60),
            Height::new(0, 1234).unwrap(),
            ProofSpecs::default(),
            vec![],
            AllowUpdate {
                after_expiry: false,
                after_misbehaviour: false,
            },
        )
        .unwrap()
        .into();
        client.client_state.insert((), client_state).unwrap();
        let consensus_state = TmConsensusState::new(
            CommitmentRoot::from_bytes(&[0; 32]),
            Time::from_unix_timestamp(0, 0).unwrap(),
            Hash::Sha256([5; 32]),
        )
        .into();
        client
            .consensus_states
            .insert("0-100".to_string().into(), consensus_state)
            .unwrap();
        let client_id = IbcClientId::new(ClientType::new("07-tendermint").unwrap(), 123)
            .unwrap()
            .into();
        client.client_type = ClientType::new("07-tendermint").unwrap().into();
        client
            .updates
            .insert(
                "0-100".to_string().into(),
                (
                    IbcTimestamp::default().into(),
                    Height::new(0, 123).unwrap().into(),
                ),
            )
            .unwrap();
        let conn_id = IbcConnectionId::new(123);
        client
            .connections
            .insert(conn_id.clone().into(), ())
            .unwrap();

        ibc.clients.insert(client_id, client).unwrap();
        let conn = IbcConnectionEnd::default().into();
        ibc.connections.insert(conn_id.into(), conn).unwrap();

        let channel_end_path = ChannelEndPath(PortId::transfer(), ChannelId::new(123)).into();
        let chan = IbcChannelEnd::new(
            ibc::core::ics04_channel::channel::State::Open,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .unwrap()
        .into();
        ibc.channel_ends.insert(channel_end_path, chan).unwrap();

        let seq_sends_path = SeqSendPath(PortId::transfer(), ChannelId::new(123)).into();
        ibc.next_sequence_send
            .insert(seq_sends_path, 1.into())
            .unwrap();

        let seq_recvs_path = SeqRecvPath(PortId::transfer(), ChannelId::new(123)).into();
        ibc.next_sequence_recv
            .insert(seq_recvs_path, 2.into())
            .unwrap();

        let seq_acks_path = SeqAckPath(PortId::transfer(), ChannelId::new(123)).into();
        ibc.next_sequence_ack
            .insert(seq_acks_path, 3.into())
            .unwrap();

        let commitments_path = CommitmentPath {
            port_id: PortId::transfer(),
            channel_id: ChannelId::new(123),
            sequence: 1.into(),
        }
        .into();
        ibc.commitments
            .insert(commitments_path, vec![1, 2, 3])
            .unwrap();

        let acks_path = AckPath {
            port_id: PortId::transfer(),
            channel_id: ChannelId::new(123),
            sequence: 1.into(),
        }
        .into();
        ibc.acks.insert(acks_path, vec![1, 2, 3]).unwrap();

        let receipts_path = ReceiptPath {
            port_id: PortId::transfer(),
            channel_id: ChannelId::new(123),
            sequence: 1.into(),
        }
        .into();
        ibc.receipts.insert(receipts_path, ()).unwrap();

        let mut bytes = vec![];
        app.flush(&mut bytes).unwrap();
        assert_eq!(
            bytes,
            vec![
                0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 127, 255, 255, 255, 255, 255, 255, 255, 127, 255,
                255, 255, 255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 123, 0, 0, 0, 0, 0, 0, 1, 200,
                0, 0, 0, 0, 0, 0, 3, 21, 0
            ]
        );

        let mut entries = store.range(..);
        let mut assert_next = |key: &[u8], value: &[u8]| {
            let (k, v) = entries.next().unwrap().unwrap();
            assert_eq!(
                String::from_utf8(k).unwrap(),
                String::from_utf8(key.to_vec()).unwrap()
            );
            assert_eq!(
                v,
                value,
                "key: {}",
                String::from_utf8(key.to_vec()).unwrap()
            );
        };

        assert_next(
            b"acks/ports/transfer/channels/channel-123/sequences/1",
            &[1, 2, 3],
        );
        assert_next(
            b"channelEnds/ports/transfer/channels/channel-123",
            &[
                8, 3, 16, 1, 26, 13, 10, 11, 100, 101, 102, 97, 117, 108, 116, 80, 111, 114, 116,
            ],
        );
        assert_next(b"clients/07-tendermint-123/", b"\x0007-tendermint");
        assert_next(
            b"clients/07-tendermint-123/clientState",
            &[
                10, 43, 47, 105, 98, 99, 46, 108, 105, 103, 104, 116, 99, 108, 105, 101, 110, 116,
                115, 46, 116, 101, 110, 100, 101, 114, 109, 105, 110, 116, 46, 118, 49, 46, 67,
                108, 105, 101, 110, 116, 83, 116, 97, 116, 101, 18, 90, 10, 5, 102, 111, 111, 45,
                48, 18, 4, 8, 1, 16, 3, 26, 4, 8, 128, 245, 36, 34, 4, 8, 128, 234, 73, 42, 2, 8,
                60, 50, 0, 58, 3, 16, 210, 9, 66, 25, 10, 9, 8, 1, 24, 1, 32, 1, 42, 1, 0, 18, 12,
                10, 2, 0, 1, 16, 33, 24, 4, 32, 12, 48, 1, 66, 25, 10, 9, 8, 1, 24, 1, 32, 1, 42,
                1, 0, 18, 12, 10, 2, 0, 1, 16, 32, 24, 1, 32, 1, 48, 1,
            ],
        );
        assert_next(b"clients/07-tendermint-123/connections/connection-123", &[]);
        assert_next(
            b"clients/07-tendermint-123/consensusStates/0-100",
            &[
                10, 46, 47, 105, 98, 99, 46, 108, 105, 103, 104, 116, 99, 108, 105, 101, 110, 116,
                115, 46, 116, 101, 110, 100, 101, 114, 109, 105, 110, 116, 46, 118, 49, 46, 67,
                111, 110, 115, 101, 110, 115, 117, 115, 83, 116, 97, 116, 101, 18, 72, 10, 0, 18,
                34, 10, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 26, 32, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
                5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
            ],
        );
        assert_next(
            b"clients/07-tendermint-123/updates/0-100",
            &[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 123,
            ],
        );
        assert_next(
            b"commitments/ports/transfer/channels/channel-123/sequences/1",
            &[1, 2, 3],
        );
        assert_next(
            b"connections/connection-123",
            &[
                10, 15, 48, 55, 45, 116, 101, 110, 100, 101, 114, 109, 105, 110, 116, 45, 48, 34,
                19, 10, 15, 48, 55, 45, 116, 101, 110, 100, 101, 114, 109, 105, 110, 116, 45, 48,
                26, 0,
            ],
        );
        assert_next(b"nextSequenceAck/ports/transfer/channels/channel-123", b"3");
        assert_next(
            b"nextSequenceRecv/ports/transfer/channels/channel-123",
            b"2",
        );
        assert_next(
            b"nextSequenceSend/ports/transfer/channels/channel-123",
            b"1",
        );
        assert_next(
            b"receipts/ports/transfer/channels/channel-123/sequences/1",
            &[],
        );
        assert!(entries.next().is_none());
    }
}
