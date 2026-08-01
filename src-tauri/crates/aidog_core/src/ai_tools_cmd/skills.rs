use crate::gateway::{self, db::Db};
use tauri::State;
use std::sync::Arc;


use gateway::skills::{
    CachedSkills, CatalogEntry, SkillAgent, SkillScope, SkillsEnv, SkillsOpResult,
};

crate::tauri_command! {
    /// 探测 npx / node 环境（写操作前置）。
    pub async fn skills_check_env() -> Result<SkillsEnv, String> {
        Ok(gateway::skills::check_env())
    }
}

/// 读取上游代理设置并构造 npx/npm 用代理 URL（enabled → Some，否则 None）。
/// 所有 skills npx / catalog 抓取命令复用此值注入代理，使 skill 下载/查询尊重上游代理。
pub(crate) async fn skills_proxy_url(db: &State<'_, Db>) -> Option<String> {
    let db_arc = Arc::new(db.inner().clone());
    let settings = gateway::http_client::load_proxy_client_settings(&db_arc).await;
    gateway::skills::proxy_env_url(&settings)
}

crate::tauri_command! {
    /// 浏览 catalog（HTTP 抓 skills.sh，回退 npx find）。尊重上游代理。
    pub async fn skills_browse_catalog(db: State<'_, Db>) -> Result<Vec<CatalogEntry>, String> {
        let proxy = skills_proxy_url(&db).await;
        Ok(gateway::skills::browse_catalog(proxy.as_deref()).await)
    }
}

crate::tauri_command! {
    /// 搜索 catalog。尊重上游代理。
    pub async fn skills_search(db: State<'_, Db>, keyword: String) -> Result<Vec<CatalogEntry>, String> {
        tracing::debug!(command = "skills_search", keyword = %keyword, "command invoked");
        let proxy = skills_proxy_url(&db).await;
        Ok(gateway::skills::search(&keyword, proxy.as_deref()).await)
    }
}

crate::tauri_command! {
    /// 列指定 scope 下已装 skills —— **立即返回缓存**（内存→磁盘，命中即 0 子进程）。
    /// 冷启动（无缓存）返回空 + `stale=true`，前端据此显加载态并触发 `skills_list_refresh`。
    /// SWR 的 "stale" 半：不跑 npx，开页瞬间渲染。
    pub async fn skills_list_installed(scope: SkillScope) -> Result<CachedSkills, String> {
        Ok(gateway::skills::list_cached(&scope))
    }
}

crate::tauri_command! {
    /// 强制跑 `npx skills list --json`、更新内存+磁盘缓存、返回 fresh（`stale=false`）。尊重上游代理。
    /// SWR 的 "revalidate" 半：前端后台调用，完成后更新列表。
    pub async fn skills_list_refresh(db: State<'_, Db>, scope: SkillScope) -> Result<CachedSkills, String> {
        let proxy = skills_proxy_url(&db).await;
        Ok(gateway::skills::list_refresh(&scope, proxy.as_deref()))
    }
}

crate::tauri_command! {
    /// 为某 agent 启用 skill（shell out `npx skills add <path> -a <slug> [-g] -y`）。
    /// `path` = skill 本地安装路径（前端传 `SkillInfo.installed_path`），不依赖锁文件 source。
    /// 启用可能触发 skill 下载 → 尊重上游代理。
    pub async fn skills_enable(
        db: State<'_, Db>,
        name: String,
        path: String,
        agent: SkillAgent,
        scope: SkillScope,
    ) -> Result<SkillsOpResult, String> {
        tracing::debug!(command = "skills_enable", name = %name, "command invoked");
        let proxy = skills_proxy_url(&db).await;
        let res = gateway::skills::enable(&name, &path, agent, &scope, proxy.as_deref());
        if res.success {
            gateway::skills::invalidate(&scope);
        }
        Ok(res)
    }
}

crate::tauri_command! {
    /// 从 catalog 安装 skill 到多个 agent（shell out `npx skills add <id> -a <slug> -y`）。
    /// `id` = `owner/repo@skill`（CatalogEntry.id）。尊重上游代理。
    pub async fn skills_install(
        db: State<'_, Db>,
        id: String,
        agents: Vec<SkillAgent>,
        scope: SkillScope,
    ) -> Result<SkillsOpResult, String> {
        tracing::debug!(command = "skills_install", id = %id, agents = ?agents, "command invoked");
        let proxy = skills_proxy_url(&db).await;
        let res = gateway::skills::install(&id, &agents, &scope, proxy.as_deref());
        if res.success {
            gateway::skills::invalidate(&scope);
        }
        Ok(res)
    }
}

