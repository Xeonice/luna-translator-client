use crate::contracts::*;
#[derive(Debug, Clone)]
pub struct CredentialSelection {
    pub credential_id: String,
    pub vault_ref: String,
    pub pool_id: String,
    pub pool_revision: u32,
    pub member_id: String,
    pub masked_identity: String,
}
pub fn validate_pool(pool: &CredentialPoolInput) -> AppResult<()> {
    if !["selected", "round_robin", "weighted"].contains(&pool.policy.as_str())
        || pool.members.is_empty()
    {
        return Err(AppError::new(
            "invalid_pool",
            "credentials",
            "请选择支持的池策略并添加凭据",
        ));
    }
    let enabled = pool.members.iter().filter(|m| m.enabled).count();
    if enabled == 0 || (pool.policy == "selected" && enabled != 1) {
        return Err(AppError::new(
            "invalid_pool_selection",
            "credentials",
            "单选策略必须恰好选择一项；轮询策略至少启用一项",
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for m in &pool.members {
        super::require_id(&m.credential_id)?;
        if m.weight == 0 || !ids.insert(&m.credential_id) {
            return Err(AppError::new(
                "invalid_pool_member",
                "credentials",
                "凭据不得重复且权重必须为正整数",
            ));
        }
    }
    Ok(())
}
/// Deterministic weighted rotation; changing policy or weights creates a new immutable pool revision.
pub fn choose_member<'a>(
    policy: &str,
    members: &'a [CredentialPoolMember],
    cursor: u64,
) -> AppResult<(&'a CredentialPoolMember, u64)> {
    if !["selected", "round_robin", "weighted"].contains(&policy)
        || members.iter().any(|m| m.weight == 0)
    {
        return Err(AppError::new(
            "invalid_pool_policy",
            "credentials",
            "凭据池策略或权重无效",
        ));
    }
    let active = members.iter().filter(|m| m.enabled).collect::<Vec<_>>();
    if active.is_empty() {
        return Err(AppError::new(
            "no_active_credential",
            "credentials",
            "凭据池没有可用成员",
        ));
    }
    if policy == "selected" {
        if active.len() != 1 {
            return Err(AppError::new(
                "invalid_pool_selection",
                "credentials",
                "单选池具有多项启用成员",
            ));
        }
        return Ok((active[0], 0));
    }
    let total = active
        .iter()
        .try_fold(0u64, |sum, m| {
            sum.checked_add(if policy == "weighted" {
                u64::from(m.weight)
            } else {
                1
            })
        })
        .ok_or_else(|| AppError::new("invalid_pool_weight", "credentials", "凭据权重总数溢出"))?;
    let at = cursor % total;
    let mut boundary = 0u64;
    for member in active {
        boundary += if policy == "weighted" {
            u64::from(member.weight)
        } else {
            1
        };
        if at < boundary {
            return Ok((member, (at + 1) % total));
        }
    }
    Err(AppError::new(
        "invalid_pool_policy",
        "credentials",
        "无法解析凭据池策略",
    ))
}

/// Trusted SQL lifecycle facts for one short-lived Vault read. Never part of an ordinary snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialLeaseMetadata {
    pub credential: super::model_config::credentials::CredentialVersionRef,
    pub vault_ref: String,
    pub storage: super::model_config::credentials::CredentialStorageMetadata,
    pub lifecycle_revision: u32,
    pub pool_id: String,
    pub pool_revision: u32,
    pub member_id: String,
}
