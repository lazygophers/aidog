/// CLI 集成（`settings/coding_tools`）—— 对齐
/// `src/components/settings/CodingToolsSettings.tsx`。
///
/// 即时保存，没有保存按钮，也**不注册离页 guard**（React 文件头第 2 行写明）。
/// 7 项控件各写不同的外部文件：
///
/// | 控件 | 落点 |
/// |---|---|
/// | 扩展走 aidog 代理 | `~/.claude/config.json` primaryApiKey |
/// | 跳过首启引导 | `~/.claude.json` hasCompletedOnboarding |
/// | 日期改写防检测 | 镜像 `middleware_rule.enabled`（**不是** coding_tools_settings） |
/// | 内置工具兼容总开关 | settings scope=proxy key=builtin_tool_compat（与系统页同源） |
/// | 语言 | `~/.claude/settings.json` 的 language |
/// | 努力级别 | **双写** claude 顶层 effortLevel + codex model_reasoning_effort |
/// | 代理 | claude `env.HTTP_PROXY/HTTPS_PROXY/ALL_PROXY` 三键同值 + `NO_PROXY` |
///
/// 两条必须照搬、漏了就丢用户操作的规则：
/// ① **乐观翻转 → 失败回滚 → 常驻错误**（`runCommit`）。错误不自动消失，
///    否则用户看不到「开关其实没生效」。
/// ② **`dirty` 闸门**：用户一旦动过任一项，首屏那几个慢 `get()` 即便晚到也不许覆盖。
///    React 用 `dirtyRef` 防 StrictMode 双 mount，这里同理防慢响应回写。
library;

import '../invoke.dart';

/// 内置·日期格式改写防检测 规则名（与 Rust `schema.rs::builtin_rule_specs` 一致）。
const kDateRewriteRuleName = '内置·日期格式改写防检测';

/// 努力级别档位。claude 顶层 effortLevel 原生 low/medium/high/xhigh；
/// codex model_reasoning_effort 原生 minimal/low/medium/high/xhigh；
/// `max` 是 claude env 档位。双写时任一侧不识别由对应 CLI 自行忽略。
const List<String> kEffortOptions = ['low', 'medium', 'high', 'xhigh', 'max'];
const String kEffortDefault = 'medium';

/// 代理 URL 写这三个键（同值）。绝大多数代理三协议同址，合并成一个输入框。
const List<String> kProxyUrlKeys = ['HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY'];
const String kNoProxyKey = 'NO_PROXY';

/// 常见本地代理端口预置（Clash 7890 / 通用 8080 / SS 1080 / Clash Verge 7899）。
/// 下拉可选，也允许自由输入任意 URL。
const List<String> kProxyUrlPresets = [
  'http://127.0.0.1:7890',
  'http://127.0.0.1:8080',
  'http://127.0.0.1:1080',
  'http://127.0.0.1:7899',
];

/// 已移除的遗留键：自动压缩窗口。残留值会继续压过客户端自动判定，
/// 挂载时一次性从两侧剥掉。清理失败静默，下次挂载再试。
const String kLegacyClaudeCompactEnvKey = 'CLAUDE_CODE_AUTO_COMPACT_WINDOW';
const String kLegacyCodexCompactKey = 'model_auto_compact_token_limit';

/// 代理两个输入框的内容。
class ProxyDraft {
  const ProxyDraft({this.url = '', this.no = ''});

  final String url;
  final String no;

  ProxyDraft copyWith({String? url, String? no}) =>
      ProxyDraft(url: url ?? this.url, no: no ?? this.no);

  /// onBlur 是否需要提交：trim 后与已生效值有差异才写。
  bool differsFrom(ProxyDraft other) =>
      url.trim() != other.url.trim() || no.trim() != other.no.trim();
}

/// `runCommit` 成功后的提示文案（apply / clear 两种）。
class CodingToolsTexts {
  const CodingToolsTexts({
    required this.applied,
    required this.cleared,
    required this.writeFailed,
  });

  final String applied;
  final String cleared;

  /// React：`t("codingTools.writeFailed") + ": " + String(e)`。
  final String writeFailed;
}

class CodingToolsController {
  CodingToolsController({InvokeFn? invoke, this.onChanged})
    : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  bool applyToClaudePlugin = false;
  bool skipClaudeOnboarding = false;
  String language = '';
  String effort = '';
  bool btcGlobal = false;

