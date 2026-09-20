/// MITM 解密隧道配置（`settings/mitm`）—— 对齐
/// `src/components/settings/MitmConfig.tsx`。
///
/// 这一页的分支最多，也最容易搬漏。按控件逐条记在这里，方便与 React 对读：
///
/// | 控件 | 禁用条件 | 校验 / 边界 |
/// |---|---|---|
/// | 主开关 | `busy` | — |
/// | 安装 CA | `busy \|\| !caPresent` | exit=0 才回写已安装；非 0 走分类 + 手动兜底 |
/// | 添加规则 | `busy \|\| 输入去空白后为空` | suffix 类型 strip 所有前导点；strip 后为空不加 |
/// | 导入默认 | `busy` | 幂等，可重复点 |
/// | 清空 | `busy \|\| 白名单为空` | **先弹确认框**，不可撤销 |
/// | URL 测试 | `busy \|\| 输入去空白后为空` | — |
///
/// 字段名照 `src/services/api/mitm.ts` 的 `MitmStatus` / `CaCommandSpec` /
/// `CaInstallOutcome`（这三个是手写类型，不在 generated 目录）。
library;

import '../invoke.dart';

/// 4 个合法匹配方式。后端 `valid_rule_type` 会再校验一次。
enum WhitelistRuleType {
  domain,
  suffix,
  keyword,
  ipcidr;

  String get wire => name;
  static WhitelistRuleType parse(Object? v) => WhitelistRuleType.values
      .firstWhere((e) => e.name == v, orElse: () => WhitelistRuleType.suffix);
}

class WhitelistEntry {
  const WhitelistEntry({
    required this.hostPattern,
    required this.enabled,
    required this.source,
    required this.ruleType,
  });

  final String hostPattern;
  final bool enabled;

  /// `default` = 内置 37 条之一，`user` = 用户自己加的。
  final String source;
  final WhitelistRuleType ruleType;

  factory WhitelistEntry.fromJson(Map<String, Object?> j) => WhitelistEntry(
        hostPattern: '${j['host_pattern']}',
        enabled: j['enabled'] as bool? ?? false,
        source: j['source'] as String? ?? 'user',
        // React：`e.rule_type ?? "suffix"`。
        ruleType: WhitelistRuleType.parse(j['rule_type']),
      );
}

class MitmStatus {
  const MitmStatus({
    required this.enabled,
    required this.caPresent,
    required this.caInstalled,
    required this.caFingerprint,
    required this.whitelist,
  });

  final bool enabled;
  final bool caPresent;
  final bool caInstalled;
  final String caFingerprint;
  final List<WhitelistEntry> whitelist;

  factory MitmStatus.fromJson(Map<String, Object?> j) => MitmStatus(
        enabled: j['enabled'] as bool? ?? false,
        caPresent: j['ca_present'] as bool? ?? false,
        caInstalled: j['ca_installed'] as bool? ?? false,
        caFingerprint: j['ca_fingerprint'] as String? ?? '',
        whitelist: (j['whitelist'] as List? ?? const [])
            .map((e) => WhitelistEntry.fromJson(Map<String, Object?>.from(e as Map)))
            .toList(),
      );
}

/// 手动安装兜底弹窗的数据（`CaCommandSpec`）。
class CaCommandSpec {
  const CaCommandSpec({required this.name, required this.caPemPath, required this.manualDisplay});

  final String name;
  final String caPemPath;

  /// 给用户复制到终端的真实 sudo 命令。
  final String manualDisplay;

  factory CaCommandSpec.fromJson(Map<String, Object?> j) => CaCommandSpec(
        name: '${j['name']}',
        caPemPath: '${j['ca_pem_path']}',
        manualDisplay: '${j['manual_display']}',
      );
}

/// `mitm_install_ca` 的执行结果。`code` 被信号杀死时为 null。
class CaInstallOutcome {
  const CaInstallOutcome({
    required this.code,
    required this.stdout,
    required this.stderr,
    required this.program,
  });

  final int? code;
  final String stdout;
  final String stderr;
  final String program;

  factory CaInstallOutcome.fromJson(Map<String, Object?> j) => CaInstallOutcome(
        code: (j['code'] as num?)?.toInt(),
        stdout: j['stdout'] as String? ?? '',
        stderr: j['stderr'] as String? ?? '',
        program: j['program'] as String? ?? '',
      );
}

/// 一次 URL 命中测试的结果行。
class MatchedRule {
  const MatchedRule({required this.hostPattern, required this.ruleType});

