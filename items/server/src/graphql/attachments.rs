//! Attachments, as akter's fields show them.

use async_graphql::{Context, Enum, ID, Object, SimpleObject};
use chrono::{DateTime, Utc};
use kuria_engine::sbom::{SbomFormat as EngineFormat, SbomSummary};
use kuria_store::attachments::{self, AttachmentRow};
use uuid::Uuid;

use super::Principal;
use super::people::User;
use crate::akter::Akter;
use crate::attachments::hex;

/// A file uploaded to Kuria, held by an akt's field.
pub struct Attachment(pub Uuid);

impl Attachment {
    async fn row(&self, ctx: &Context<'_>) -> async_graphql::Result<AttachmentRow> {
        let tenant: Uuid = ctx.data::<Principal>()?.tenant.into();
        let mut conn = ctx.data::<Akter>()?.pool().get().await?;
        attachments::find(&mut conn, tenant, &[self.0])
            .await?
            .pop()
            .ok_or_else(|| async_graphql::Error::new("the attachment is gone"))
    }
}

#[Object]
impl Attachment {
    async fn id(&self) -> ID {
        ID(self.0.to_string())
    }

    /// Where its bytes are downloaded from, relative to Kuria's URL.
    async fn url(&self) -> String {
        format!("/attachments/{}", self.0)
    }

    async fn name(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        Ok(self.row(ctx).await?.name)
    }

    async fn media_type(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        Ok(self.row(ctx).await?.media_type)
    }

    /// Its size in bytes.
    async fn size(&self, ctx: &Context<'_>) -> async_graphql::Result<i64> {
        Ok(self.row(ctx).await?.size)
    }

    /// Its SHA-256, in lowercase hexadecimal.
    async fn sha256(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        Ok(hex(&self.row(ctx).await?.sha256))
    }

    async fn uploaded_by(&self, ctx: &Context<'_>) -> async_graphql::Result<User> {
        Ok(User(self.row(ctx).await?.uploaded_by))
    }

    async fn uploaded_at(&self, ctx: &Context<'_>) -> async_graphql::Result<DateTime<Utc>> {
        Ok(self.row(ctx).await?.uploaded_at)
    }

    /// The SBOM's summary, if Kuria recognised it as one.
    async fn sbom(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Sbom>> {
        Ok(match self.row(ctx).await?.sbom {
            Some(json) => Some(Sbom::from(serde_json::from_value::<SbomSummary>(json)?)),
            None => None,
        })
    }
}

/// What Kuria recognised of an SBOM.
#[derive(SimpleObject)]
pub struct Sbom {
    format: SbomFormat,
    spec_version: String,
    name: Option<String>,
    /// How many packages (SPDX) or components (CycloneDX) it lists.
    components: usize,
}

impl From<SbomSummary> for Sbom {
    fn from(summary: SbomSummary) -> Self {
        Self {
            format: match summary.format {
                EngineFormat::Spdx => SbomFormat::Spdx,
                EngineFormat::Cyclonedx => SbomFormat::Cyclonedx,
            },
            spec_version: summary.spec_version,
            name: summary.name,
            components: summary.components,
        }
    }
}

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum SbomFormat {
    Spdx,
    Cyclonedx,
}