  /// null = 加载中 / 规则没找到 → 开关不响应。
  bool? dateRewriteEnabled;
  int? dateRewriteRuleId;

  ProxyDraft proxyDraft = const ProxyDraft();

  /// 已经写进文件、确认生效的那份。onBlur 拿它和 draft 比。
  ProxyDraft proxyApplied = const ProxyDraft();

  bool loading = true;
  bool busy = false;
  String message = '';

  /// 写外部文件失败的**常驻**错误，不自动消失（与瞬时成功提示分开）。
  String error = '';

  /// 用户动过任一项 → 首屏的慢响应不许再覆盖本地值。
  bool dirty = false;

  bool _cancelled = false;

  void _notify() => onChanged?.call();

  /// 页面销毁时调，避免已卸载后还有慢响应回写。
  void dispose() => _cancelled = true;

  bool get _blocked => _cancelled || dirty;

  /// 四路并发加载，每一路独立失败：一路挂了不挡其余开关。
  Future<void> load() async {
    await Future.wait([
      _loadCodingToolsSettings(),
      _loadClaudeAndCodex(),
      _loadBtcGlobal(),
      _loadDateRewriteRule(),
    ]);
    _notify();
  }

  Future<void> _loadCodingToolsSettings() async {
    try {
      final s = _map(await _invoke('coding_tools_settings_get'));
      if (_blocked) return;
      applyToClaudePlugin = s['apply_to_claude_plugin'] as bool? ?? false;
      skipClaudeOnboarding = s['skip_claude_onboarding'] as bool? ?? false;
    } catch (e) {
      if (!_cancelled) error = '$e';
    } finally {
      if (!_cancelled) loading = false;
    }
  }

  Future<void> _loadClaudeAndCodex() async {
    Map<String, Object?>? cfg;
    Map<String, Object?>? cx;
    try {
      final r = await _invoke('settings_get', {
        'scope': 'global',
        'key': 'claude_code',
      });
      cfg = r is Map ? Map<String, Object?>.from(r) : null;
    } catch (_) {
      /* 读失败不阻塞其余：language 留空 */
    }
    try {
      final r = await _invoke('codex_config_read');
      cx = r is Map ? Map<String, Object?>.from(r) : null;
    } catch (_) {
      /* 同上 */
    }

    // 遗留键清理，**先于 dirty 闸门**执行 —— 它不碰界面状态，用户已操作也无妨。
    await _cleanLegacyKeys(cfg, cx);

    if (_blocked) return;
    final lang = cfg?['language'];
    if (lang is String) language = lang;

    final effortVal = cfg?['effortLevel'];
    final codexEffort = cx?['model_reasoning_effort'];
    final eff = (effortVal is String && effortVal.isNotEmpty)
        ? effortVal
        : (codexEffort is String && codexEffort.isNotEmpty ? codexEffort : '');
    effort = eff.isNotEmpty ? eff : kEffortDefault;

    // 三个 URL 键不一致时取**首个非空**作显示值。
    final env = cfg?['env'];
    final envMap = env is Map ? env : const {};
    final url =
        kProxyUrlKeys
            .map((k) => envMap[k])
            .whereType<String>()
            .where((v) => v.isNotEmpty)
            .firstOrNull ??
        '';
    final noV = envMap[kNoProxyKey];
    final loaded = ProxyDraft(url: url, no: noV is String ? noV : '');
    proxyDraft = loaded;
    proxyApplied = loaded;
  }

  Future<void> _cleanLegacyKeys(
    Map<String, Object?>? cfg,
    Map<String, Object?>? cx,
  ) async {
    final env = cfg?['env'];
    final envVal = env is Map ? env[kLegacyClaudeCompactEnvKey] : null;
    if (envVal != null && envVal != '') {
      try {
        await _writeClaudeConfigField((c) {
          final e = Map<String, Object?>.from((c['env'] as Map?) ?? const {});
          e.remove(kLegacyClaudeCompactEnvKey);
          return {...c, 'env': e};
        });
      } catch (_) {
        /* 静默：清理不阻塞加载 */
      }
    }
    if (cx != null && cx[kLegacyCodexCompactKey] != null) {
      final rest = Map<String, Object?>.from(cx)
        ..remove(kLegacyCodexCompactKey);
      try {
        await _invoke('codex_config_write', {'value': rest});
      } catch (_) {
        /* 同上 */
      }
    }
  }

