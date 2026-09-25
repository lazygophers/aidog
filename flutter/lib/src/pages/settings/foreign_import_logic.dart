/// 异源导入 —— cc-switch 与 sub2api，两者在 `settings/importexport` 页里。
/// 对齐 `src/components/settings/CcSwitchImport.tsx` 与 `Sub2ApiImport.tsx`。
///
/// 两条流程同形（所以是一个控制器带两套接线，不是两份拷贝）：
///   探测 / 读文件 → 后端解析出 provider 列表 → 用户勾选 + 定冲突决策
///   → 前端转成 Platform JSON → `*_import` 走 apply 写入
///   → `autoGroup` 开着就建 / 加入对应分组。
///
/// `autoGroup` 默认**开**（两页的 toggle 初值都是 true）。
library;

import '../invoke.dart';
import 'importexport_logic.dart' show ConflictDecision, ConflictDecisionKind;

/// 两个异源的接线差异，只有命令名和分组名。
class ForeignSource {
  const ForeignSource({
    required this.importCmd,
    required this.autoGroupName,
    this.detectCmd,
    this.readCmd,
    this.parseCmd,
    this.readFileCmd,
  });

  /// cc-switch：探测安装位置 → 读 providers。
  static const ccswitch = ForeignSource(
    detectCmd: 'ccswitch_detect',
    readCmd: 'ccswitch_read',
    importCmd: 'ccswitch_import',
    autoGroupName: 'cc-switch',
  );

  /// sub2api：读文件文本（避开前端 fs scope 限制）→ 后端解析。
  static const sub2api = ForeignSource(
    readFileCmd: 'sub2api_read_file',
    parseCmd: 'sub2api_parse',
    importCmd: 'sub2api_import',
    autoGroupName: 'sub2api',
  );

  final String? detectCmd;
  final String? readCmd;
  final String? parseCmd;
  final String? readFileCmd;
  final String importCmd;
  final String autoGroupName;
}

class ForeignImportController {
  ForeignImportController({
    required this.source,
    InvokeFn? invoke,
    this.onChanged,
  }) : _invoke = invoke ?? kernelInvoke;

  final ForeignSource source;
  final InvokeFn _invoke;
  final void Function()? onChanged;

  /// `ccswitch_detect` 的结果（路径是否存在等）。null = 还没探测。
  Map<String, Object?>? detection;

  /// 解析出来的 provider 列表。
  List<Map<String, Object?>> providers = const [];

  /// 勾中的 provider 下标。默认全选。
  Set<int> selected = {};

  Map<String, ConflictDecision> decisions = {};

  /// 导入后建 / 加入对应分组。**默认开**。
  bool autoGroup = true;

  Map<String, Object?>? report;
  bool busy = false;
  String error = '';

  void _notify() => onChanged?.call();

  bool get canImport => !busy && selected.isNotEmpty;

  /// cc-switch：探测安装位置。[overridePath] 为用户手选的路径。
  Future<void> detect({String? overridePath}) async {
    final cmd = source.detectCmd;
    if (cmd == null) return;
    busy = true;
    error = '';
    _notify();
    try {
      detection = _map(await _invoke(cmd, {'overridePath': overridePath}));
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  /// cc-switch：读 providers（仅 claude + codex）。
  Future<void> read({String? path}) async {
    final cmd = source.readCmd;
    if (cmd == null) return;
    busy = true;
    error = '';
    _notify();
    try {
      final r = _map(await _invoke(cmd, {'path': path}));
      _takeProviders(r);
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  /// sub2api：后端读文件文本（避开前端 fs scope 限制）。
  Future<String?> readFile(String path) async {
    final cmd = source.readFileCmd;
    if (cmd == null) return null;
    try {
      return '${await _invoke(cmd, {'path': path})}';
    } catch (e) {
      error = '$e';
      _notify();
      return null;
    }
  }

  /// sub2api：解析粘贴 / 读到的 JSON 文本。
  Future<void> parse(String jsonText) async {
    final cmd = source.parseCmd;
    if (cmd == null) return;
    busy = true;
    error = '';
    _notify();
    try {
      _takeProviders(_map(await _invoke(cmd, {'jsonText': jsonText})));
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  void _takeProviders(Map<String, Object?> r) {
    final list = r['providers'] ?? r['accounts'] ?? r['items'];
    providers = (list is List ? list : const [])
        .whereType<Map>()
        .map(Map<String, Object?>.from)
        .toList();
    // 默认全选。
    selected = {for (var i = 0; i < providers.length; i++) i};
    decisions = {};
    report = null;
  }

  void toggleSelected(int index, bool on) {
    final next = {...selected};
    if (on) {
      next.add(index);
    } else {
      next.remove(index);
    }
    selected = next;
    _notify();
  }

  void setAutoGroup(bool v) {
    autoGroup = v;
    _notify();
  }

  void decide(String key, ConflictDecisionKind kind) {
    decisions = {
      ...decisions,
      key: ConflictDecision(
        kind,
        newKey: kind == ConflictDecisionKind.keepBoth ? '$key-imported' : '',
      ),
    };
    _notify();
  }

  void setRenameKey(String key, String newKey) {
    final cur = decisions[key];
    if (cur == null) return;
    decisions = {...decisions, key: cur.withNewKey(newKey)};
    _notify();
  }

  /// 把勾中的 provider 转成 Platform JSON 交给后端 apply。
  /// [toPlatformPayload] 由 UI 层提供（形状与页面上的表单绑定）。
  Future<void> runImport(
    List<Map<String, Object?>> Function(List<Map<String, Object?>> chosen)
    toPlatformPayload,
  ) async {
    if (!canImport) return;
    busy = true;
    error = '';
    _notify();
    try {
      final chosen = [
        for (var i = 0; i < providers.length; i++)
          if (selected.contains(i)) providers[i],
      ];
      report = _map(
        await _invoke(source.importCmd, {
          'platformPayload': toPlatformPayload(chosen),
          // 后端收的是 `Vec<ConflictDecision>`，**三个字段都必填**
          //（`gateway/import_export/mod.rs:190-195`）：原先漏了 `scope`，
          // 且 `decision` 发的是裸字符串，两处都会让反序列化失败。
          // 异源导入进来的都是平台，scope 固定 `platform`（`mod.rs:31`，单数）。
          'decisions': [
            for (final e in decisions.entries)
              {'scope': 'platform', 'key': e.key, 'decision': e.value.toWire()},
          ],
          'autoGroup': autoGroup,
        }),
      );
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

/// 导入后把平台并进分组用的两条命令，以及导入页刷新平台列表用的一条。
/// 单独放在这里是因为 cc-switch 页在导入完成后会直接用它们收尾
/// （`CcSwitchImport.tsx` 里 `platform_ensure_auto_group` / `platform_update`）。
class ForeignImportFollowUp {
  ForeignImportFollowUp({InvokeFn? invoke}) : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;

  /// 建 / 取自动分组，返回分组 id。
  Future<Object?> ensureAutoGroup(String name) =>
      _invoke('platform_ensure_auto_group', {'name': name});

  /// 导入后回写平台字段（如并进分组）。
  Future<Object?> updatePlatform(Map<String, Object?> input) =>
      _invoke('platform_update', {'input': input});

  /// 导入页刷新用。
  Future<Object?> listPlatforms() => _invoke('platform_list');

  /// 分组明细列表（导入目标分组选择器）。
  Future<Object?> listGroupDetails() => _invoke('group_detail_list');
}