  final String hostPattern;
  final WhitelistRuleType ruleType;

  factory MatchedRule.fromJson(Map<String, Object?> j) => MatchedRule(
        hostPattern: '${j['host_pattern']}',
        ruleType: WhitelistRuleType.parse(j['rule_type']),
      );
}

/// CA 安装失败的分类文案。UI 传进来，这一层不碰 i18n。
class MitmInstallTexts {
  const MitmInstallTexts({
    required this.cancel,
    required this.authFail,
    required this.noAgent,
    required this.failed,
  });

  final String cancel;
  final String authFail;
  final String noAgent;

  /// 接 exit code 返回成品文案（React 的 `装信任库失败（exit={{code}}）`）。
  final String Function(String code) failed;
}

class MitmController {
  MitmController({InvokeFn? invoke, this.onChanged}) : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  MitmStatus? status;
  bool loading = true;
  bool busy = false;
  String error = '';
  String message = '';
  String newPattern = '';

  /// 默认 suffix；**添加成功后不重置**，方便连加同类型规则。
  WhitelistRuleType newRuleType = WhitelistRuleType.suffix;

  CaCommandSpec? manualInstall;
  CaInstallOutcome? installResult;

  String search = '';
  String testUrl = '';

  /// null = 还没测过（不渲染结果区）；空列表 = 测过但未命中。两者不是一回事。
  List<MatchedRule>? testResult;

  bool showClearConfirm = false;

  void _notify() => onChanged?.call();

  // ── 派生态（与 React 的 JSX 内联表达式逐条对应）──
  bool get enabled => status?.enabled ?? false;
  bool get caPresent => status?.caPresent ?? false;
  bool get caInstalled => status?.caInstalled ?? false;
  String get fingerprint => status?.caFingerprint ?? '';
  List<WhitelistEntry> get whitelist => status?.whitelist ?? const [];

  /// 前端纯 filter，实时无按钮，大小写不敏感。
  List<WhitelistEntry> get filteredWhitelist {
    final key = search.trim().toLowerCase();
    if (key.isEmpty) return whitelist;
    return whitelist.where((e) => e.hostPattern.toLowerCase().contains(key)).toList();
  }

  bool get canAdd => !busy && newPattern.trim().isNotEmpty;
  bool get canTest => !busy && testUrl.trim().isNotEmpty;
  bool get canClear => !busy && whitelist.isNotEmpty;
  bool get canInstallCa => !busy && caPresent;

  /// 列表空态分两种，文案不同：整份为空 vs 搜索没命中。
  bool get showEmptyList => whitelist.isEmpty;
  bool get showSearchEmpty => whitelist.isNotEmpty && filteredWhitelist.isEmpty;

  Future<void> refresh() async {
    try {
      status = MitmStatus.fromJson(_map(await _invoke('mitm_status')));
    } catch (e) {
      error = '$e';
    } finally {
      loading = false;
      _notify();
    }
  }

