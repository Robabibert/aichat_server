use super::*;

use crate::{
    client::Model,
    function::{Functions, FunctionsFilter, SELECTED_ALL_FUNCTIONS},
};

use anyhow::{Context, Result};
use std::{fs::read_to_string, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Agent {
    name: String,
    config: AgentConfig,
    definition: AgentDefinition,
    #[serde(skip)]
    functions: Functions,
    #[serde(skip)]
    rag: Option<Arc<Rag>>,
    #[serde(skip)]
    model: Model,
}

impl Agent {
    pub async fn init(
        config: &GlobalConfig,
        name: &str,
        abort_signal: AbortSignal,
    ) -> Result<Self> {
        let definition_path = Config::agent_definition_file(name)?;
        let functions_path = Config::agent_functions_file(name)?;
        let rag_path = Config::agent_rag_file(name)?;
        let embeddings_dir = Config::agent_embeddings_dir(name)?;
        let definition = AgentDefinition::load(&definition_path)?;
        let functions = if functions_path.exists() {
            Functions::init(&functions_path)?
        } else {
            Functions::default()
        };
        let agent_config = config
            .read()
            .agents
            .iter()
            .find(|v| v.name == name)
            .cloned()
            .unwrap_or_else(|| AgentConfig::new(name));
        let model = {
            let config = config.read();
            match agent_config.model_id.as_ref() {
                Some(model_id) => Model::retrieve_chat(&config, model_id)?,
                None => config.current_model().clone(),
            }
        };
        let rag = if rag_path.exists() {
            Some(Arc::new(Rag::load(config, "rag", &rag_path)?))
        } else if embeddings_dir.is_dir() {
            println!("The agent uses an embeddings directory, initializing RAG...");
            let doc_path = embeddings_dir.display().to_string();
            Some(Arc::new(
                Rag::init(config, "rag", &rag_path, &[doc_path], abort_signal).await?,
            ))
        } else {
            None
        };

        Ok(Self {
            name: name.to_string(),
            config: agent_config,
            definition,
            functions,
            rag,
            model,
        })
    }

    pub fn export(&self) -> Result<String> {
        let mut value = serde_json::json!(self);
        value["functions_dir"] = Config::agent_functions_dir(&self.name)?
            .display()
            .to_string()
            .into();
        value["config_dir"] = Config::agent_config_dir(&self.name)?
            .display()
            .to_string()
            .into();
        let data = serde_yaml::to_string(&value)?;
        Ok(data)
    }

    pub fn banner(&self) -> String {
        self.definition.banner()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    pub fn functions(&self) -> &Functions {
        &self.functions
    }

    pub fn definition(&self) -> &AgentDefinition {
        &self.definition
    }

    pub fn rag(&self) -> Option<Arc<Rag>> {
        self.rag.clone()
    }

    pub fn conversation_staters(&self) -> &[String] {
        &self.definition.conversation_starters
    }
}

impl RoleLike for Agent {
    fn to_role(&self) -> Role {
        let mut role = Role::new("", &self.definition.instructions);
        role.sync(self);
        role
    }

    fn model(&self) -> &Model {
        &self.model
    }

    fn model_mut(&mut self) -> &mut Model {
        &mut self.model
    }

    fn temperature(&self) -> Option<f64> {
        self.config.temperature
    }

    fn top_p(&self) -> Option<f64> {
        self.config.top_p
    }

    fn functions_filter(&self) -> Option<FunctionsFilter> {
        if self.functions.is_empty() {
            None
        } else {
            Some(SELECTED_ALL_FUNCTIONS.into())
        }
    }

    fn set_model(&mut self, model: &Model) {
        self.config.model_id = Some(model.id());
        self.model = model.clone();
    }

    fn set_temperature(&mut self, value: Option<f64>) {
        self.config.temperature = value;
    }

    fn set_top_p(&mut self, value: Option<f64>) {
        self.config.top_p = value;
    }

    fn set_functions_filter(&mut self, _value: Option<FunctionsFilter>) {}
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AgentConfig {
    pub name: String,
    #[serde(rename(serialize = "model", deserialize = "model"))]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dangerously_functions_filter: Option<FunctionsFilter>,
}

impl AgentConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AgentDefinition {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    pub instructions: String,
    #[serde(default)]
    pub conversation_starters: Vec<String>,
}

impl AgentDefinition {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = read_to_string(path)
            .with_context(|| format!("Failed to read agent index file at '{}'", path.display()))?;
        let definition: Self = serde_yaml::from_str(&contents)
            .with_context(|| format!("Failed to load agent at '{}'", path.display()))?;
        Ok(definition)
    }

    fn banner(&self) -> String {
        let AgentDefinition {
            name,
            description,
            version,
            conversation_starters,
            ..
        } = self;
        let starters = if conversation_starters.is_empty() {
            String::new()
        } else {
            let starters = conversation_starters
                .iter()
                .map(|v| format!("- {v}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                r#"

## Conversation Starters
{starters}"#
            )
        };
        format!(
            r#"# {name} {version}
{description}{starters}"#
        )
    }
}

pub fn list_agents() -> Vec<String> {
    list_agents_impl().unwrap_or_default()
}

fn list_agents_impl() -> Result<Vec<String>> {
    let base_dir = Config::functions_dir()?;
    let contents = read_to_string(base_dir.join("agents.txt"))?;
    let agents = contents
        .split('\n')
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                None
            } else {
                Some(line.to_string())
            }
        })
        .collect();
    Ok(agents)
}
