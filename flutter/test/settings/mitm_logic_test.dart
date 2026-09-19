/// `MitmController` —— 对照 `src/components/settings/MitmConfig.tsx`。
///
/// 这一页的「每控件细节」最密，用例按票里点名的那几类编：
/// 校验规则 / 禁用条件 / 破坏性操作的确认框 / 空态 / 错误文案。
library;

import 'package:aidog_flutter/src/pages/settings/mitm_logic.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fake_invoke.dart';

const _texts = MitmInstallTexts(
  cancel: '已取消安装（未输入密码或点了取消）',
  authFail: '密码错误或鉴权被拒，请重试',
  noAgent: 'Linux 缺少 polkit 鉴权 agent，请用下方命令手动装',
  failed: _failed,
);
String _failed(String code) => '装信任库失败（exit=$code）';

Map<String, Object?> statusJson({
  bool enabled = true,
  bool caPresent = true,
  bool caInstalled = false,
  List<Map<String, Object?>> whitelist = const [],
}) =>
    {
      'enabled': enabled,
      'ca_present': caPresent,
      'ca_installed': caInstalled,
      'ca_fingerprint': 'AA:BB',
      'whitelist': whitelist,
    };

Map<String, Object?> entry(String p, {String source = 'user', String type = 'suffix', bool on = true}) =>
    {'host_pattern': p, 'enabled': on, 'source': source, 'rule_type': type};

