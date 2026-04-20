use std::collections::BTreeMap;

use serde::Deserialize;
use zot_core::{EditorialNotice, SciteItemReport, SciteTally, ZotError, ZotResult};

use crate::http::HttpRuntime;

#[derive(Clone)]
pub struct SciteClient {
    client: reqwest::Client,
    base_url: String,
}

impl SciteClient {
    pub fn new(runtime: &HttpRuntime) -> Self {
        Self {
            client: runtime.client_clone(),
            base_url: std::env::var("ZOT_SCITE_API_BASE")
                .unwrap_or_else(|_| "https://api.scite.ai".to_string()),
        }
    }

    pub async fn get_report(&self, doi: &str) -> ZotResult<Option<SciteItemReport>> {
        let tally = self.get_tally(doi).await?;
        let paper = self.get_paper(doi).await?;
        if tally.is_none() && paper.is_none() {
            return Ok(None);
        }
        let title = paper
            .as_ref()
            .and_then(|paper| paper.title.clone())
            .unwrap_or_else(|| doi.to_string());
        let notices = paper
            .as_ref()
            .map(|paper| {
                paper
                    .editorial_notices
                    .iter()
                    .map(|notice| EditorialNotice {
                        notice_type: notice
                            .notice_type
                            .clone()
                            .unwrap_or_else(|| "notice".to_string()),
                        source: notice.source_doi.clone().or_else(|| notice.source.clone()),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(Some(SciteItemReport {
            doi: doi.to_string(),
            title,
            tally,
            notices,
        }))
    }

    pub async fn get_reports_batch(
        &self,
        dois: &[String],
    ) -> ZotResult<BTreeMap<String, SciteItemReport>> {
        let tally_map = self.get_tallies_batch(dois).await?;
        let paper_map = self.get_papers_batch(dois).await?;
        let mut results = BTreeMap::new();
        for doi in dois {
            if let Some(report) = self.merge_report(
                doi,
                tally_map.get(doi).cloned(),
                paper_map.get(doi).cloned(),
            ) {
                results.insert(doi.clone(), report);
            }
        }
        Ok(results)
    }

    async fn get_tally(&self, doi: &str) -> ZotResult<Option<SciteTally>> {
        let response = self
            .client
            .get(format!("{}/tallies/{}", self.base_url, doi))
            .send()
            .await
            .map_err(remote_err("scite-tally"))?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let payload: TallyPayload = response
            .json()
            .await
            .map_err(remote_err("scite-tally-json"))?;
        Ok(Some(payload.into()))
    }

    async fn get_tallies_batch(&self, dois: &[String]) -> ZotResult<BTreeMap<String, SciteTally>> {
        if dois.is_empty() {
            return Ok(BTreeMap::new());
        }
        let response = self
            .client
            .post(format!("{}/tallies", self.base_url))
            .json(&dois.iter().take(500).collect::<Vec<_>>())
            .send()
            .await
            .map_err(remote_err("scite-tallies"))?;
        if !response.status().is_success() {
            return Ok(BTreeMap::new());
        }
        let payload: TalliesBatchPayload = response
            .json()
            .await
            .map_err(remote_err("scite-tallies-json"))?;
        Ok(payload
            .tallies
            .into_iter()
            .map(|(doi, tally)| (doi, tally.into()))
            .collect())
    }

    async fn get_paper(&self, doi: &str) -> ZotResult<Option<PaperPayload>> {
        let response = self
            .client
            .get(format!("{}/papers/{}", self.base_url, doi))
            .send()
            .await
            .map_err(remote_err("scite-paper"))?;
        if !response.status().is_success() {
            return Ok(None);
        }
        response
            .json()
            .await
            .map(Some)
            .map_err(remote_err("scite-paper-json"))
    }

    async fn get_papers_batch(&self, dois: &[String]) -> ZotResult<BTreeMap<String, PaperPayload>> {
        if dois.is_empty() {
            return Ok(BTreeMap::new());
        }
        let response = self
            .client
            .post(format!("{}/papers", self.base_url))
            .json(&serde_json::json!({ "dois": dois.iter().take(500).collect::<Vec<_>>() }))
            .send()
            .await
            .map_err(remote_err("scite-papers"))?;
        if !response.status().is_success() {
            return Ok(BTreeMap::new());
        }
        let payload: PapersBatchPayload = response
            .json()
            .await
            .map_err(remote_err("scite-papers-json"))?;
        Ok(payload.papers)
    }

    fn merge_report(
        &self,
        doi: &str,
        tally: Option<SciteTally>,
        paper: Option<PaperPayload>,
    ) -> Option<SciteItemReport> {
        if tally.is_none() && paper.is_none() {
            return None;
        }
        let title = paper
            .as_ref()
            .and_then(|paper| paper.title.clone())
            .unwrap_or_else(|| doi.to_string());
        let notices = paper
            .as_ref()
            .map(|paper| {
                paper
                    .editorial_notices
                    .iter()
                    .map(|notice| EditorialNotice {
                        notice_type: notice
                            .notice_type
                            .clone()
                            .unwrap_or_else(|| "notice".to_string()),
                        source: notice.source_doi.clone().or_else(|| notice.source.clone()),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Some(SciteItemReport {
            doi: doi.to_string(),
            title,
            tally,
            notices,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TallyPayload {
    #[serde(default)]
    supporting: u32,
    #[serde(default, rename = "contradicting")]
    contrasting: u32,
    #[serde(default)]
    mentioning: u32,
    #[serde(default)]
    total: u32,
    #[serde(default, rename = "citingPublications")]
    citing_publications: Option<u32>,
}

impl From<TallyPayload> for SciteTally {
    fn from(value: TallyPayload) -> Self {
        Self {
            supporting: value.supporting,
            contrasting: value.contrasting,
            mentioning: value.mentioning,
            total: value.total,
            citing_publications: value.citing_publications,
        }
    }
}

#[derive(Debug, Deserialize)]
struct TalliesBatchPayload {
    #[serde(default)]
    tallies: BTreeMap<String, TallyPayload>,
}

#[derive(Debug, Clone, Deserialize)]
struct PaperPayload {
    title: Option<String>,
    #[serde(default, rename = "editorialNotices")]
    editorial_notices: Vec<EditorialNoticePayload>,
}

#[derive(Debug, Clone, Deserialize)]
struct EditorialNoticePayload {
    #[serde(default, rename = "type")]
    notice_type: Option<String>,
    #[serde(default, rename = "sourceDoi")]
    source_doi: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PapersBatchPayload {
    #[serde(default)]
    papers: BTreeMap<String, PaperPayload>,
}

fn remote_err(code: &'static str) -> impl Fn(reqwest::Error) -> ZotError {
    move |err| ZotError::Remote {
        code: code.to_string(),
        message: err.to_string(),
        hint: None,
        status: err.status().map(|status| status.as_u16()),
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorialNoticePayload, PaperPayload, SciteClient, TallyPayload};

    #[test]
    fn converts_tally_payload_into_core_shape() {
        let tally: zot_core::SciteTally = TallyPayload {
            supporting: 12,
            contrasting: 2,
            mentioning: 5,
            total: 19,
            citing_publications: Some(9),
        }
        .into();
        assert_eq!(tally.supporting, 12);
        assert_eq!(tally.contrasting, 2);
        assert_eq!(tally.mentioning, 5);
        assert_eq!(tally.total, 19);
        assert_eq!(tally.citing_publications, Some(9));
    }

    #[test]
    fn merges_scite_report_title_tally_and_notices() {
        let runtime = crate::http::HttpRuntime::default();
        let client = SciteClient::new(&runtime);
        let report = client.merge_report(
            "10.1038/nature12373",
            Some(zot_core::SciteTally {
                supporting: 7,
                contrasting: 1,
                mentioning: 3,
                total: 11,
                citing_publications: Some(4),
            }),
            Some(PaperPayload {
                title: Some("A landmark paper".to_string()),
                editorial_notices: vec![EditorialNoticePayload {
                    notice_type: Some("retraction".to_string()),
                    source_doi: Some("10.0000/retraction".to_string()),
                    source: None,
                }],
            }),
        );
        let report = match report {
            Some(report) => report,
            None => panic!("expected report"),
        };
        assert_eq!(report.title, "A landmark paper");
        assert_eq!(report.tally.as_ref().map(|tally| tally.total), Some(11));
        assert_eq!(report.notices.len(), 1);
        assert_eq!(report.notices[0].notice_type, "retraction");
        assert_eq!(
            report.notices[0].source.as_deref(),
            Some("10.0000/retraction")
        );
    }
}
