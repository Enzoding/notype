use super::{api_base, transport, Profile, ProviderKind};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub transcription: bool,
}
#[derive(Default)]
pub struct ModelCatalog {
    pub transcription: Vec<ModelEntry>,
    pub polishing: Vec<ModelEntry>,
}
impl ModelCatalog {
    pub fn from_entries(entries: Vec<ModelEntry>) -> Self {
        let (transcription, polishing) = entries.into_iter().partition(|m| m.transcription);
        Self {
            transcription,
            polishing,
        }
    }
    pub fn visible(&self, transcription: bool) -> &[ModelEntry] {
        let items = if transcription {
            &self.transcription
        } else {
            &self.polishing
        };
        &items[..items.len().min(6)]
    }
}
pub fn fetch_models(profile: &Profile, key: &str) -> Result<Vec<ModelEntry>, String> {
    super::validate_base(&profile.base_url)?;
    let client = transport::client()?;
    let base = api_base(profile.kind, &profile.base_url);
    let mut models = Vec::new();
    let queries: &[&str] = if profile.kind == ProviderKind::OpenRouter {
        &["", "?output_modalities=transcription"]
    } else {
        &[""]
    };
    for query in queries {
        let response = client
            .get(format!("{base}/models{query}"))
            .bearer_auth(key)
            .send()
            .map_err(|_| "无法获取模型目录".to_string())?;
        if !response.status().is_success() {
            return Err(transport::error_message(response.status(), ""));
        }
        let value: serde_json::Value = response
            .json()
            .map_err(|_| "模型目录不是有效 JSON".to_string())?;
        let data = value["data"].as_array().ok_or("模型目录缺少 data")?;
        for item in data {
            if let Some(id) = item["id"].as_str() {
                let transcription = !query.is_empty()
                    || item["type"] == "transcription"
                    || item
                        .pointer("/architecture/output_modalities")
                        .and_then(|v| v.as_array())
                        .is_some_and(|a| a.iter().any(|v| v == "transcription"));
                let text_output = item
                    .pointer("/architecture/output_modalities")
                    .and_then(|v| v.as_array())
                    .is_some_and(|a| a.iter().any(|v| v == "text"));
                if !transcription && item["type"] != "language" && !text_output {
                    continue;
                }
                if !models
                    .iter()
                    .any(|m: &ModelEntry| m.id == id && m.transcription == transcription)
                {
                    models.push(ModelEntry {
                        id: id.into(),
                        transcription,
                    });
                }
            }
        }
    }
    if models.is_empty() {
        return Err("渠道没有返回可用模型".into());
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_catalogs_mount_only_a_bounded_slice_per_model_type() {
        let entries = (0..2000)
            .map(|n| ModelEntry {
                id: format!("synthetic/model-{n}"),
                transcription: n % 2 == 0,
            })
            .collect();
        let catalog = ModelCatalog::from_entries(entries);
        assert_eq!(catalog.transcription.len(), 1000);
        assert_eq!(catalog.polishing.len(), 1000);
        assert_eq!(catalog.visible(true).len(), 6);
        assert_eq!(catalog.visible(false).len(), 6);
        assert!(catalog.visible(true).iter().all(|m| m.transcription));
    }
}