  Future<void> _loadBtcGlobal() async {
    try {
      final v = await _invoke('settings_get', {
        'scope': 'proxy',
        'key': 'builtin_tool_compat',
      });
      if (_blocked) return;
      btcGlobal = v is Map && v['enabled'] == true;
    } catch (_) {
      /* 读失败保持默认关闭，不阻塞其余开关 */
    }
  }

  Future<void> _loadDateRewriteRule() async {
    try {
      final rules = await _invoke('middleware_list_rules');
      if (_blocked) return;
      final r = _findDateRule(rules);
      if (r != null) {
        dateRewriteRuleId = (r['id'] as num).toInt();
        dateRewriteEnabled = r['enabled'] == true;
      }
    } catch (_) {
      /* 读失败时 dateRewriteEnabled 保持 null → 开关不响应 */
    }
  }

  static Map<String, Object?>? _findDateRule(Object? rules) {
    if (rules is! List) return null;
    for (final e in rules) {
      if (e is! Map) continue;
      if (e['name'] == kDateRewriteRuleName && e['is_builtin'] == true) {
        return Map<String, Object?>.from(e);
      }
    }
    return null;
  }

  /// 统一提交模板：乐观翻转 → persist → 失败回滚 + 常驻错误，成功给 toast。
  /// `persist` 返回 true=应用 / false=清除，决定成功文案。
  Future<void> _runCommit(
    void Function() optimistic,
    void Function() revert,
    Future<bool> Function() persist,
    CodingToolsTexts texts,
  ) async {
    if (busy) return;
    dirty = true;
    message = '';
    error = '';
    optimistic();
    busy = true;
    _notify();
    try {
      final isApply = await persist();
      message = isApply ? texts.applied : texts.cleared;
    } catch (e) {
      revert();
      error = '${texts.writeFailed}: $e';
    } finally {
      busy = false;
      _notify();
    }
  }

  /// 读全量 claude_code → 改 → 写回 → best-effort sync。
  /// 对应 `services/api/settings.ts::writeClaudeConfigField`。
  /// **严禁自实现文件写**（PRD D2 硬约束），一律走这条。
  Future<void> _writeClaudeConfigField(
    Map<String, Object?> Function(Map<String, Object?> cfg) mutator,
  ) async {
    final raw = await _invoke('settings_get', {
      'scope': 'global',
      'key': 'claude_code',
    });
    final cfg = raw is Map
        ? Map<String, Object?>.from(raw)
        : <String, Object?>{};
    await _invoke('settings_set', {
      'input': {'scope': 'global', 'key': 'claude_code', 'value': mutator(cfg)},
    });
    try {
      await _invoke('sync_group_settings');
    } catch (_) {
      /* sync 失败不阻断（与 React 一致） */
    }
  }

  Future<void> toggleApplyToClaudePlugin(bool next, CodingToolsTexts texts) =>
      _runCommit(
        () => applyToClaudePlugin = next,
        () => applyToClaudePlugin = !next,
        () async {
          final u = _map(
            await _invoke('coding_tools_settings_set', {
              'applyToClaudePlugin': next,
            }),
          );
          applyToClaudePlugin = u['apply_to_claude_plugin'] as bool? ?? next;
          skipClaudeOnboarding =
              u['skip_claude_onboarding'] as bool? ?? skipClaudeOnboarding;
          return next;
        },
        texts,
      );

  Future<void> toggleSkipOnboarding(bool next, CodingToolsTexts texts) =>
      _runCommit(
        () => skipClaudeOnboarding = next,
        () => skipClaudeOnboarding = !next,
        () async {
          final u = _map(
            await _invoke('coding_tools_settings_set', {
              'skipClaudeOnboarding': next,
            }),
          );
          applyToClaudePlugin =
              u['apply_to_claude_plugin'] as bool? ?? applyToClaudePlugin;
          skipClaudeOnboarding = u['skip_claude_onboarding'] as bool? ?? next;
          return next;
        },
        texts,
      );