crate::tauri_command! {
    /// 列已装 skill 目录文件树（详情视图浏览，只读）。
    pub async fn skill_detail(installed_path: String) -> Result<gateway::skills::SkillDetail, String> {
        tracing::debug!(command = "skill_detail", path = %installed_path, "command invoked");
        gateway::skills::detail(&installed_path)
    }
}

crate::tauri_command! {
    /// 读 skill 内单文件（只读浏览）。带路径遍历防护 + 二进制检测 + 大小上限。
    pub async fn skill_read_file(
        installed_path: String,
        rel: String,
    ) -> Result<gateway::skills::SkillFileContent, String> {
        tracing::debug!(command = "skill_read_file", path = %installed_path, rel = %rel, "command invoked");
        gateway::skills::read_file(&installed_path, &rel)
    }
}

crate::tauri_command! {
    /// 为某 agent 关闭 skill（shell out `npx skills remove -s -a -y`）。尊重上游代理。
    pub async fn skills_disable(
        db: State<'_, Db>,
        name: String,
        agent: SkillAgent,
        scope: SkillScope,
    ) -> Result<SkillsOpResult, String> {
        tracing::debug!(command = "skills_disable", name = %name, "command invoked");
        let proxy = skills_proxy_url(&db).await;
        let res = gateway::skills::disable(&name, agent, &scope, proxy.as_deref());
        if res.success {
            gateway::skills::invalidate(&scope);
        }
        Ok(res)
    }
}

crate::tauri_command! {
    /// 更新已装 skills（shell out `npx skills update`）。尊重上游代理（拉取更新）。
    pub async fn skills_update(db: State<'_, Db>, scope: SkillScope) -> Result<SkillsOpResult, String> {
        let proxy = skills_proxy_url(&db).await;
        let res = gateway::skills::update(&scope, proxy.as_deref());
        if res.success {
            gateway::skills::invalidate(&scope);
        }
        Ok(res)
    }
}

crate::tauri_command! {
    /// 一键卸载当前 scope 下所有平台所有 skills（破坏性，前端二次确认）。
    pub async fn skills_uninstall_all(db: State<'_, Db>, scope: SkillScope) -> Result<SkillsOpResult, String> {
        let proxy = skills_proxy_url(&db).await;
        let res = gateway::skills::uninstall_all(&scope, proxy.as_deref());
        if res.success {
            gateway::skills::invalidate(&scope);
        }
        Ok(res)
    }
}

crate::tauri_command! {
    /// 卸载单一 skill（破坏性，前端二次确认）：删规范存储 + 所有 agent 启用配置。
    pub async fn skills_uninstall(
        db: State<'_, Db>,
        name: String,
        scope: SkillScope,
    ) -> Result<SkillsOpResult, String> {
        let proxy = skills_proxy_url(&db).await;
        let result = gateway::skills::uninstall(&name, &scope, proxy.as_deref());
        tracing::debug!(
            command = "skills_uninstall",
            name = %name,
            scope = ?scope,
            success = result.success,
            stdout = %result.stdout.trim(),
            stderr = %result.stderr.trim(),
            "npx remove result",
        );
        if result.success {
            gateway::skills::invalidate(&scope);
        }
        Ok(result)
    }
}

crate::tauri_command! {
    /// 对齐两 agent 的 skills 启用配置（使 `to` 与 `from` 完全一致）。
    pub async fn skills_align_agents(
        db: State<'_, Db>,
        from: SkillAgent,
        to: SkillAgent,
        scope: SkillScope,
    ) -> Result<SkillsOpResult, String> {
        let proxy = skills_proxy_url(&db).await;
        let res = gateway::skills::align_agents(from, to, &scope, proxy.as_deref());
        if res.success {
            gateway::skills::invalidate(&scope);
        }
        Ok(res)
    }
}

crate::tauri_command! {
    /// 为某 agent 启用当前 scope 全部已装 skills（只增不减）。
    pub async fn skills_enable_all(
        db: State<'_, Db>,
        agent: SkillAgent,
        scope: SkillScope,
    ) -> Result<SkillsOpResult, String> {
        let proxy = skills_proxy_url(&db).await;
        let res = gateway::skills::enable_all(agent, &scope, proxy.as_deref());
        if res.success {
            gateway::skills::invalidate(&scope);
        }
        Ok(res)
    }
}
