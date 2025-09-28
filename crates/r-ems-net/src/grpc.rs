//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Network connectivity and edge adapters."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::net::SocketAddr;
use std::sync::Arc;

use prost_types::value::Kind;
use prost_types::{Struct, Value};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tonic::transport::server::TcpIncoming;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use crate::{
    CommandAuthoriser, CommandError, CommandHandler, CommandRequest, CommandResponse,
    ControllerStatus, GridStatus, StatusProvider, StatusSnapshot,
};

#[allow(missing_docs)]
pub mod proto {
    tonic::include_proto!("ems");
}

use proto::command_service_server::{CommandService, CommandServiceServer};
use proto::status_service_server::{StatusService, StatusServiceServer};
use proto::Empty;

/// Builder for the gRPC server implementation.
/// Configures and spawns the networking gRPC server.
#[derive(Clone)]
pub struct GrpcServerBuilder {
    listen: SocketAddr,
    provider: Arc<dyn StatusProvider>,
    handler: Arc<dyn CommandHandler>,
    authoriser: Arc<dyn CommandAuthoriser>,
}

impl GrpcServerBuilder {
    /// Create a new builder from the core service dependencies.
    pub fn new(
        listen: SocketAddr,
        provider: Arc<dyn StatusProvider>,
        handler: Arc<dyn CommandHandler>,
        authoriser: Arc<dyn CommandAuthoriser>,
    ) -> Self {
        Self {
            listen,
            provider,
            handler,
            authoriser,
        }
    }

    /// Spawn the gRPC server and return a handle for coordinated shutdown.
    pub async fn spawn(self) -> anyhow::Result<GrpcServerHandle> {
        let listener = tokio::net::TcpListener::bind(self.listen).await?;
        let local_addr = listener.local_addr()?;
        info!(address = %local_addr, "grpc api listening");

        let status_service = StatusSvc {
            provider: self.provider,
        };
        let command_service = CommandSvc {
            handler: self.handler,
            authoriser: self.authoriser,
        };

        let incoming = TcpIncoming::from_listener(listener, true, None)
            .map_err(|err| anyhow::anyhow!("failed to build grpc incoming listener: {err}"))?;
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(async move {
            let server = Server::builder()
                .add_service(StatusServiceServer::new(status_service))
                .add_service(CommandServiceServer::new(command_service))
                .serve_with_incoming_shutdown(incoming, async move {
                    let _ = shutdown_rx.changed().await;
                });
            if let Err(err) = server.await {
                warn!(error = %err, "grpc server exited with error");
            }
        });

        Ok(GrpcServerHandle {
            address: local_addr,
            shutdown: shutdown_tx,
            task,
        })
    }
}

/// Handle returned when spawning the gRPC server.
pub struct GrpcServerHandle {
    address: SocketAddr,
    shutdown: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl GrpcServerHandle {
    /// Socket address the server bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    /// Signal shutdown and await task completion.
    pub async fn shutdown(self) -> anyhow::Result<()> {
        let _ = self.shutdown.send(true);
        match self.task.await {
            Ok(()) => Ok(()),
            Err(err) => Err(anyhow::anyhow!(err)),
        }
    }
}

struct StatusSvc {
    provider: Arc<dyn StatusProvider>,
}

#[tonic::async_trait]
impl StatusService for StatusSvc {
    async fn get_status(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<proto::StatusSnapshot>, Status> {
        let snapshot = self.provider.snapshot();
        Ok(Response::new(proto::StatusSnapshot::from(snapshot)))
    }
}

struct CommandSvc {
    handler: Arc<dyn CommandHandler>,
    authoriser: Arc<dyn CommandAuthoriser>,
}

#[tonic::async_trait]
impl CommandService for CommandSvc {
    async fn submit_command(
        &self,
        request: Request<proto::CommandRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let metadata = request.metadata();
        let api_key = metadata
            .get("x-api-key")
            .or_else(|| metadata.get("authorization"))
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim().trim_start_matches("Bearer ").to_owned())
            .ok_or_else(|| Status::unauthenticated("missing api key"))?;

        let command = CommandRequest::try_from(request.into_inner())
            .map_err(|err| Status::invalid_argument(err.to_string()))?;

        if !self.authoriser.authorise(&api_key, &command) {
            return Err(Status::permission_denied("command not authorised"));
        }

