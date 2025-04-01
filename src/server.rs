use axum::http::{HeaderMap, HeaderName, HeaderValue};
use rmcp::{
    Error as McpError, RoleServer, ServerHandler, const_string,
    model::*,
    schemars::{self, JsonSchema},
    service::RequestContext,
    tool,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{env, fmt};

#[derive(Clone)]
pub struct Server {
    client: reqwest::Client,
    base_url: String,
    n8n_user: Option<String>,
    n8n_password: Option<String>,
}

impl Server {
    pub fn from_env() -> Self {
        let api_key = env::var("N8N_API_KEY").expect("N8N_API_KEY to exist");
        let base_url = env::var("N8N_BASE_URL").expect("N8N_BASE_URL to exist");
        let n8n_user = env::var("N8N_USER").ok();
        let n8n_password = env::var("N8N_PASSWORD").ok();

        let mut headers = HeaderMap::new();
        headers.insert("X-N8N-API-KEY", HeaderValue::from_str(&api_key).unwrap());

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        Self {
            client,
            base_url,
            n8n_user,
            n8n_password,
        }
    }
}

#[derive(Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename = "camelCase")]
pub struct WorkflowSettings {
    save_execution_progress: bool,
    save_manual_executions: bool,
    save_data_error_execution: AllOrNone,
    save_data_success_execution: AllOrNone,
    execution_timeout: u16,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename = "camelCase")]
pub struct RetrieveAllWorkflowParams {
    active: Option<bool>,
    tags: Option<String>,
    name: Option<String>,
    project_id: Option<String>,
    exclude_pinned_data: Option<String>,
    limit: Option<u8>,
    cursor: Option<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename = "camelCase")]
pub struct RetrieveSingleWorkflowParams {
    id: String,
    exclude_pinned_data: Option<bool>,
}

#[derive(Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename = "snake_case")]
#[serde(untagged)]
pub enum AllOrNone {
    #[default]
    All,
    None,
}

impl fmt::Display for AllOrNone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::None => write!(f, "none"),
        }
    }
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename = "camelCase")]
pub struct CreateWorkflowParams {
    name: String,
    nodes: serde_json::Value,
    connections: serde_json::Value,
    settings: WorkflowSettings,
    static_data: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename = "camelCase")]
#[serde(untagged)]
pub enum ExecutionStatus {
    Error,
    Success,
    Waiting,
}