void main() {
  group('加载', () {
    test('读到状态后 loading 落下', () async {
      final fake = FakeInvoke({'mitm_status': statusJson()});
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      expect(c.loading, isFalse);
      expect(c.enabled, isTrue);
      expect(c.fingerprint, 'AA:BB');
    });

    test('读失败也要落下 loading，并显示错误（否则页面永远卡在「加载中」）', () async {
      final fake = FakeInvoke();
      fake.errors['mitm_status'] = StateError('db gone');
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      expect(c.loading, isFalse);
      expect(c.error, contains('db gone'));
    });

    test('status 为 null 时派生态全走默认，不抛', () {
      final c = MitmController(invoke: FakeInvoke().fn);
      expect(c.enabled, isFalse);
      expect(c.caPresent, isFalse);
      expect(c.caInstalled, isFalse);
      expect(c.fingerprint, '');
      expect(c.whitelist, isEmpty);
    });

    test('条目缺 rule_type 时回落 suffix（与 React 的 ?? "suffix" 一致）', () {
      final e = WhitelistEntry.fromJson({'host_pattern': 'a.com', 'enabled': true, 'source': 'user'});
      expect(e.ruleType, WhitelistRuleType.suffix);
    });
  });

  group('主开关', () {
    test('开着 → 点一下发 disable；关着 → 发 enable', () async {
      final fake = FakeInvoke({'mitm_status': statusJson(enabled: true)});
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      await c.toggleEnabled();
      expect(fake.commands, contains('mitm_disable'));

      fake.responses['mitm_status'] = statusJson(enabled: false);
      await c.refresh();
      await c.toggleEnabled();
      expect(fake.commands, contains('mitm_enable'));
    });

    test('status 还没读到时点开关是空操作', () async {
      final fake = FakeInvoke();
      final c = MitmController(invoke: fake.fn);
      await c.toggleEnabled();
      expect(fake.calls, isEmpty);
    });

    test('切换失败 → 错误上屏，busy 复位', () async {
      final fake = FakeInvoke({'mitm_status': statusJson(enabled: false)});
      fake.errors['mitm_enable'] = StateError('no perm');
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      await c.toggleEnabled();
      expect(c.error, contains('no perm'));
      expect(c.busy, isFalse);
    });
  });

  group('添加规则的校验', () {
    test('suffix 类型 strip 所有前导点', () {
      expect(MitmController.normalizePattern('.cn', WhitelistRuleType.suffix), 'cn');
      expect(MitmController.normalizePattern('..cn', WhitelistRuleType.suffix), 'cn');
      expect(MitmController.normalizePattern('  .a.com  ', WhitelistRuleType.suffix), 'a.com');
    });

    test('domain / keyword / ipcidr 不动前导点', () {
      for (final t in [
        WhitelistRuleType.domain,
        WhitelistRuleType.keyword,
        WhitelistRuleType.ipcidr,
      ]) {
        expect(MitmController.normalizePattern('.cn', t), '.cn', reason: '$t');
      }
    });

    test('纯空白不发命令', () async {
      final fake = FakeInvoke({'mitm_status': statusJson()});
      final c = MitmController(invoke: fake.fn)..setNewPattern('   ');
      await c.addRule();
      expect(fake.callsTo('mitm_whitelist_add'), isEmpty);
    });

    test('全是点（strip 后为空）也不加废规则', () async {
      final fake = FakeInvoke({'mitm_status': statusJson()});
      final c = MitmController(invoke: fake.fn)
        ..setNewRuleType(WhitelistRuleType.suffix)
        ..setNewPattern('...');
      await c.addRule();
      expect(fake.callsTo('mitm_whitelist_add'), isEmpty);
    });

    test('成功后清空输入框，但**保留**匹配方式（方便连加同类型）', () async {
      final fake = FakeInvoke({'mitm_status': statusJson()});
      final c = MitmController(invoke: fake.fn)
        ..setNewRuleType(WhitelistRuleType.keyword)
        ..setNewPattern('anthropic');
      await c.addRule();

      expect(c.newPattern, '');
      expect(c.newRuleType, WhitelistRuleType.keyword);
      expect(fake.lastCallTo('mitm_whitelist_add')?.args, {
        'input': {'host_pattern': 'anthropic', 'rule_type': 'keyword'},
      });
    });

    test('添加失败时输入框内容保住，让用户能改了重试', () async {
      final fake = FakeInvoke({'mitm_status': statusJson()});
      fake.errors['mitm_whitelist_add'] = StateError('bad pattern');
      final c = MitmController(invoke: fake.fn)..setNewPattern('***');
      await c.addRule();
      expect(c.newPattern, '***');
      expect(c.error, contains('bad pattern'));
    });

    test('禁用条件：busy 或输入去空白后为空', () {
      final c = MitmController(invoke: FakeInvoke().fn);
      expect(c.canAdd, isFalse);
      c.setNewPattern('  ');
      expect(c.canAdd, isFalse);
      c.setNewPattern('a.com');
      expect(c.canAdd, isTrue);
      c.busy = true;
      expect(c.canAdd, isFalse);
    });
  });

  group('列表操作', () {
    test('开关与删除按 host_pattern 走 camelCase 入参', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(whitelist: [entry('a.com')]),
      });
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      await c.toggleEntry('a.com', false);
      expect(fake.lastCallTo('mitm_whitelist_toggle')?.args, {
        'hostPattern': 'a.com',
        'enabled': false,
      });
      await c.removeRule('a.com');
      expect(fake.lastCallTo('mitm_whitelist_remove')?.args, {'hostPattern': 'a.com'});
    });

    test('开关与删除不置 busy（列表上多个开关可以连点）', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(whitelist: [entry('a.com')]),
      });
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      await c.toggleEntry('a.com', false);
      expect(c.busy, isFalse);
    });
  });

  group('搜索过滤与两种空态', () {
    test('大小写不敏感的子串匹配', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(whitelist: [entry('API.Anthropic.com'), entry('openai.com')]),
      });
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      c.setSearch('ANTHROPIC');
      expect(c.filteredWhitelist.map((e) => e.hostPattern).toList(), ['API.Anthropic.com']);
    });

    test('空搜索返回全部', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(whitelist: [entry('a.com'), entry('b.com')]),
      });
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      c.setSearch('   ');
      expect(c.filteredWhitelist, hasLength(2));
    });

    test('「列表为空」与「搜索无命中」是两种空态，文案不同', () async {
      final fake = FakeInvoke({'mitm_status': statusJson(whitelist: const [])});
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      expect(c.showEmptyList, isTrue);
      expect(c.showSearchEmpty, isFalse);

      fake.responses['mitm_status'] = statusJson(whitelist: [entry('a.com')]);
      await c.refresh();
      c.setSearch('zzz');
      expect(c.showEmptyList, isFalse);
      expect(c.showSearchEmpty, isTrue);
    });
  });

  group('URL 命中测试', () {
    test('未测过是 null，测过未命中是空列表 —— 两者不同', () async {
      final fake = FakeInvoke({'mitm_status': statusJson(), 'mitm_whitelist_test_url': []});
      final c = MitmController(invoke: fake.fn);
      expect(c.testResult, isNull);
      c.setTestUrl('https://x.com');
      await c.runUrlTest();
      expect(c.testResult, isNotNull);
      expect(c.testResult, isEmpty);
    });

    test('命中结果按 host_pattern + rule_type 解析', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(),
        'mitm_whitelist_test_url': [
          {'host_pattern': 'anthropic.com', 'rule_type': 'suffix'},
        ],
      });
      final c = MitmController(invoke: fake.fn)..setTestUrl('https://api.anthropic.com/v1/messages');
      await c.runUrlTest();
      expect(c.testResult!.single.hostPattern, 'anthropic.com');
      expect(c.testResult!.single.ruleType, WhitelistRuleType.suffix);
      expect(fake.lastCallTo('mitm_whitelist_test_url')?.args, {
        'url': 'https://api.anthropic.com/v1/messages',
      });
    });

    test('空输入不发命令；禁用条件同 canAdd', () async {
      final fake = FakeInvoke({'mitm_status': statusJson()});
      final c = MitmController(invoke: fake.fn)..setTestUrl('  ');
      expect(c.canTest, isFalse);
      await c.runUrlTest();
      expect(fake.callsTo('mitm_whitelist_test_url'), isEmpty);
    });
  });

  group('导入默认白名单', () {
    test('提示里带 imported / skipped 两个数', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(),
        'mitm_whitelist_import_defaults': {'imported': 30, 'skipped': 7},
      });
      final c = MitmController(invoke: fake.fn);
      await c.importDefaults((i, s) => '已导入 $i 条默认规则（$s 条已存在跳过）');
      expect(c.message, '已导入 30 条默认规则（7 条已存在跳过）');
    });
  });

  group('清空（破坏性，必须先确认）', () {
    test('禁用条件：busy 或白名单为空', () async {
      final fake = FakeInvoke({'mitm_status': statusJson(whitelist: const [])});
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      expect(c.canClear, isFalse, reason: '空列表没什么可清');

      fake.responses['mitm_status'] = statusJson(whitelist: [entry('a.com')]);
      await c.refresh();
      expect(c.canClear, isTrue);
      c.busy = true;
      expect(c.canClear, isFalse);
    });

    test('点「清空」只开确认框，**不发命令**', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(whitelist: [entry('a.com')]),
      });
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      c.openClearConfirm();
      expect(c.showClearConfirm, isTrue);
      expect(fake.callsTo('mitm_whitelist_clear'), isEmpty);
    });

    test('取消确认框 → 一条都不删', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(whitelist: [entry('a.com')]),
      });
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      c.openClearConfirm();
      c.closeClearConfirm();
      expect(c.showClearConfirm, isFalse);
      expect(fake.callsTo('mitm_whitelist_clear'), isEmpty);
    });

    test('确认后才清，并把搜索框与测试结果一起清掉', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(whitelist: [entry('a.com')]),
        'mitm_whitelist_clear': 12,
      });
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      c.setSearch('a');
      c.setTestUrl('https://a.com');
      c.testResult = const [];
      c.openClearConfirm();

      await c.confirmClear((n) => '已清空 $n 条白名单规则');
      expect(c.message, '已清空 12 条白名单规则');
      expect(c.showClearConfirm, isFalse);
      expect(c.search, '');
      expect(c.testUrl, '');
      expect(c.testResult, isNull, reason: '列表已空，旧结果没有意义');
    });
  });

  group('安装 CA', () {
    test('禁用条件：busy 或 CA 还没生成', () async {
      final fake = FakeInvoke({'mitm_status': statusJson(caPresent: false)});
      final c = MitmController(invoke: fake.fn);
      await c.refresh();
      expect(c.canInstallCa, isFalse);

      fake.responses['mitm_status'] = statusJson(caPresent: true);
      await c.refresh();
      expect(c.canInstallCa, isTrue);
    });

    test('exit=0 → 回写已安装、不弹手动兜底、无错误', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(),
        'mitm_install_ca_prepare': {'name': 'mac', 'ca_pem_path': '/tmp/ca.pem', 'manual_display': 'sudo x'},
        'mitm_install_ca': {'code': 0, 'stdout': '', 'stderr': '', 'program': 'osascript'},
      });
      final c = MitmController(invoke: fake.fn);
      await c.installCa(_texts);

      expect(fake.lastCallTo('mitm_set_ca_installed')?.args, {'installed': true});
      expect(c.manualInstall, isNull);
      expect(c.error, '');
    });

    test('用户取消 → cancel 文案 + 手动兜底命令', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(),
        'mitm_install_ca_prepare': {'name': 'mac', 'ca_pem_path': '/tmp/ca.pem', 'manual_display': 'sudo x'},
        'mitm_install_ca': {'code': 1, 'stdout': '', 'stderr': 'User canceled', 'program': 'osascript'},
        'mitm_classify_trust_error': 'cancel',
      });
      final c = MitmController(invoke: fake.fn);
      await c.installCa(_texts);

      expect(fake.lastCallTo('mitm_set_ca_installed')?.args, {'installed': false});
      expect(c.error, '已取消安装（未输入密码或点了取消）');
      expect(c.manualInstall?.manualDisplay, 'sudo x');
      expect(c.installResult?.code, 1);
    });

    test('分类命令拿到 name / code / stderr 三个入参', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(),
        'mitm_install_ca_prepare': {'name': 'linux', 'ca_pem_path': '/p', 'manual_display': 'd'},
        'mitm_install_ca': {'code': 127, 'stdout': '', 'stderr': 'no agent', 'program': 'pkexec'},
        'mitm_classify_trust_error': 'no_agent',
      });
      final c = MitmController(invoke: fake.fn);
      await c.installCa(_texts);
      expect(fake.lastCallTo('mitm_classify_trust_error')?.args, {
        'name': 'linux',
        'code': 127,
        'stderr': 'no agent',
      });
      expect(c.error, 'Linux 缺少 polkit 鉴权 agent，请用下方命令手动装');
    });

    test('cmd_fail 才把 stderr 附在文案后面', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(),
        'mitm_install_ca_prepare': {'name': 'mac', 'ca_pem_path': '/p', 'manual_display': 'd'},
        'mitm_install_ca': {'code': 3, 'stdout': '', 'stderr': 'boom detail', 'program': 'p'},
        'mitm_classify_trust_error': 'cmd_fail',
      });
      final c = MitmController(invoke: fake.fn);
      await c.installCa(_texts);
      expect(c.error, '装信任库失败（exit=3）\nboom detail');
    });

    test('auth_fail 不堆 stderr', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(),
        'mitm_install_ca_prepare': {'name': 'mac', 'ca_pem_path': '/p', 'manual_display': 'd'},
        'mitm_install_ca': {'code': 1, 'stdout': '', 'stderr': 'noise', 'program': 'p'},
        'mitm_classify_trust_error': 'auth_fail',
      });
      final c = MitmController(invoke: fake.fn);
      await c.installCa(_texts);
      expect(c.error, '密码错误或鉴权被拒，请重试');
    });

    test('code 为 null（被信号杀死）也要出可读文案', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(),
        'mitm_install_ca_prepare': {'name': 'mac', 'ca_pem_path': '/p', 'manual_display': 'd'},
        'mitm_install_ca': {'code': null, 'stdout': '', 'stderr': '', 'program': 'p'},
        'mitm_classify_trust_error': 'unknown_kind',
      });
      final c = MitmController(invoke: fake.fn);
      await c.installCa(_texts);
      expect(c.error, '装信任库失败（exit=null）');
      expect(fake.lastCallTo('mitm_classify_trust_error')?.args!['code'], isNull);
    });

    test('分类文案算出来是空串时退回原始 exit/stderr/stdout（绝不出无文字的提示）', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(),
        'mitm_install_ca_prepare': {'name': 'mac', 'ca_pem_path': '/p', 'manual_display': 'd'},
        'mitm_install_ca': {'code': 9, 'stdout': '', 'stderr': '', 'program': 'p'},
        'mitm_classify_trust_error': 'cancel',
      });
      final c = MitmController(invoke: fake.fn);
      await c.installCa(const MitmInstallTexts(
        cancel: '   ', // i18n 缺键 → 空白
        authFail: '',
        noAgent: '',
        failed: _failed,
      ));
      expect(c.error, 'exit=9 stderr=(empty) stdout=(empty)');
    });

    test('命令整个 reject → 错误上屏并仍然取出手动兜底命令', () async {
      final fake = FakeInvoke({
        'mitm_status': statusJson(),
        'mitm_install_ca_prepare': {'name': 'mac', 'ca_pem_path': '/p', 'manual_display': 'sudo fallback'},
      });
      fake.errors['mitm_install_ca'] = StateError('spawn failed');
      final c = MitmController(invoke: fake.fn);
      await c.installCa(_texts);

      expect(c.error, contains('spawn failed'));
      expect(c.manualInstall?.manualDisplay, 'sudo fallback');
      expect(c.busy, isFalse);
    });

    test('reject 后连 prepare 也失败 → 不再抛，busy 仍复位', () async {
      final fake = FakeInvoke({'mitm_status': statusJson()});
      fake.errors['mitm_install_ca_prepare'] = StateError('no ca');
      final c = MitmController(invoke: fake.fn);
      await c.installCa(_texts);
      expect(c.busy, isFalse);
      expect(c.manualInstall, isNull);
      expect(c.error, contains('no ca'));
    });
  });
}