        match self.handler.handle_command(&api_key, command.clone()).await {
            Ok(response) => Ok(Response::new(proto::CommandResponse::from(response))),
            Err(CommandError::NotAuthorised) => {
                Err(Status::permission_denied("command not authorised"))
            }
            Err(CommandError::InvalidPayload(err)) => Err(Status::invalid_argument(err)),
            Err(CommandError::ExecutionFailed(err)) => Err(Status::aborted(err)),
        }
    }
}

impl From<StatusSnapshot> for proto::StatusSnapshot {
    fn from(snapshot: StatusSnapshot) -> Self {
        Self {
            mode: snapshot.mode,
            revision: snapshot.revision,
            grids: snapshot.grids.into_iter().map(Into::into).collect(),
            metrics_endpoint: snapshot.metrics_endpoint.unwrap_or_default(),
        }
    }
}

impl From<GridStatus> for proto::GridStatus {
    fn from(status: GridStatus) -> Self {
        Self {
            id: status.id,
            controllers: status.controllers.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ControllerStatus> for proto::ControllerStatus {
    fn from(status: ControllerStatus) -> Self {
        Self {
            id: status.id,
            role: status.role,
            healthy: status.healthy,
            last_heartbeat_ms: status.last_heartbeat_ms,
        }
    }
}

impl TryFrom<proto::CommandRequest> for CommandRequest {
    type Error = anyhow::Error;

    fn try_from(value: proto::CommandRequest) -> Result<Self, Self::Error> {
        let parameters = value
            .parameters
            .map(struct_to_serde)
            .unwrap_or(serde_json::Value::Null);
        Ok(CommandRequest {
            target: value.target,
            command: value.command,
            parameters,
        })
    }
}

impl From<CommandResponse> for proto::CommandResponse {
    fn from(value: CommandResponse) -> Self {
        Self {
            accepted: value.accepted,
            message: value.message,
        }
    }
}

impl From<CommandRequest> for proto::CommandRequest {
    fn from(value: CommandRequest) -> Self {
        Self {
            target: value.target,
            command: value.command,
            parameters: Some(serde_value_to_struct(value.parameters)),
        }
    }
}

fn serde_value_to_struct(value: serde_json::Value) -> Struct {
    match value {
        serde_json::Value::Object(map) => Struct {
            fields: map
                .into_iter()
                .map(|(k, v)| (k, serde_value_to_prost(v)))
                .collect(),
        },
        other => Struct {
            fields: std::iter::once(("value".to_string(), serde_value_to_prost(other))).collect(),
        },
    }
}

fn serde_value_to_prost(value: serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value {
            kind: Some(Kind::NullValue(0)),
        },
        serde_json::Value::Bool(b) => Value {
            kind: Some(Kind::BoolValue(b)),
        },
        serde_json::Value::Number(num) => Value {
            kind: Some(Kind::NumberValue(num.as_f64().unwrap_or_default())),
        },
        serde_json::Value::String(s) => Value {
            kind: Some(Kind::StringValue(s)),
        },
        serde_json::Value::Array(arr) => Value {
            kind: Some(Kind::ListValue(prost_types::ListValue {
                values: arr.into_iter().map(serde_value_to_prost).collect(),
            })),
        },
        serde_json::Value::Object(map) => Value {
            kind: Some(Kind::StructValue(Struct {
                fields: map
                    .into_iter()
                    .map(|(k, v)| (k, serde_value_to_prost(v)))
                    .collect(),
            })),
        },
    }
}

fn struct_to_serde(struct_: Struct) -> serde_json::Value {
    let map = struct_
        .fields
        .into_iter()
        .map(|(k, v)| (k, prost_value_to_serde(v)))
        .collect();
    serde_json::Value::Object(map)
}

fn prost_value_to_serde(value: Value) -> serde_json::Value {
    match value.kind {
        Some(Kind::NullValue(_)) | None => serde_json::Value::Null,
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(b),
        Some(Kind::NumberValue(n)) => serde_json::Number::from_f64(n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s),
        Some(Kind::ListValue(list)) => {
            serde_json::Value::Array(list.values.into_iter().map(prost_value_to_serde).collect())
        }
        Some(Kind::StructValue(struct_)) => struct_to_serde(struct_),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest::{StaticApiKeyAuthoriser, StatusProvider};
    use tokio::time::{sleep, Duration};
    use tonic::metadata::MetadataValue;
    use tonic::transport::Channel;

    struct TestStatus;
    impl StatusProvider for TestStatus {
        fn snapshot(&self) -> StatusSnapshot {
            StatusSnapshot {
                mode: "simulation".into(),
                revision: "test".into(),
                grids: vec![],
                metrics_endpoint: None,
            }
        }
    }

    struct TestHandler;
    #[async_trait::async_trait]
    impl CommandHandler for TestHandler {
        async fn handle_command(
            &self,
            principal: &str,
            request: CommandRequest,
        ) -> Result<CommandResponse, CommandError> {
            assert_eq!(principal, "grpc-key");
            assert_eq!(request.command, "noop");
            Ok(CommandResponse {
                accepted: true,
                message: "ok".into(),
            })
        }
    }

    #[tokio::test]
    async fn grpc_status_and_command_flow() {
        let builder = GrpcServerBuilder::new(
            "127.0.0.1:0".parse().unwrap(),
            Arc::new(TestStatus),
            Arc::new(TestHandler),
            Arc::new(StaticApiKeyAuthoriser::new([(
                "grpc-key".into(),
                vec!["noop".into()],
            )])),
        );
        let handle = builder.spawn().await.unwrap();

        let channel = Channel::from_shared(format!("http://{}", handle.local_addr()))
            .unwrap()
            .connect()
            .await
            .unwrap();

        let mut status_client =
            proto::status_service_client::StatusServiceClient::new(channel.clone());
        let status = status_client
            .get_status(tonic::Request::new(Empty {}))
            .await
            .unwrap();
        assert_eq!(status.into_inner().mode, "simulation");

        let mut command_client = proto::command_service_client::CommandServiceClient::new(channel);
        let mut request = tonic::Request::new(proto::CommandRequest {
            target: "grid-a".into(),
            command: "noop".into(),
            parameters: None,
        });
        request
            .metadata_mut()
            .insert("x-api-key", MetadataValue::from_static("grpc-key"));
        let response = command_client.submit_command(request).await.unwrap();
        assert!(response.into_inner().accepted);

        sleep(Duration::from_millis(10)).await;
        handle.shutdown().await.unwrap();
    }
}