#[tool(tool_box)]
impl Server {
    #[tool(description = "Retrieve all executions.")]
    async fn retrieve_all_executions(
        &self,
        #[tool(param)]
        #[schemars(description = "Whether or not to include the execution's detailed data.")]
        include_data: bool,
        #[tool(param)]
        #[schemars(
            description = "The status of an execution. Can either be: 'error' | 'success' | 'waiting'"
        )]
        status: ExecutionStatus,
        #[tool(param)]
        #[schemars(description = "Workflow ID to filter executions by. Optional.")]
        workflow_id: Option<String>,
        #[tool(param)]
        #[schemars(description = "Project ID to filter executions by. Optional.")]
        project_id: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "The maximum number of items to return. The absolute maximum is 250 - if you go above this, you will receive an error."
        )]
        limit: u8,
        #[tool(param)]
        #[schemars(
            description = "Page number, used for pagination. You can either set this to navigate the page, or leave it blank to get the first page."
        )]
        cursor: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/api/v1/executions", self.base_url);

        let json_object = json!({
            "includeData": include_data,
            "status": status,
            "workflowId": workflow_id,
            "projectId": project_id,
            "limit": limit,
            "cursor": cursor
        });

        let res = self.client.get(url).query(&json_object).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Retrieve an execution by ID.")]
    async fn retrieve_execution_by_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The execution ID to use.")]
        execution_id: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/api/v1/workflows/{execution_id}", self.base_url);

        let res = self.client.get(url).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Deletes an execution by ID.")]
    async fn delete_execution_by_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The execution ID to use.")]
        execution_id: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/api/v1/workflows/{execution_id}", self.base_url);

        let res = self.client.delete(url).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Create a new workflow.")]
    async fn create_workflow(
        &self,
        #[tool(param)]
        #[schemars(description = "The name of your workflow.")]
        name: String,
        #[tool(param)]
        #[schemars(description = "The nodes you want to use in your workflow.")]
        nodes: serde_json::Value,
        #[tool(param)]
        #[schemars(description = "The connections you want for your workflow.")]
        connections: serde_json::Value,
    ) -> Result<CallToolResult, rmcp::Error> {
        let url = format!("{}/api/v1/workflows", self.base_url);

        let settings = WorkflowSettings::default();

        let json_object = json!({
            "name": name,
            "nodes": nodes,
            "connections": connections,
            "settings": settings,
            "staticData": "null"
        });

        let res = self.client.post(url).json(&json_object).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(
        description = "Retrieve all workflows (with optional parameters for filtering).

            Note that in order for a returned workflow to be runnable, the first node of a workflow entry MUST be of type 'n8n-nodes-base.webhook'.
            "
    )]
    async fn retrieve_workflows(
        &self,
        #[tool(param)]
        #[schemars(
            description = "The parameters to fetch workflows by. If you leave all fields as blank, it will attempt to fetch everything.

                Note that the pages can be navigated by adjusting the cursor value."
        )]
        retrieve_workflow_params: RetrieveAllWorkflowParams,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/api/v1/workflows", self.base_url);

        let res = self
            .client
            .get(url)
            .query(&retrieve_workflow_params)
            .send()
            .await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Retrieve the details of a single workflow by its ID.")]
    async fn retrieve_workflow_by_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The workflow ID to fetch.")]
        workflow_id: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/api/v1/workflows/{workflow_id}", self.base_url);

        let res = self.client.get(url).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Delete a single workflow by its ID.")]
    async fn delete_workflow_by_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The workflow ID to use.")]
        workflow_id: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/api/v1/workflows/{workflow_id}", self.base_url);

        let res = self.client.delete(url).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Activates a single workflow by ID.")]
    async fn activate_workflow_by_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The workflow ID to use.")]
        workflow_id: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/api/v1/workflows/{workflow_id}/activate", self.base_url);

        let res = self.client.post(url).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Deactivates a single workflow by ID.")]
    async fn deactivate_workflow_by_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The workflow ID to use.")]
        workflow_id: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!(
            "{}/api/v1/workflows/{workflow_id}/deactivate",
            self.base_url
        );

        let res = self.client.post(url).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Updates a workflow.")]
    async fn update_workflow_by_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The ID of the workflow to be updated.")]
        workflow_id: String,
        #[tool(param)]
        #[schemars(description = "The name of your workflow.")]
        name: String,
        #[tool(param)]
        #[schemars(description = "The nodes you want to use in your workflow.")]
        nodes: serde_json::Value,
        #[tool(param)]
        #[schemars(description = "The connections you want for your workflow.")]
        connections: serde_json::Value,
    ) -> Result<CallToolResult, rmcp::Error> {
        let url = format!("{}/api/v1/workflows/{workflow_id}", self.base_url);

        let settings = WorkflowSettings::default();

        let json_object = json!({
            "name": name,
            "nodes": nodes,
            "connections": connections,
            "settings": settings,
            "staticData": "null"
        });

        let res = self.client.put(url).json(&json_object).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Gets the tags of a single workflow by ID.")]
    async fn get_workflow_tags_by_workflow_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The workflow ID to use.")]
        workflow_id: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/api/v1/workflows/{workflow_id}/tags", self.base_url);

        let res = self.client.get(url).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Updates the tags of a single workflow to the provided tags.")]
    async fn update_workflow_tags_by_workflow_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The workflow ID to use.")]
        workflow_id: String,
        #[tool(param)]
        #[schemars(description = "The IDs of the tags to assign to this workflow.")]
        tags: Vec<Id>,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/api/v1/workflows/{workflow_id}/tags", self.base_url);

        let res = self.client.put(url).json(&json!(tags)).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Run a workflow.

            If you don't have a workflow ID to use, retrieve all workflows and search for an appropriate
            workflow to run (according to the user's prompt.)")]
    async fn run_workflow(
        &self,
        #[tool(param)]
        #[schemars(description = "The path of the webhook (that belongs to the workflow to run).")]
        webhook_path: String,
        #[tool(param)]
        #[schemars(
            description = "The data to pass to the webhook. If the user has not explicitly asked for data to be sent, leave this as None."
        )]
        data: Option<serde_json::Value>,
    ) -> Result<CallToolResult, rmcp::Error> {
        let url = format!("{}/webhook/{webhook_path}", self.base_url);

        let res = if let Some(data) = data {
            self.client.post(url).json(&data).send().await
        } else {
            self.client.get(url).send().await
        };

        match res {
            Ok(res) => Ok(CallToolResult::success(vec![Content::text(
                "Workflow run successful",
            )])),
            Err(err) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Workflow error: {err}"
            ))])),
        }
    }

    #[tool(description = "Create a tag.")]
    async fn create_tag(
        &self,
        #[tool(param)]
        #[schemars(description = "The name to use.")]
        name: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/tags", self.base_url);

        let res = self
            .client
            .post(url)
            .json(&json!({"name": name}))
            .send()
            .await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Retrieve all tags.")]
    async fn retrieve_tags(
        &self,
        #[tool(param)]
        #[schemars(
            description = "The cursor to be used for navigating between pages. Note that this isn't provided by the user - to get the next cursor you have to run this function first."
        )]
        cursor: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/tags", self.base_url);

        let res = self
            .client
            .post(url)
            .query(&json!({"cursor": cursor}))
            .send()
            .await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Retrieve a tag by ID.")]
    async fn retrieve_tag_by_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The tag ID to use.")]
        tag_id: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/tags/{tag_id}", self.base_url);

        let res = self.client.get(url).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Delete a tag by its ID.")]
    async fn delete_tag_by_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The ID of the tag to delete.")]
        tag_id: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/tags/{tag_id}", self.base_url);

        let res = self.client.delete(url).send().await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }

    #[tool(description = "Updates a tag by its ID.")]
    async fn update_tag_by_id(
        &self,
        #[tool(param)]
        #[schemars(description = "The tag ID to use.")]
        tag_id: String,
        #[tool(param)]
        #[schemars(description = "The name to use.")]
        name: String,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/tags/{tag_id}", self.base_url);

        let res = self
            .client
            .put(url)
            .json(&json!({"name": name}))
            .send()
            .await;

        let res = match res {
            Ok(res) => res,
            Err(err) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Workflow error: {err}"
                ))]));
            }
        };

        // This should essentially never panic as the response from n8n should always be 100% correctly formatted JSON
        let res = res.json::<serde_json::Value>().await.unwrap();
        let json_as_string = serde_json::to_string_pretty(&res).unwrap();

        Ok(CallToolResult::success(vec![Content::text(json_as_string)]))
    }
}

#[tool(tool_box)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("This server provides a tool that can interact with a n8n server.

                n8n (or 'node-mation') is a service for creating automation that can either be used on n8n's cloud offfering or self-hosted.
                Using this server, users can create, retrieve (in bulk and by id), update and delete workflows and retrieve the tags for a given workflow.
                They can also additionally retrieve (in bulk and by id) executions and additionally delete executions.

                Users can also additionally retrieve (in bulk and by id), create, update and delete tags.

                If the user requests you to update or run a workflow (or assign a tag), you might need to either fetch all workflows first to see what workflows are possible.
                ".to_string()),
        }
    }
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Id {
    id: String,
}
