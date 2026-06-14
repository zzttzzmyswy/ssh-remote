use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt as _;
use uuid::Uuid;

use crate::proto::Message as ProtoMessage;
use crate::relay::SharedState;

pub async fn sse_handler(
    State(state): State<Arc<SharedState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let token = params.get("token").cloned().unwrap_or_default();

    let (_session_id, _permission) = match state.sessions.authenticate(&token).await {
        Some(result) => result,
        None => {
            return Sse::new(
                futures_util::stream::once(std::future::ready(Ok::<_, Infallible>(
                    Event::default()
                        .event("error")
                        .data(r#"{"code":"AUTH_INVALID_TOKEN","message":"Invalid token"}"#),
                ))),
            )
            .into_response();
        }
    };

    let stream = async_stream::stream! {
        yield Ok::<_, Infallible>(Event::default()
            .event("endpoint")
            .data(format!("/mcp/messages?token={}", token)));

        let init_result = json!({
            "jsonrpc": "2.0",
            "id": 0,
            "result": {
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "ssh-remote",
                    "version": "0.1.0"
                },
                "capabilities": {
                    "tools": {}
                }
            }
        }).to_string();

        yield Ok::<_, Infallible>(Event::default()
            .event("message")
            .data(init_result));

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
            yield Ok::<_, Infallible>(Event::default()
                .event("heartbeat")
                .data("{}"));
        }
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

pub async fn messages_handler(
    State(state): State<Arc<SharedState>>,
    Query(params): Query<HashMap<String, String>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let token = params.get("token").map(|s| s.as_str()).unwrap_or("");

    let (session_id, _permission) = match state.sessions.authenticate(token).await {
        Some(result) => result,
        None => {
            return Json(json!({
                "jsonrpc": "2.0",
                "id": body.get("id"),
                "error": {
                    "code": -32001,
                    "message": "Invalid token"
                }
            }))
            .into_response();
        }
    };

    let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let request_id = body.get("id").cloned().unwrap_or(Value::Null);

    match method {
        "initialize" => Json(json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "ssh-remote",
                    "version": "0.1.0"
                },
                "capabilities": {
                    "tools": {}
                }
            }
        }))
        .into_response(),

        "tools/list" => Json(json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "tools": [
                    {
                        "name": "exec",
                        "description": "Execute a shell command on the remote machine",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "cmd": {
                                    "type": "string",
                                    "description": "The shell command to execute"
                                },
                                "timeout_ms": {
                                    "type": "number",
                                    "description": "Optional timeout in milliseconds"
                                }
                            },
                            "required": ["cmd"]
                        }
                    },
                    {
                        "name": "file_read",
                        "description": "Read the content of a file on the remote machine",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "Absolute path to the file to read"
                                }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "file_write",
                        "description": "Write content to a file on the remote machine",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "Absolute path to the file to write"
                                },
                                "content": {
                                    "type": "string",
                                    "description": "Content to write to the file"
                                }
                            },
                            "required": ["path", "content"]
                        }
                    },
                    {
                        "name": "file_list",
                        "description": "List contents of a directory on the remote machine",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "Absolute path to the directory to list"
                                }
                            },
                            "required": ["path"]
                        }
                    }
                ]
            }
        }))
        .into_response(),

        "tools/call" => {
            let params_obj = body.get("params").unwrap_or(&Value::Null);
            let tool_name = params_obj
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let arguments = params_obj.get("arguments").unwrap_or(&Value::Null);

            let (msg_type, payload) = match tool_name {
                "exec" => {
                    let cmd = arguments
                        .get("cmd")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let timeout_ms = arguments
                        .get("timeout_ms")
                        .and_then(|v| v.as_u64());
                    let payload = if let Some(timeout) = timeout_ms {
                        json!({"cmd": cmd, "timeout_ms": timeout})
                    } else {
                        json!({"cmd": cmd})
                    };
                    ("mcp:exec", payload)
                }
                "file_read" => {
                    let path = arguments
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    ("fs:read", json!({"path": path}))
                }
                "file_write" => {
                    let path = arguments
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let content = arguments
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    ("fs:write", json!({"path": path, "content": content}))
                }
                "file_list" => {
                    let path = arguments
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    ("fs:list", json!({"path": path}))
                }
                _ => {
                    return Json(json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "error": {
                            "code": -32601,
                            "message": format!("Unknown tool: {}", tool_name)
                        }
                    }))
                    .into_response();
                }
            };

            let (response_tx, mut response_rx) = oneshot::channel::<String>();
            let mcp_request_id = Uuid::new_v4().to_string();

            {
                let mut pending = state.pending_mcp.write().await;
                pending.insert(mcp_request_id.clone(), (session_id.clone(), response_tx));
            }

            let mcp_request_msg = json!({
                "type": msg_type,
                "session_id": session_id,
                "payload": payload,
                "_mcp_request_id": mcp_request_id
            })
            .to_string();

            let sent = {
                let broadcast = state.agent_broadcast.read().await;
                if let Some(channel_map) = broadcast.get(&session_id) {
                    let mut sent = false;
                    for tx in &channel_map.senders {
                        let _ = tx.send(mcp_request_msg.clone());
                        sent = true;
                    }
                    sent
                } else {
                    false
                }
            };

            if !sent {
                let mut pending = state.pending_mcp.write().await;
                pending.remove(&mcp_request_id);
                return Json(json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {
                        "content": [{
                            "type": "text",
                            "text": "Error: No agent connected for this session"
                        }],
                        "isError": true
                    }
                }))
                .into_response();
            }

            let timeout_duration = match tool_name {
                "exec" => {
                    let timeout_ms = arguments
                        .get("timeout_ms")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(30000);
                    tokio::time::Duration::from_millis(timeout_ms)
                }
                _ => tokio::time::Duration::from_secs(30),
            };

            match tokio::time::timeout(timeout_duration, &mut response_rx).await {
                Ok(Ok(response_text)) => Json(json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {
                        "content": [{
                            "type": "text",
                            "text": response_text
                        }],
                        "isError": false
                    }
                }))
                .into_response(),
                _ => {
                    let mut pending = state.pending_mcp.write().await;
                    pending.remove(&mcp_request_id);
                    Json(json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": "Error: Request timed out waiting for agent response"
                            }],
                            "isError": true
                        }
                    }))
                    .into_response()
                }
            }
        }

        _ => Json(json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {
                "code": -32601,
                "message": format!("Unknown method: {}", method)
            }
        }))
        .into_response(),
    }
}
