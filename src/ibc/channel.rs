use super::Ibc;
use ibc::core::ics02_client::client_consensus::AnyConsensusState;
use ibc::core::ics02_client::client_state::AnyClientState;
use ibc::core::ics03_connection::connection::ConnectionEnd;
use ibc::core::ics04_channel::channel::ChannelEnd;
use ibc::core::ics04_channel::commitment::{AcknowledgementCommitment, PacketCommitment};
use ibc::core::ics04_channel::context::{ChannelKeeper, ChannelReader};
use ibc::core::ics04_channel::error::Error;
use ibc::core::ics04_channel::packet::{Receipt, Sequence};
use ibc::core::ics24_host::identifier::{ChannelId, ClientId, ConnectionId, PortId};
use ibc::timestamp::Timestamp;
use ibc::Height;

impl ChannelReader for Ibc {
    fn channel_end(&self, port_channel_id: &(PortId, ChannelId)) -> Result<ChannelEnd, Error> {
        todo!()
    }

    fn connection_end(&self, connection_id: &ConnectionId) -> Result<ConnectionEnd, Error> {
        todo!()
    }

    fn connection_channels(&self, cid: &ConnectionId) -> Result<Vec<(PortId, ChannelId)>, Error> {
        todo!()
    }

    fn client_state(&self, client_id: &ClientId) -> Result<AnyClientState, Error> {
        todo!()
    }

    fn client_consensus_state(
        &self,
        client_id: &ClientId,
        height: Height,
    ) -> Result<AnyConsensusState, Error> {
        todo!()
    }

    fn get_next_sequence_send(
        &self,
        port_channel_id: &(PortId, ChannelId),
    ) -> Result<Sequence, Error> {
        todo!()
    }

    fn get_next_sequence_recv(
        &self,
        port_channel_id: &(PortId, ChannelId),
    ) -> Result<Sequence, Error> {
        todo!()
    }

    fn get_next_sequence_ack(
        &self,
        port_channel_id: &(PortId, ChannelId),
    ) -> Result<Sequence, Error> {
        todo!()
    }

    fn get_packet_commitment(
        &self,
        key: &(PortId, ChannelId, Sequence),
    ) -> Result<PacketCommitment, Error> {
        todo!()
    }

    fn get_packet_receipt(&self, key: &(PortId, ChannelId, Sequence)) -> Result<Receipt, Error> {
        todo!()
    }

    fn get_packet_acknowledgement(
        &self,
        key: &(PortId, ChannelId, Sequence),
    ) -> Result<AcknowledgementCommitment, Error> {
        todo!()
    }

    fn hash(&self, value: Vec<u8>) -> Vec<u8> {
        todo!()
    }

    fn host_height(&self) -> Height {
        todo!()
    }

    fn channel_counter(&self) -> Result<u64, Error> {
        todo!()
    }

    fn host_consensus_state(&self, height: Height) -> Result<AnyConsensusState, Error> {
        todo!()
    }

    fn client_update_height(&self, client_id: &ClientId, height: Height) -> Result<Height, Error> {
        todo!()
    }

    fn client_update_time(&self, client_id: &ClientId, height: Height) -> Result<Timestamp, Error> {
        todo!()
    }

    fn max_expected_time_per_block(&self) -> std::time::Duration {
        todo!()
    }

    fn pending_host_consensus_state(&self) -> Result<AnyConsensusState, Error> {
        todo!()
    }
}

impl ChannelKeeper for Ibc {
    fn store_packet_commitment(
        &mut self,
        key: (PortId, ChannelId, Sequence),
        commitment: PacketCommitment,
    ) -> Result<(), Error> {
        todo!()
    }

    fn delete_packet_commitment(
        &mut self,
        key: (PortId, ChannelId, Sequence),
    ) -> Result<(), Error> {
        todo!()
    }

    fn store_packet_receipt(
        &mut self,
        key: (PortId, ChannelId, Sequence),
        receipt: Receipt,
    ) -> Result<(), Error> {
        todo!()
    }

    fn store_packet_acknowledgement(
        &mut self,
        key: (PortId, ChannelId, Sequence),
        ack: AcknowledgementCommitment,
    ) -> Result<(), Error> {
        todo!()
    }

    fn delete_packet_acknowledgement(
        &mut self,
        key: (PortId, ChannelId, Sequence),
    ) -> Result<(), Error> {
        todo!()
    }

    fn store_connection_channels(
        &mut self,
        conn_id: ConnectionId,
        port_channel_id: &(PortId, ChannelId),
    ) -> Result<(), Error> {
        todo!()
    }

    fn store_channel(
        &mut self,
        port_channel_id: (PortId, ChannelId),
        channel_end: &ChannelEnd,
    ) -> Result<(), Error> {
        todo!()
    }

    fn store_next_sequence_send(
        &mut self,
        port_channel_id: (PortId, ChannelId),
        seq: Sequence,
    ) -> Result<(), Error> {
        todo!()
    }

    fn store_next_sequence_recv(
        &mut self,
        port_channel_id: (PortId, ChannelId),
        seq: Sequence,
    ) -> Result<(), Error> {
        todo!()
    }

    fn store_next_sequence_ack(
        &mut self,
        port_channel_id: (PortId, ChannelId),
        seq: Sequence,
    ) -> Result<(), Error> {
        todo!()
    }

    fn increase_channel_counter(&mut self) {
        todo!()
    }
}
