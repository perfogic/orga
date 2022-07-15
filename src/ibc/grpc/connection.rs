use ibc::core::ics24_host::identifier::ConnectionId;
use ibc_proto::ibc::core::connection::v1::{
    query_server::{Query as ConnectionQuery, QueryServer as ConnectionQueryServer},
    ConnectionEnd as RawConnectionEnd, IdentifiedConnection as RawIdentifiedConnection,
    QueryClientConnectionsRequest, QueryClientConnectionsResponse,
    QueryConnectionClientStateRequest, QueryConnectionClientStateResponse,
    QueryConnectionConsensusStateRequest, QueryConnectionConsensusStateResponse,
    QueryConnectionRequest, QueryConnectionResponse, QueryConnectionsRequest,
    QueryConnectionsResponse,
};
use std::str::FromStr;

use super::Ibc;
use crate::client::{AsyncCall, AsyncQuery, Call};
use crate::query::Query;
use std::rc::Rc;
use tonic::{Request, Response, Status};

#[tonic::async_trait]
impl<T> ConnectionQuery for super::GrpcServer<T>
where
    T: Clone + Send + Sync + 'static,
    // T: AsyncCall<Call = <Ibc as Call>::Call>,
    T: AsyncQuery,
    T: for<'a> AsyncQuery<Response<'a> = Rc<Ibc>>,
    T: AsyncQuery<Query = <Ibc as Query>::Query>,
{
    async fn connection(
        &self,
        request: Request<QueryConnectionRequest>,
    ) -> Result<Response<QueryConnectionResponse>, Status> {
        let conn_id = ConnectionId::from_str(&request.get_ref().connection_id)
            .map_err(|_| Status::invalid_argument("invalid connection id"))?;
        let conn = self
            .ibc
            .connections
            .get_by_conn_id(conn_id.into())
            .await?
            .map_err(|_| Status::not_found("Connection not found"))?
            .into_inner();
        Ok(Response::new(QueryConnectionResponse {
            connection: Some(conn.into()),
            proof: vec![],
            proof_height: None,
        }))
    }

    async fn connections(
        &self,
        _request: Request<QueryConnectionsRequest>,
    ) -> Result<Response<QueryConnectionsResponse>, Status> {
        todo!()
        // let connection_path_prefix: Path = String::from("connections")
        //     .try_into()
        //     .expect("'connections' expected to be a valid Path");

        // let connection_paths = self.connection_end_store.get_keys(&connection_path_prefix);

        // let identified_connections: Vec<RawIdentifiedConnection> = connection_paths
        //     .into_iter()
        //     .map(|path| match path.try_into() {
        //         Ok(IbcPath::Connections(connections_path)) => {
        //             let connection_end = self
        //                 .connection_end_store
        //                 .get(Height::Pending, &connections_path)
        //                 .unwrap();
        //             IdentifiedConnectionEnd::new(connections_path.0, connection_end).into()
        //         }
        //         _ => panic!("unexpected path"),
        //     })
        //     .collect();

        // Ok(Response::new(QueryConnectionsResponse {
        //     connections: identified_connections,
        //     pagination: None,
        //     height: None,
        // }))
    }

    async fn client_connections(
        &self,
        request: Request<QueryClientConnectionsRequest>,
    ) -> Result<Response<QueryClientConnectionsResponse>, Status> {
        todo!()
        // trace!("Got client connections request: {:?}", request);

        // let client_id = request
        //     .get_ref()
        //     .client_id
        //     .parse()
        //     .map_err(|e| Status::invalid_argument(format!("{}", e)))?;
        // let path = path::ClientConnectionsPath(client_id);
        // let connection_ids = self
        //     .connection_ids_store
        //     .get(Height::Pending, &path)
        //     .unwrap_or_default();
        // let connection_paths = connection_ids
        //     .into_iter()
        //     .map(|conn_id| conn_id.to_string())
        //     .collect();

        // Ok(Response::new(QueryClientConnectionsResponse {
        //     connection_paths,
        //     // Note: proofs aren't being used by hermes currently
        //     proof: vec![],
        //     proof_height: None,
        // }))
    }

    async fn connection_client_state(
        &self,
        _request: Request<QueryConnectionClientStateRequest>,
    ) -> Result<Response<QueryConnectionClientStateResponse>, Status> {
        todo!()
    }

    async fn connection_consensus_state(
        &self,
        _request: Request<QueryConnectionConsensusStateRequest>,
    ) -> Result<Response<QueryConnectionConsensusStateResponse>, Status> {
        todo!()
    }
}
