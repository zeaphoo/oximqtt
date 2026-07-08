pub mod acl;
pub mod auth_jwt;
pub mod retainer;
pub mod sys_topic;

use crate::context::ServerContext;
use crate::Result;

pub async fn init_all(scx: &ServerContext) -> Result<()> {
    acl::init(scx).await?;
    retainer::init(scx).await?;
    auth_jwt::init(scx).await?;
    sys_topic::init(scx).await?;
    Ok(())
}
