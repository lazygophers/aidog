/// 拖进来的一串路径里挑出要导入的那个（`importexport_logic.dart::pickAidogxPath`）。
///
/// 拖放手势本身在 widget 测试里模拟不了（`desktop_drop` 走的是平台通道，
/// 事件从原生侧来），所以把判定抽成纯函数单独测，落区的存在与点击行为
/// 在 `pages_c_widget_test.dart` 里测。
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:aidog_flutter/src/pages/settings/importexport_logic.dart';

void main() {
  group('pickAidogxPath', () {
    test('什么都没拖到 → null', () {
      expect(pickAidogxPath(const []), isNull);
    });

    test('拖了文件但一个 .aidogx 都没有 → null（调用方报 notAidogx）', () {
      expect(pickAidogxPath(const ['/tmp/a.zip', '/tmp/b.json']), isNull);
    });

    test('拖一堆文件 → 取第一个 .aidogx，不批量导入', () {
      expect(
        pickAidogxPath(const ['/tmp/a.zip', '/tmp/one.aidogx', '/tmp/two.aidogx']),
        '/tmp/one.aidogx',
      );
    });

    test('扩展名大小写不敏感（macOS 文件系统默认不区分）', () {
      expect(pickAidogxPath(const ['/tmp/BACKUP.AIDOGX']), '/tmp/BACKUP.AIDOGX');
    });

    test('文件名里带 aidogx 但结尾不是，不算', () {
      expect(pickAidogxPath(const ['/tmp/aidogx-notes.txt']), isNull);
    });
  });
}