  /// 日期改写开关。`middleware_update_rule` 是**全量覆盖**，必须带上原规则的每个字段
  /// 只翻 enabled；先重新 list 拿最新规则，防本地态漂移把别的字段写回旧值。
  Future<void> toggleDateRewrite(bool next, CodingToolsTexts texts) {
    final prev = dateRewriteEnabled;
    return _runCommit(
      () => dateRewriteEnabled = next,
      () => dateRewriteEnabled = prev,
      () async {
        final rules = await _invoke('middleware_list_rules');
        final r = _findDateRule(rules);
        if (r == null) throw StateError('builtin date-rewrite rule not found');
        final updated = _map(
          await _invoke('middleware_update_rule', {
            'input': {
              'id': r['id'],
              'name': r['name'],
              'description': r['description'],
              'conditions': r['conditions'],
              'actions': r['actions'],
              'applies_to': r['applies_to'],
              'priority': r['priority'],
              'enabled': next,
            },
          }),
        );
        dateRewriteRuleId =
            (updated['id'] as num?)?.toInt() ?? dateRewriteRuleId;
        dateRewriteEnabled = updated['enabled'] == true;
        return next;
      },
      texts,
    );
  }

  /// 两级 AND 的第一级：关掉后所有平台级配置都不生效。与系统页同一个 setting。
  Future<void> toggleBtcGlobal(bool next, CodingToolsTexts texts) =>
      _runCommit(() => btcGlobal = next, () => btcGlobal = !next, () async {
        await _invoke('settings_set', {
          'input': {
            'scope': 'proxy',
            'key': 'builtin_tool_compat',
            'value': {'enabled': next},
          },
        });
        return next;
      }, texts);

  Future<void> setLanguage(String next, CodingToolsTexts texts) {
    if (busy || next == language) return Future.value();
    final prev = language;
    return _runCommit(() => language = next, () => language = prev, () async {
      await _writeClaudeConfigField((c) => {...c, 'language': next});
      return true;
    }, texts);
  }

  /// 努力级别：单值双写 claude `effortLevel` + codex `model_reasoning_effort`。
  Future<void> setEffort(String next, CodingToolsTexts texts) {
    if (busy || next == effort) return Future.value();
    final prev = effort;
    return _runCommit(() => effort = next, () => effort = prev, () async {
      await _writeClaudeConfigField((c) => {...c, 'effortLevel': next});
      final raw = await _invoke('codex_config_read');
      final cx = raw is Map
          ? Map<String, Object?>.from(raw)
          : <String, Object?>{};
      cx['model_reasoning_effort'] = next;
      await _invoke('codex_config_write', {'value': cx});
      return true;
    }, texts);
  }

  void setProxyUrl(String v) {
    proxyDraft = proxyDraft.copyWith(url: v);
    _notify();
  }

  void setProxyNo(String v) {
    proxyDraft = proxyDraft.copyWith(no: v);
    _notify();
  }

  /// onBlur 提交。trim 后与已生效值无差异就不写（避免每次失焦都打一遍文件）。
  /// URL 非空 → 三个键写同值；空 → 三个键一起删。NO_PROXY 同理。
  Future<void> commitProxy(CodingToolsTexts texts) {
    if (busy) return Future.value();
    final prev = proxyApplied;
    final next = proxyDraft;
    if (!next.differsFrom(prev)) return Future.value();
    return _runCommit(
      () {
        /* draft 保持用户输入；applied 等 persist 确认后再写 */
      },
      () {
        proxyDraft = prev;
        proxyApplied = prev;
      },
      () async {
        await _writeClaudeConfigField((c) {
          final env = Map<String, Object?>.from((c['env'] as Map?) ?? const {});
          final url = next.url.trim();
          for (final k in kProxyUrlKeys) {
            if (url.isNotEmpty) {
              env[k] = url;
            } else {
              env.remove(k);
            }
          }
          final no = next.no.trim();
          if (no.isNotEmpty) {
            env[kNoProxyKey] = no;
          } else {
            env.remove(kNoProxyKey);
          }
          return {...c, 'env': env};
        });
        proxyApplied = next;
        return true;
      },
      texts,
    );
  }

  /// 日期改写开关的禁用条件：忙，或规则还没读到 / 不存在。
  bool get dateRewriteDisabled => busy || dateRewriteEnabled == null;

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull => isEmpty ? null : first;
}