  Future<void> toggleEnabled() async {
    final s = status;
    if (s == null) return;
    busy = true;
    error = '';
    _notify();
    try {
      await _invoke(s.enabled ? 'mitm_disable' : 'mitm_enable');
      await refresh();
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  /// 装 CA。失败时给分类文案 + 手动执行的命令兜底。
  ///
  /// 两条兜底不能省（React 里是「阶段 A 兜底」那两段注释）：
  /// ① 分类文案算出来是空串时，退回 `exit=... stderr=... stdout=...` 原始串；
  /// ② reject 且错误对象没有 message 时，退回 `(reject 无 message)`。
  /// 少了它们就会出现一条**没有文字的错误提示**。
  Future<void> installCa(MitmInstallTexts texts) async {
    busy = true;
    error = '';
    manualInstall = null;
    installResult = null;
    _notify();
    try {
      // spec 只为兜底弹窗的文案；真正的执行在后端。
      final spec = CaCommandSpec.fromJson(_map(await _invoke('mitm_install_ca_prepare')));
      final out = CaInstallOutcome.fromJson(_map(await _invoke('mitm_install_ca')));
      final ok = out.code == 0;
      installResult = out;
      await _invoke('mitm_set_ca_installed', {'installed': ok});
      if (!ok) {
        manualInstall = spec;
        final kind = '${await _invoke('mitm_classify_trust_error', {
              'name': spec.name,
              'code': out.code,
              'stderr': out.stderr,
            })}';
        final base = switch (kind) {
          'cancel' => texts.cancel,
          'auth_fail' => texts.authFail,
          'no_agent' => texts.noAgent,
          _ => texts.failed('${out.code}'),
        };
        // cmd_fail 附原始 stderr 辅助诊断；其余分类不堆 stderr（用户无需看）。
        final detail = kind == 'cmd_fail' && out.stderr.isNotEmpty ? '$base\n${out.stderr}' : base;
        error = detail.trim().isNotEmpty
            ? detail
            : 'exit=${out.code} stderr=${out.stderr.isEmpty ? '(empty)' : out.stderr} '
                'stdout=${out.stdout.isEmpty ? '(empty)' : out.stdout}';
      }
      await refresh();
    } catch (e) {
      final s = '$e';
      error = s.isNotEmpty ? s : '(reject 无 message)';
      try {
        manualInstall = CaCommandSpec.fromJson(_map(await _invoke('mitm_install_ca_prepare')));
      } catch (_) {/* ignore secondary error */}
    } finally {
      busy = false;
      _notify();
    }
  }

  void setNewPattern(String v) {
    newPattern = v;
    _notify();
  }

  void setNewRuleType(WhitelistRuleType v) {
    newRuleType = v;
    _notify();
  }

  void setSearch(String v) {
    search = v;
    _notify();
  }

  void setTestUrl(String v) {
    testUrl = v;
    _notify();
  }

  /// suffix 类型 strip 所有前导点（与后端 `matches_rule` 的容错对齐，
  /// 防 `.cn` / `..cn` 这类脏数据进库）。domain / keyword / ipcidr 不动。
  static String normalizePattern(String raw, WhitelistRuleType type) {
    final p = raw.trim();
    if (type != WhitelistRuleType.suffix) return p;
    return p.replaceFirst(RegExp(r'^\.+'), '');
  }

  Future<void> addRule() async {
    var p = newPattern.trim();
    if (p.isEmpty) return;
    p = normalizePattern(p, newRuleType);
    if (p.isEmpty) return; // 全是点，strip 后为空 → 不加废规则
    busy = true;
    error = '';
    _notify();
    try {
      await _invoke('mitm_whitelist_add', {
        'input': {'host_pattern': p, 'rule_type': newRuleType.wire},
      });
      newPattern = '';
      // newRuleType 不重置。
      await refresh();
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  /// 注意：开关与删除**不置 busy**（与 React 一致），所以列表上的多个开关可以连点。
  Future<void> toggleEntry(String hostPattern, bool enabled) async {
    error = '';
    try {
      await _invoke('mitm_whitelist_toggle', {'hostPattern': hostPattern, 'enabled': enabled});
      await refresh();
    } catch (e) {
      error = '$e';
      _notify();
    }
  }

  Future<void> removeRule(String hostPattern) async {
    error = '';
    try {
      await _invoke('mitm_whitelist_remove', {'hostPattern': hostPattern});
      await refresh();
    } catch (e) {
      error = '$e';
      _notify();
    }
  }

  /// 导入默认白名单（37 条，INSERT OR IGNORE，幂等可重复点）。
  /// [doneText] 接 (imported, skipped) 返回成品文案。
  Future<void> importDefaults(String Function(int imported, int skipped) doneText) async {
    busy = true;
    error = '';
    message = '';
    _notify();
    try {
      final r = _map(await _invoke('mitm_whitelist_import_defaults'));
      message = doneText(
        (r['imported'] as num?)?.toInt() ?? 0,
        (r['skipped'] as num?)?.toInt() ?? 0,
      );
      await refresh();
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  Future<void> runUrlTest() async {
    final u = testUrl.trim();
    if (u.isEmpty) return;
    busy = true;
    error = '';
    testResult = null;
    _notify();
    try {
      final hits = await _invoke('mitm_whitelist_test_url', {'url': u});
      testResult = (hits as List? ?? const [])
          .map((e) => MatchedRule.fromJson(Map<String, Object?>.from(e as Map)))
          .toList();
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  void openClearConfirm() {
    showClearConfirm = true;
    _notify();
  }

  void closeClearConfirm() {
    showClearConfirm = false;
    _notify();
  }

  /// **不可撤销**：只能由确认框的确认按钮调。清空后连搜索框与测试结果一起清掉
  /// （列表已空，旧结果没有意义）。
  Future<void> confirmClear(String Function(int n) doneText) async {
    showClearConfirm = false;
    busy = true;
    error = '';
    message = '';
    _notify();
    try {
      final n = await _invoke('mitm_whitelist_clear');
      message = doneText((n as num?)?.toInt() ?? 0);
      search = '';
      testUrl = '';
      testResult = null;
      await refresh();
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
}
