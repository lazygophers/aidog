/// 平台「新增 / 编辑」表单界面，对齐 `src/pages/platforms/PlatformEditForm.tsx`
/// 及其 11 个分区（`formSections.tsx` / `formSectionsEndpoints.tsx` /
/// `ModelsMatrixSection.tsx` / `MultiKeyPreview.tsx` / `WindowsEditModal.tsx` /
/// `MockConfigEditor.tsx`）。
///
/// 状态全在 [PlatformFormController]（`platform_form_logic.dart`），本文件只有三样
/// 纯 UI 临时态：时段档列头的编辑目标、两个「覆盖/导入」确认卡的开合。
///
/// 与 React 的一处**有意形态差异**（已见于 README 的差异表）：那边的确认弹窗走
/// Portal，这里一律画成页面内的格子 —— widget 测试用 `find.byType` 就能断言，
/// 不必去 dialog 的 route 里捞。
library;

import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../../utils/formatters.dart';
import '../../utils/hex_color.dart';
import '../shell/theme.dart';
import '../utils/pinyin.dart';
import '../shell/tiles.dart';
import 'platform_card_bits.dart' show MiniBadge;
import 'platform_defaults.dart';
import 'platform_extra.dart';
import 'platform_form_bits.dart';
import 'platform_form_logic.dart';
import 'smart_paste_modal.dart';
import 'time_window.dart';
import 'ui_bits.dart';

/// 毫秒时间戳 → `YYYY-MM-DDTHH:MM`（本地时区，无秒）。
/// 与 `formSections.tsx:33::toDatetimeLocal` 同格式 —— 那边是 `datetime-local`
/// input 的值，这里是文本框的值，两侧用户看到的串一字不差。
String toDatetimeLocal(int ms) {
  final d = DateTime.fromMillisecondsSinceEpoch(ms);
  return '${d.year}-${pad(d.month)}-${pad(d.day)}'
      'T${pad(d.hour)}:${pad(d.minute)}';
}

/// `YYYY-MM-DDTHH:MM` → 毫秒时间戳；空串 / 非法 / <=0 → null。
int? datetimeLocalToMs(String v) {
  if (v.trim().isEmpty) return null;
  final ms = DateTime.tryParse(v.trim())?.millisecondsSinceEpoch;
  if (ms == null || ms <= 0) return null;
  return ms;
}

/// Unix 秒 → `datetime-local` 串（`formSections.tsx:40::secToLocalInput`）。
String secToLocalInput(int? sec) =>
    (sec == null || sec <= 0) ? '' : toDatetimeLocal(sec * 1000);

/// `datetime-local` 串 → Unix 秒（`formSections.tsx:46::localInputToSec`）。
int? localInputToSec(String v) {
  final ms = datetimeLocalToMs(v);
  return ms == null ? null : ms ~/ 1000;
}

/// number input 值裁到 `[min,max]`；非法输入回落 min（`formSections.tsx:601::clampInt`）。
int clampInt(String raw, int min, int max) {
  final v = num.tryParse(raw.trim())?.round();
  if (v == null) return min;
  return v < min ? min : (v > max ? max : v);
}

/// 窗口预览可读时段（`formSections.tsx:612::formatWindowPreview`）：
/// 半开区间 `[start, end)` → 显示 `end-1:59:59`。
String formatWindowPreview(TimeWindow w, TzMode tzMode, I18nController t) {
  final startDisplay = w.timezone != null
      ? (hour: w.startHour, minute: w.startMinute ?? 0)
      : utcToDisplay(w.startHour, w.startMinute ?? 0, tzMode);

  final endTotalMin = w.endHour * 60 + (w.endMinute ?? 0);
  final endMinusOneMin = endTotalMin - 1;
  final endHourRaw = endMinusOneMin < 0 ? 23 : endMinusOneMin ~/ 60;
  final endMinRaw = endMinusOneMin < 0 ? 59 : endMinusOneMin % 60;
  final endDisplay = w.timezone != null
      ? (hour: endHourRaw, minute: endMinRaw)
      : utcToDisplay(endHourRaw, endMinRaw, tzMode);

  final startStr = '${pad(startDisplay.hour)}:${pad(startDisplay.minute)}:00';
  final endStr = '${pad(endDisplay.hour)}:${pad(endDisplay.minute)}:59';
  final tzLabel =
      w.timezone ??
      (tzMode == TzMode.local
          ? t.t('platform.timezone_local')
          : t.t('platform.timezone_utc'));
  final isNextDay = w.endHour < w.startHour;
  final nextDayLabel = isNextDay ? '（${t.t('platform.peak_next_day')}）' : '';
  return '$startStr - $endStr（$tzLabel）$nextDayLabel';
}

/// 单窗口的紧凑描述（`ModelsMatrixSection.tsx:40::describeWindow`）。
String describeWindow(TimeWindow w, TzMode tzMode, I18nController t) {
  final isFullDay =
      w.startHour == 0 &&
      w.endHour == 24 &&
      (w.startMinute ?? 0) == 0 &&
      (w.endMinute ?? 0) == 0 &&
      w.daysOfWeek == null &&
      w.daysOfMonth == null;
  if (isFullDay) return t.t('platform.window_all_day');
  final start = utcToDisplay(w.startHour, w.startMinute ?? 0, tzMode);
  final end = utcToDisplay(w.endHour, w.endMinute ?? 0, tzMode);
  final hourOnly = start.minute == 0 && end.minute == 0;
  final timePart = hourOnly
      ? '${start.hour}-${end.hour}'
      : '${pad(start.hour)}:${pad(start.minute)}-'
            '${pad(end.hour)}:${pad(end.minute)}';
  var dayPart = '';
  if (w.daysOfWeek != null && w.daysOfWeek!.isNotEmpty) {
    dayPart =
        '(${w.daysOfWeek!.map((d) => t.t('platform.weekday_short.$d')).join(',')})';
  } else if (w.daysOfMonth != null && w.daysOfMonth!.isNotEmpty) {
    dayPart = '(每月${w.daysOfMonth!.join(',')}日)';
  }
  final tzLabel = tzMode == TzMode.local
      ? t.t('platform.timezone_local')
      : t.t('platform.timezone_utc');
  return '$timePart$dayPart（$tzLabel）';
}

/// 时段档列头的描述（`ModelsMatrixSection.tsx:67::describeWindows`）：
/// 取首窗口描述，多窗口加 `+N`，空 → 「永不命中」。
String describeWindows(
  List<TimeWindow> windows,
  TzMode tzMode,
  I18nController t,
) {
  if (windows.isEmpty) return t.t('platform.window_never');
  final first = describeWindow(windows.first, tzMode, t);
  if (windows.length == 1) return first;
  return '$first+${windows.length - 1}';
}

/// `MultiKeyPreview.tsx:25::maskTail` —— 前缀 `••••` + 尾 4 位。
String maskTail(String k) =>
    k.length <= 4 ? k : '••••${k.substring(k.length - 4)}';

// ═══════════════════════════════════════════════════════════════════════

class PlatformEditForm extends StatefulWidget {
  const PlatformEditForm({
    super.key,
    required this.controller,
    this.copyText = native.writeText,
  });

  final PlatformFormController controller;

  /// 复制到剪贴板（编辑态 Token 那颗按钮）。抽成参数是为了测试能注入假实现。
  final Future<void> Function(String text) copyText;

  @override
  State<PlatformEditForm> createState() => _PlatformEditFormState();
}

class _PlatformEditFormState extends State<PlatformEditForm> {
  /// 时段档列头点开的编辑目标（React 的 `editingIdx` + `WindowsEditModal`）。
  int? _editingRuleIdx;

  /// 「从高峰时段导入」的确认卡开合（React 的 `importModalOpen`）。
  bool _importPeakOpen = false;

  /// 「导入默认高峰配置」的确认卡开合（React 的 `modalOpen`）。
  bool _overwritePeakOpen = false;

  /// 智能识别弹窗开合（React 的 `showPaste`，`PlatformEditForm.tsx:129`）。
  bool _showPaste = false;

  PlatformFormController get c => widget.controller;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    // 整张表单淡入（`PlatformEditForm.tsx:138` 的 `animate-fade-in`）：
    // 从列表切到表单是整页替换，没有过渡会「啪」地换掉一屏。
    return Reveal(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          _header(t),
          // 页容器 gap 20（`PlatformEditForm.tsx:96`）。
          const SizedBox(height: 20),
          _basicSection(t),
          if (c.isMock) _mockSection(t),
          if (!c.isMock && !c.isPassthrough) _quotaScriptSection(t),
          if (c.protocol == 'devin') _devinSection(t),
          if (c.isPassthrough) _passthroughSection(t),
          if (!c.isMock && !c.isPassthrough) ...[
            _endpointsSection(t),
            _authSection(t),
            if (c.isBatch && !c.keyOptional) _multiKeyPreview(t),
            _modelsMatrixSection(t),
          ],
          if (!c.isPassthrough) _manualBudgetsSection(t),
          if (c.editing != null && !c.isPassthrough) _breakerSection(t),
          if (c.editing != null && !c.isPassthrough) _peakSection(t),
          if (!c.isPassthrough) _groupAssignSection(t),
          _expirySection(t),
          if (c.saveError.isNotEmpty) ToastBar(text: c.saveError, ok: false),
          // 智能识别弹窗（票 20）。浮层由 AidogModal 画，所以挂在树里哪一层都行。
          if (_showPaste)
            SmartPasteModal(
              presets: c.list.protocolMeta.pastePresets,
              protocolLabels: c.protocolLabelMap,
              invoke: c.invoke,
              onApply: c.applyPaste,
              onClose: () => setState(() => _showPaste = false),
            ),
        ],
      ),
    );
  }

  // ── 页头（返回 / 标题 / 取消 / 保存）────────────────────────────

  Widget _header(I18nController t) {
    final theme = AidogTheme.of(context);
    final editing = c.editing;
    final saveLabel = editing != null
        ? t.t('action.save')
        : (c.isBatch
              ? t.t('platform.batch.createN', {'n': c.batchPreviewKeys!.length})
              : t.t('action.create'));
    return Row(
      children: [
        // 返回按钮 `padding: 4px 8px`、14（`PlatformEditForm.tsx:99`）。
        SmallButton(
          label: '← ${t.t('action.back')}',
          fontSize: 14,
          padding: (8, 4),
          onTap: c.resetForm,
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                editing != null ? editing.name : t.t('platform.add'),
                // `.section-title` = 18 w700 ls-0.02em（`globals.css:702-707`）。
                style: AidogType.title.copyWith(
                  fontSize: 18,
                  fontWeight: FontWeight.w700,
                  letterSpacing: -0.36,
                  color: theme.c.fg,
                ),
              ),
              if (editing != null)
                Text(
                  '${c.protocolLabelMap[editing.platformType] ?? editing.platformType}'
                  ' · ${_primaryBaseUrlOf(editing.platformType)}',
                  // `.section-desc` = 13 secondary（`globals.css:709-712`）。
                  style: AidogType.caption.copyWith(
                    fontSize: 13,
                    color: theme.c.fg2,
                  ),
                ),
            ],
          ),
        ),
        // 「智能识别」只在**新建**态出现（`PlatformEditForm.tsx:109`）：
        // 编辑已有平台时整段灌入会把用户改过的字段冲掉。
        if (editing == null) ...[
          // 这三颗都是 shadcn `<Button>` 默认 / outline 档：
          // h-9(36) px-4(16) py-2(8) text-sm(14)（`ui/button.tsx:27`）。
          SmallButton(
            label: t.t('platform.paste.title'),
            fontSize: 14,
            padding: (16, 8),
            onTap: () => setState(() => _showPaste = true),
          ),
          const SizedBox(width: 8),
        ],
        SmallButton(
          label: t.t('action.cancel'),
          fontSize: 14,
          padding: (16, 8),
          onTap: c.resetForm,
        ),
        const SizedBox(width: 8),
        SmallButton(
          label: saveLabel,
          fontSize: 14,
          padding: (16, 8),
          filled: true,
          onTap: (c.canSave && !c.saving) ? _save : null,
        ),
      ],
    );
  }

  String _primaryBaseUrlOf(String protocol) {
    for (final ep in c.endpoints) {
      if (ep.protocol == protocol) return ep.baseUrl;
    }
    return c.endpoints.isNotEmpty
        ? c.endpoints.first.baseUrl
        : (c.editing?.baseUrl ?? '');
  }

  Future<void> _save() async {
    final t = AidogI18n.of(context);
    await c.handleSave(
      saveFailText: t.t('platform.saveFail'),
      progressText: (done, total) =>
          t.t('platform.batch.progress', {'done': done, 'total': total}),
      allOkText: (n) => t.t('platform.batch.allOk', {'n': n}),
      summaryText: (ok, fail) =>
          t.t('platform.batch.summary', {'ok': ok, 'fail': fail}),
      noBaseUrlText: t.t('platform.batch.noBaseUrl'),
    );
  }

  // ── F1 基础信息 ────────────────────────────────────────────────

  Widget _basicSection(I18nController t) {
    final theme = AidogTheme.of(context);
    return FormSection(
      title: t.t('platform.sectionBasic'),
      children: [
        PlatformField(
          value: c.name,
          hint: t.t('platform.name'),
          onChanged: c.setName,
        ),
        const SizedBox(height: AidogSpace.ssm),
        if (c.editing != null)
          // 创建后协议锁定：只读展示，不给选择器。
          Row(
            children: [
              Builder(
                builder: (context) {
                  // 锁定徽标：`padding: 2px 8px`、11 w700，底 = 协议色 12%、
                  // 字 = 协议色（`PlatformEditForm.tsx:150-155`）。
                  final brand =
                      parseHexColor(c.defaults.protocolColorMap()[c.protocol]) ??
                      theme.c.accentText;
                  return Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 8,
                      vertical: 2,
                    ),
                    decoration: BoxDecoration(
                      color: brand.withValues(alpha: 0.12),
                      borderRadius: BorderRadius.circular(AidogRadius.sm),
                    ),
                    child: Text(
                      c.protocolLabelMap[c.protocol] ?? c.protocol,
                      style: AidogType.micro.copyWith(
                        fontSize: 11,
                        letterSpacing: 0,
                        fontWeight: FontWeight.w700,
                        color: brand,
                      ),
                    ),
                  );
                },
              ),
              const SizedBox(width: AidogSpace.ssm),
              Expanded(
                child: Text(
                  t.t('platform.protocolLocked'),
                  style: AidogType.caption.copyWith(color: theme.c.fg3),
                ),
              ),
            ],
          )
        else
          ProtocolPicker(
            value: c.protocol,
            codingPlan: c.codingPlan,
            searchHint: t.t('platform.searchPlaceholder'),
            // React 这一行是写死的英文字面量（`SearchableProtocolSelect.tsx:216`），
            // 不走 i18n —— 这里照抄，保持零差。
            noMatchText: 'No match',
            // logo 缺图时的品牌色圆圈（`ProtocolLogo.tsx:26` 的 color map）。
            colors: {
              for (final e in c.defaults.protocolColorMap().entries)
                if (parseHexColor(e.value) != null)
                  e.key: parseHexColor(e.value)!,
            },
            options: [
              for (final p in c.defaults.protocolOptions(t.locale))
                (
                  value: p.value,
                  label: p.label,
                  codingPlan: p.codingPlan,
                  terms: p.searchTerms,
                ),
            ],
            onChanged: (proto, cp) =>
                c.handleProtocolChange(proto, newCodingPlan: cp),
          ),
      ],
    );
  }

  // ── F2 Mock 特例配置 ───────────────────────────────────────────

  Widget _mockSection(I18nController t) {
    final m = c.mockConfig;
    // 数字型（`formSections.tsx` 那几处都是 `type="number" min=0`）：过滤 +
    // 步进走 [NumberField]。原先是普通文本框 + `tryParse ?? 0`，敲错一个字符
    // 整格静默变 0。
    Widget num1(String label, int value, ValueChanged<int> onChanged) =>
        Padding(
          padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
          child: NumberField(
            label: label,
            value: '$value',
            min: 0,
            onChanged: (v) {
              final n = int.tryParse(v.trim());
              if (n != null) onChanged(n);
            },
          ),
        );
    // 可选数值：留空 = null（透传给 Rust Option，禁塞 0 假装默认）。
    Widget numOpt(
      String label,
      num? value,
      String hint,
      void Function(num? v) onChanged, {
      num? min,
      num? max,
    }) => Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          PlatformField(
            label: label,
            value: value == null ? '' : '$value',
            onChanged: (raw) {
              if (raw.trim().isEmpty) {
                onChanged(null);
                return;
              }
              var n = num.tryParse(raw.trim());
              if (n == null) return;
              if (min != null && n < min) n = min;
              if (max != null && n > max) n = max;
              onChanged(n);
            },
          ),
          FormHint(hint),
        ],
      ),
    );

    final streamValue = m.streamOverride == null
        ? 'follow'
        : (m.streamOverride! ? 'force_on' : 'force_off');

    return FormSection(
      title: t.t('platform.sectionSpecial'),
      desc: '${t.t('platform.mockConfig')}（${t.t('platform.mockConfigHint')}）',
      children: [
        PlatformField(
          label: '${t.t('platform.mockResponseText')}（response_text）',
          value: m.responseText,
          maxLines: 3,
          onChanged: (v) => c.setMockConfig(m.copyWith(responseText: v)),
        ),
        const SizedBox(height: AidogSpace.sxs),
        PlatformField(
          label: 'finish_reason',
          value: m.finishReason,
          onChanged: (v) => c.setMockConfig(m.copyWith(finishReason: v)),
        ),
        const SizedBox(height: AidogSpace.sxs),
        num1(
          '${t.t('platform.mockStatusCode')}（status_code）',
          m.statusCode,
          (v) => c.setMockConfig(m.copyWith(statusCode: v)),
        ),
        num1(
          '${t.t('platform.mockDelayMs')}（delay_ms）',
          m.delayMs,
          (v) => c.setMockConfig(m.copyWith(delayMs: v)),
        ),
        num1(
          '${t.t('platform.mockInputTokens')}（input_tokens）',
          m.inputTokens,
          (v) => c.setMockConfig(m.copyWith(inputTokens: v)),
        ),
        num1(
          '${t.t('platform.mockOutputTokens')}（output_tokens）',
          m.outputTokens,
          (v) => c.setMockConfig(m.copyWith(outputTokens: v)),
        ),
        num1(
          '${t.t('platform.mockCacheTokens')}（cache_tokens）',
          m.cacheTokens,
          (v) => c.setMockConfig(m.copyWith(cacheTokens: v)),
        ),
        num1(
          '${t.t('platform.mockChunkCount')}（chunk_count）',
          m.chunkCount,
          (v) => c.setMockConfig(m.copyWith(chunkCount: v)),
        ),
        numOpt(
          '${t.t('platform.mockTtftMs')}（ttft_ms）',
          m.ttftMs,
          t.t('platform.mockTtftMsHint'),
          (v) => c.setMockConfig(
            v == null
                ? m.copyWith(clearTtftMs: true)
                : m.copyWith(ttftMs: v.toInt()),
          ),
          min: 0,
        ),
        numOpt(
          '${t.t('platform.mockInterChunkMs')}（inter_chunk_ms）',
          m.interChunkMs,
          t.t('platform.mockInterChunkMsHint'),
          (v) => c.setMockConfig(
            v == null
                ? m.copyWith(clearInterChunkMs: true)
                : m.copyWith(interChunkMs: v.toInt()),
          ),
          min: 0,
        ),
        FormDropdown(
          label: '${t.t('platform.mockErrorMode')}（error_mode）',
          value: m.errorMode,
          options: kMockErrorModes,
          labelOf: (v) => t.t(kMockErrorModeLabelKeys[v] ?? v),
          onChanged: (v) => c.setMockConfig(m.copyWith(errorMode: v)),
        ),
        const SizedBox(height: AidogSpace.sxs),
        FormDropdown(
          label: '${t.t('platform.mockStreamOverride')}（stream_override）',
          value: streamValue,
          options: const ['follow', 'force_on', 'force_off'],
          labelOf: (v) => switch (v) {
            'follow' => '${t.t('platform.mockStreamFollow')}（null）',
            'force_on' => '${t.t('platform.mockStreamForceOn')}（true）',
            _ => '${t.t('platform.mockStreamForceOff')}（false）',
          },
          onChanged: (v) => c.setMockConfig(
            v == 'follow'
                ? m.copyWith(clearStreamOverride: true)
                : m.copyWith(streamOverride: v == 'force_on'),
          ),
        ),
        const SizedBox(height: AidogSpace.sxs),
        numOpt(
          '${t.t('platform.mockErrorRate')}（error_rate）',
          m.errorRate,
          t.t('platform.mockErrorRateHint'),
          (v) => c.setMockConfig(
            v == null
                ? m.copyWith(clearErrorRate: true)
                : m.copyWith(errorRate: v.toDouble()),
          ),
          min: 0,
          max: 1,
        ),
      ],
    );
  }

  // ── F3 配额查询脚本 ────────────────────────────────────────────

  Widget _quotaScriptSection(I18nController t) {
    final variants = c.quotaVariants;
    final customOnly = variants.isEmpty;
    final selection = c.quotaSelection;
    final selVariant = c.selectedQuotaVariant;
    return FormSection(
      title: t.t('platform.quotaScript.title'),
      desc: t.t('platform.quotaScript.desc'),
      children: [
        if (customOnly)
          FormHint(t.t('platform.quotaScript.noBuiltin'))
        else ...[
          FormDropdown(
            label: t.t('platform.quotaScript.variant'),
            value: selection.isEmpty ? kQuotaCustomVariant : selection,
            options: [for (final v in variants) v.id, kQuotaCustomVariant],
            labelOf: (id) => id == kQuotaCustomVariant
                ? t.t('platform.quotaScript.custom')
                : quotaVariantLabel(
                    variants.firstWhere((v) => v.id == id).name,
                    id,
                    t.locale,
                  ),
            onChanged: c.handleQuotaVariantChange,
          ),
          if (c.quotaFellBack) FormHint(t.t('platform.quotaScript.fellBack')),
        ],
        if (selection == kQuotaCustomVariant) ...[
          const SizedBox(height: AidogSpace.sxs),
          PlatformField(
            label: t.t('platform.quotaScript.customLabel'),
            hint: t.t('platform.quotaScript.customPlaceholder'),
            value: c.quotaCustomScript,
            maxLines: 10,
            mono: true,
            onChanged: c.setQuotaCustomScript,
          ),
          FormHint(t.t('platform.quotaScript.customHint')),
        ] else
          for (final r in selVariant?.requires ?? const <QuotaScriptRequire>[])
            Padding(
              padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
              child: PlatformField(
                label: quotaVariantLabel(r.label, r.key, t.locale),
                hint: r.key,
                value: c.quotaRequires[r.key] ?? '',
                onChanged: (v) => c.setQuotaRequire(r.key, v),
              ),
            ),
        // New API 用户 ID：非 requires 字段，保留旧表单入口。
        if (c.protocol == 'newapi')
          PlatformField(
            label: t.t('platform.newapiUserId'),
            hint: t.t('platform.newapiUserIdPlaceholder'),
            value: c.quotaRequires['user_id'] ?? '',
            onChanged: (v) => c.setQuotaRequire('user_id', v),
          ),
      ],
    );
  }

  // ── F4 Devin ───────────────────────────────────────────────────

  Widget _devinSection(I18nController t) => FormSection(
    title: t.t('platform.devinConfig'),
    desc: t.t('platform.devinConfigHint'),
    children: [
      // 值本身就是字符串存的，不过数字键盘与上下步进得给
      // （`formSections.tsx:287-296` 的 `type="number" min=0`）。
      NumberField(
        label: t.t('platform.devinTimeout'),
        hint: t.t('platform.devinTimeoutPlaceholder'),
        value: c.devinConfig.devinTimeout,
        min: 0,
        onChanged: (v) =>
            c.setDevinConfig(c.devinConfig.copyWith(devinTimeout: v)),
      ),
      const SizedBox(height: AidogSpace.sxs),
      FormDropdown(
        label: t.t('platform.devinMode'),
        // radix 那边用 `__none__` 哨兵映射空串，这里同一套路。
        value: c.devinConfig.devinMode.isEmpty
            ? '__none__'
            : c.devinConfig.devinMode,
        options: const ['__none__', ...kDevinModes],
        labelOf: (v) => v == '__none__' ? t.t('platform.devinModeAuto') : v,
        onChanged: (v) => c.setDevinConfig(
          c.devinConfig.copyWith(devinMode: v == '__none__' ? '' : v),
        ),
      ),
    ],
  );

  // ── F5 Claude Code 透传 ────────────────────────────────────────

  Widget _passthroughSection(I18nController t) => FormSection(
    title: t.t('platform.sectionPassthrough'),
    children: [
      PlatformField(
        label: t.t('platform.passthroughBaseUrl'),
        hint: 'https://api.anthropic.com',
        value: c.endpoints.isNotEmpty ? c.endpoints.first.baseUrl : '',
        onChanged: c.setPassthroughBaseUrl,
      ),
      FormHint(t.t('platform.passthroughBaseUrlHint')),
      const SizedBox(height: AidogSpace.sxs),
      _apiKeyField(t, hint: t.t('platform.apiKeyOptional')),
      FormHint(t.t('platform.passthroughNote')),
    ],
  );

  // ── F7 Token ───────────────────────────────────────────────────

  Widget _apiKeyField(I18nController t, {required String hint}) {
    // 创建态多行（每行一个 token，多行触发批量创建预览）；编辑态单行 + 明文切换。
    final multiline = c.editing == null;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: PlatformField(
            value: c.apiKey,
            hint: hint,
            mono: multiline,
            maxLines: multiline ? c.apiKey.split('\n').length.clamp(2, 6) : 1,
            obscure: !multiline && !c.showKey,
            onChanged: c.setApiKey,
          ),
        ),
        if (!multiline) ...[
          // 编辑态且有值才给复制（`formSections.tsx:132-144` 同判据）。
          // 输入框默认密文，不给这颗按钮就只能手选——密文状态下连选都选不准。
          if (c.editing != null && c.apiKey.isNotEmpty) ...[
            const SizedBox(width: AidogSpace.sxs),
            IconButton(
              key: const ValueKey('token-copy'),
              iconSize: 14,
              visualDensity: VisualDensity.compact,
              tooltip: t.t('action.copy'),
              icon: Icon(
                Icons.copy_outlined,
                color: AidogTheme.of(context).c.fg3,
              ),
              onPressed: () => widget.copyText(c.apiKey),
            ),
          ],
          const SizedBox(width: AidogSpace.sxs),
          IconButton(
            iconSize: 14,
            visualDensity: VisualDensity.compact,
            tooltip: c.showKey ? 'Hide token' : 'Show token',
            icon: Icon(
              c.showKey ? Icons.visibility_off : Icons.visibility,
              color: AidogTheme.of(context).c.fg3,
            ),
            onPressed: c.toggleShowKey,
          ),
        ],
      ],
    );
  }

  Widget _authSection(I18nController t) => FormSection(
    title: t.t('platform.sectionAuth'),
    children: [
      _apiKeyField(
        t,
        hint: c.editing != null
            ? t.t('platform.tokenPlaceholderEdit')
            : t.t('platform.tokenPlaceholder'),
      ),
    ],
  );

  // ── F6 Protocol Endpoints ──────────────────────────────────────

  Widget _endpointsSection(I18nController t) {
    final locked = c.endpointsLocked;
    final clientTypes = c.defaults.clientTypes;
    // 默认条目（group == ""）置顶，其余按 group 聚合 —— 下拉里用「组名 / 名称」
    // 一行表达 React 的 SelectGroup 层级。
    final ctValues = <String>[
      for (final x in clientTypes)
        if (x.group.isEmpty) x.value,
      for (final x in clientTypes)
        if (x.group.isNotEmpty) x.value,
    ];
    String ctLabel(String v) {
      for (final x in clientTypes) {
        if (x.value == v) {
          return x.group.isEmpty
              ? t.t('platform.mockDefault')
              : '${x.group} / ${x.label}';
        }
      }
      return v;
    }

    return FormSection(
      title: t.t('platform.endpoints'),
      desc: locked
          ? t.t('platform.endpointsLockedHint')
          : t.t('platform.endpointsHint'),
      action: locked
          ? null
          // `size="sm"` + `fontSize 12, padding "4px 10px", color accent`
          //（`formSectionsEndpoints.tsx:49-59`）。
          : SmallButton(
              label: '+ ${t.t('platform.addEndpoint')}',
              fontSize: 12,
              padding: (10, 4),
              color: AidogTheme.of(context).c.accentText,
              onTap: c.addEndpoint,
            ),
      children: [
        // 空态 12 + 斜体（同上 :62-66）。
        if (c.endpoints.isEmpty)
          FormHint(t.t('platform.noEndpoints'), fontSize: 12, italic: true),
        // 端点行之间只有 FormSection 的 gap 12（`formSections.tsx:58`）；
        // 原先每行还自带 bottom 6，叠出 18。
        for (var i = 0; i < c.endpoints.length; i++)
          // 一条端点 = 一行五个控件（`formSectionsEndpoints.tsx:68-176`），
          // 控件间 `gap: 6`（同上 :68）。原先拆成上下两行，多端点时整张表单被拉得很长。
          Row(
            children: [
              FormDropdown(
                width: 120,
                // `SelectTrigger className="input"` = 描边盒 + 13（同上 :80）。
                boxed: true,
                fontSize: 13,
                value: c.endpoints[i].protocol,
                options: [for (final p in kEndpointProtocols) p.value],
                labelOf: (v) {
                  for (final p in kEndpointProtocols) {
                    if (p.value == v) return p.label;
                  }
                  return v;
                },
                onChanged: locked ? null : (v) => c.setEndpointProtocol(i, v),
              ),
              const SizedBox(width: AidogSpace.ssm),
              Expanded(
                child: PlatformField(
                  value: c.endpoints[i].baseUrl,
                  hint: 'Endpoint Base URL',
                  enabled: !locked,
                  onChanged: (v) => c.setEndpointBaseUrl(i, v),
                ),
              ),
              const SizedBox(width: AidogSpace.ssm),
              // 下拉本身没有标签，「这一列是干嘛的」只写在悬浮提示里
              //（`formSectionsEndpoints.tsx:109` 的 `title=`）。
              Tooltip(
                message: t.t('platform.clientType'),
                child: FormDropdown(
                  width: 140,
                  boxed: true,
                  fontSize: 13,
                  value: c.endpoints[i].clientType.isEmpty
                      ? 'default'
                      : c.endpoints[i].clientType,
                  options: ctValues,
                  labelOf: ctLabel,
                  onChanged: locked
                      ? null
                      : (v) => c.setEndpointClientType(i, v),
                ),
              ),
              const SizedBox(width: AidogSpace.ssm),
              // Coding Plan 开关。开启时绿色，因为绿 = 走 coding 套餐
              //（`formSectionsEndpoints.tsx:140-162`），通用高亮色讲不出这层语义。
              // 形状是固定 28×28 方块、`padding 0`、11 w700（同上 :144-153）。
              Tooltip(
                message: c.endpoints[i].codingPlan
                    ? 'Coding Plan ON'
                    : 'Coding Plan',
                child: SizedBox(
                  width: 28,
                  height: 28,
                  child: SmallButton(
                    label: 'C',
                    padding: (0, 0),
                    fontWeight: FontWeight.w700,
                    active: c.endpoints[i].codingPlan,
                    activeTone: AidogTheme.of(context).c.ok,
                    onTap: locked ? null : () => c.toggleEndpointCodingPlan(i),
                  ),
                ),
              ),
              if (!locked) ...[
                const SizedBox(width: AidogSpace.ssm),
                // React 是 `size="icon"`（36×36）+ 14×14 垃圾桶 SVG，不是文字按钮
                //（同上 :163-175）—— 文字按钮把整行撑宽。
                IconButton(
                  icon: const Icon(Icons.delete_outline, size: 14),
                  color: AidogTheme.of(context).c.bad,
                  padding: EdgeInsets.zero,
                  splashRadius: 18,
                  constraints: const BoxConstraints.tightFor(
                    width: 36,
                    height: 36,
                  ),
                  tooltip: t.t('action.delete'),
                  onPressed: () => c.removeEndpoint(i),
                ),
              ],
            ],
          ),
      ],
    );
  }

  // ── F8 多 key 批量创建预览 ─────────────────────────────────────

  Widget _multiKeyPreview(I18nController t) {
    final theme = AidogTheme.of(context);
    final keys = c.batchPreviewKeys!;
    final names = c.previewNames;
    final baseUrl = _primaryBaseUrlOf(c.protocol);
    // 批量创建前的确认区要显眼（`MultiKeyPreview.tsx:34-76`）：accent 描边强调，
    // 每行自带底色，协议是徽标而不是一串裸字。走普通 FormSection 的话，
    // 它和上面十几个分区长得一模一样，用户容易直接点提交。
    // 淡入（`MultiKeyPreview.tsx:36` 的 `animate-fade-in`）：
    // 这张卡是粘了多把 key 之后凭空出现的，直接闪出来容易被当成误操作。
    return Reveal(
      child: Container(
        margin: const EdgeInsets.only(bottom: AidogSpace.smd),
        // 外框 `padding: 14`、底 `--bg-glass`（= surface）（`MultiKeyPreview.tsx:39-40`）。
        // 描边 React 写的是纯 `var(--accent)`，深色下 accent 近黑等于消失，
        // 这里保留 accentEdge（同 `c1-platforms.md` #29 的裁决）。
        padding: const EdgeInsets.all(14),
        decoration: BoxDecoration(
          color: theme.c.surface,
          border: Border.all(color: theme.c.accentEdge),
          borderRadius: BorderRadius.circular(AidogRadius.md),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 标题是 `.section-title` + 14 → 14 w700 ls -0.02em（同上 :44）。
            Text(
              t.t('platform.batch.previewTitle', {'count': keys.length}),
              style: AidogType.label.copyWith(
                fontSize: 14,
                fontWeight: FontWeight.w700,
                letterSpacing: -0.28,
                color: theme.c.fg,
              ),
            ),
            // 标题↔提示 `gap: 4`、提示 12（同上 :43,47）。
            const SizedBox(height: 4),
            Text(
              t.t('platform.batch.previewHint', {'base': '{base}'}),
              style: AidogType.caption.copyWith(
                fontSize: 12,
                color: theme.c.fg3,
              ),
            ),
            // 标题块↔列表 `gap: 10`（同上 :38）。
            const SizedBox(height: 10),
            for (var i = 0; i < keys.length; i++)
              Container(
                margin: const EdgeInsets.only(bottom: 4),
                // 行 `padding "6px 8px"`、底 `--bg-elevated`（= surface）（同上 :61-62）。
                padding: const EdgeInsets.symmetric(
                  horizontal: 8,
                  vertical: 6,
                ),
                decoration: BoxDecoration(
                  color: theme.c.surface,
                  borderRadius: BorderRadius.circular(AidogRadius.sm),
                ),
                child: Row(
                  children: [
                    // 序号列 24 宽，行内字号继承 12（同上 :59,62）。
                    SizedBox(
                      width: 24,
                      child: Text(
                        '#${i + 1}',
                        style: AidogType.caption.copyWith(
                          fontSize: 12,
                          color: theme.c.fg3,
                        ),
                      ),
                    ),
                    // 列间 `gap: 8`（同上 :60），原先五列直接相邻。
                    const SizedBox(width: 8),
                    Expanded(
                      flex: 3,
                      child: Text(
                        i < names.length ? names[i] : '',
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.label.copyWith(
                          fontSize: 12,
                          fontWeight: FontWeight.w600,
                          color: theme.c.fg,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      flex: 2,
                      child: Align(
                        alignment: AlignmentDirectional.centerStart,
                        // 协议徽标：`padding "2px 6px"`、r8、底 --bg-glass、
                        // 10 w700、**无描边**（同上 :67-70）。
                        child: MiniBadge(
                          text: c.protocol.toUpperCase(),
                          color: theme.c.fg2,
                          background: theme.c.surface,
                          borderColor: Colors.transparent,
                          padY: 2,
                          radius: AidogRadius.sm,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      flex: 3,
                      child: Text(
                        baseUrl.isEmpty ? '—' : baseUrl,
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.caption.copyWith(
                          fontSize: 12,
                          color: theme.c.fg2,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      flex: 2,
                      child: Text(
                        maskTail(keys[i]),
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.numSm.copyWith(
                          fontSize: 12,
                          color: theme.c.fg3,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
          ],
        ),
      ),
    );
  }

  // ── F9 + F15 模型矩阵（默认列 + 时段档列）──────────────────────

  Widget _modelsMatrixSection(I18nController t) {
    final theme = AidogTheme.of(context);
    final rules = c.timeModels;
    final candidates = c.modelDropdownSource;
    const labelW = 64.0;
    // 列宽随窗口自适应：默认列 + 各时段档列平分剩余宽度，软下限 80
    //（`ModelsMatrixSection.tsx:266-271` 的 `flex:1` + `minWidth:80`）。
    // 原先恒 200，窄窗口下时段档列被推出可视区，必须横向拖才看得到。
    final cols = 1 + rules.length;
    // 行内每列之间还有 8 的间距（`ModelsMatrixSection.tsx:270`），一并扣掉。
    final avail =
        MediaQuery.sizeOf(context).width -
        labelW -
        2 * AidogSpace.s_2xl -
        8 * cols;
    final cellW = math.max(80.0, avail / cols);

    Widget cell(String value, ValueChanged<String> onChanged) => SizedBox(
      width: cellW,
      child: ModelCell(
        value: value,
        candidates: candidates,
        onChanged: onChanged,
        hint: t.t('platform.models_placeholder'),
        pickTooltip: t.t('platform.selectModel'),
      ),
    );

    return FormSection(
      title: t.t('platform.models'),
      // 四颗操作按钮：`size="sm"` + `fontSize 12, padding "4px 10px"`，组内 `gap: 6`；
      // 「获取模型」「添加时段档」另染 accent（`ModelsMatrixSection.tsx:277-314`）。
      action: Wrap(
        spacing: AidogSpace.ssm,
        runSpacing: AidogSpace.ssm,
        alignment: WrapAlignment.end,
        children: [
          Tooltip(
            message: t.t('platform.fillAllHint'),
            child: SmallButton(
              label: t.t('platform.fillAll'),
              fontSize: 12,
              padding: (10, 4),
              onTap: c.models['default']!.trim().isEmpty
                  ? null
                  : c.handleFillAll,
            ),
          ),
          SmallButton(
            label: c.fetching
                ? t.t('status.loading')
                : t.t('platform.fetchModels'),
            fontSize: 12,
            padding: (10, 4),
            color: theme.c.accentText,
            onTap: (c.apiKeyMissing || c.endpoints.isEmpty || c.fetching)
                ? null
                : () => c.handleFetchModels(
                    emptyText: t.t('platform.fetchEmpty'),
                    authText: (code) =>
                        t.t('platform.fetchAuthError', {'code': code}),
                  ),
          ),
          Tooltip(
            message: c.peak.isEmpty ? t.t('platform.time_windows_no_peak') : '',
            child: SmallButton(
              label: t.t('platform.time_windows_import_peak'),
              fontSize: 12,
              padding: (10, 4),
              onTap: c.peak.isEmpty
                  ? null
                  : () => setState(() => _importPeakOpen = true),
            ),
          ),
          SmallButton(
            label: '+ ${t.t('platform.time_windows_add_rule')}',
            fontSize: 12,
            padding: (10, 4),
            color: theme.c.accentText,
            onTap: () => c.setTimeModels([
              ...rules,
              const TimeModelRule(windows: [], models: {}),
            ]),
          ),
        ],
      ),
      children: [
        // 取模型报错行 12（`ModelsMatrixSection.tsx:319`）。
        if (c.fetchError.isNotEmpty)
          FormHint(c.fetchError, danger: true, fontSize: 12),
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          // 横向滚动区 `paddingBottom: 4`（同上 :325）。
          padding: const EdgeInsets.only(bottom: 4),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              // 列头行。行内 `gap: 8`、`alignItems: center`（同上 :269-271），
              // 原先标签列与各单元格零间距、且顶边对齐。
              Row(
                children: [
                  const SizedBox(width: labelW),
                  const SizedBox(width: 8),
                  SizedBox(
                    width: cellW,
                    child: Center(
                      child: Text(
                        t.t('platform.modelDefault'),
                        style: AidogType.tile.copyWith(
                          fontSize: 13,
                          color: theme.c.fg2,
                        ),
                      ),
                    ),
                  ),
                  for (var ri = 0; ri < rules.length; ri++) ...[
                    const SizedBox(width: 8),
                    SizedBox(
                      width: cellW,
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Tooltip(
                            message: t.t('platform.time_windows_edit_windows'),
                            // 列头按钮 `fontSize 12, padding "3px 4px"`（同上 :346-351）。
                            child: SmallButton(
                              label: describeWindows(
                                rules[ri].windows,
                                c.windowsTz,
                                t,
                              ),
                              fontSize: 12,
                              padding: (4, 3),
                              onTap: () => setState(() => _editingRuleIdx = ri),
                            ),
                          ),
                          const SizedBox(height: 2),
                          Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              // 三颗按钮只有符号没有文字，解释全靠悬浮提示
                              //（React `ModelsMatrixSection.tsx:364/374/384`
                              // 的 `title=`）；档位是 `padding "1px 4px", fontSize 12`。
                              SmallButton(
                                label: '↑',
                                fontSize: 12,
                                padding: (4, 1),
                                tooltip: t.t('action.moveUp'),
                                onTap: ri == 0 ? null : () => _moveRule(ri, -1),
                              ),
                              const SizedBox(width: 2),
                              SmallButton(
                                label: '↓',
                                fontSize: 12,
                                padding: (4, 1),
                                tooltip: t.t('action.moveDown'),
                                onTap: ri == rules.length - 1
                                    ? null
                                    : () => _moveRule(ri, 1),
                              ),
                              const SizedBox(width: 2),
                              SmallButton(
                                label: '×',
                                danger: true,
                                fontSize: 12,
                                padding: (4, 1),
                                tooltip: t.t('action.delete'),
                                onTap: () => _removeRule(ri),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ],
                ],
              ),
              const SizedBox(height: AidogSpace.ssm),
              // 5 槽行
              for (final slot in kModelSlots)
                Padding(
                  padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                  child: Row(
                    children: [
                      SizedBox(
                        width: labelW,
                        child: Text(
                          t.t(slot.labelKey),
                          textAlign: TextAlign.right,
                          // slot 标签 13 w500 tertiary（同上 :397-401）。
                          style: AidogType.label.copyWith(
                            fontSize: 13,
                            fontWeight: FontWeight.w500,
                            color: theme.c.fg3,
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      cell(
                        c.models[slot.key] ?? '',
                        (v) => c.setModel(slot.key, v),
                      ),
                      for (var ri = 0; ri < rules.length; ri++) ...[
                        const SizedBox(width: 8),
                        cell(
                          rules[ri].models[slot.key] ?? '',
                          (v) => _updateRuleModel(ri, slot.key, v),
                        ),
                      ],
                    ],
                  ),
                ),
              // 空态 13 + 斜体 + 左缩进 LABEL_W+8 = 72（同上 :428）。
              if (rules.isEmpty)
                Padding(
                  padding: const EdgeInsetsDirectional.only(start: 72),
                  child: FormHint(
                    t.t('platform.time_windows_empty'),
                    fontSize: 13,
                    italic: true,
                  ),
                ),
            ],
          ),
        ),
        if (_editingRuleIdx != null)
          WindowsEditor(
            key: ValueKey('windows-$_editingRuleIdx'),
            initial: rules[_editingRuleIdx!].windows,
            tzMode: c.windowsTz,
            onTzMode: c.setWindowsTz,
            onCancel: () => setState(() => _editingRuleIdx = null),
            onSave: (ws) {
              final idx = _editingRuleIdx!;
              c.setTimeModels([
                for (var i = 0; i < rules.length; i++)
                  i == idx ? rules[i].copyWith(windows: ws) : rules[i],
              ]);
              setState(() => _editingRuleIdx = null);
            },
          ),
        if (_importPeakOpen)
          // React 是普通 `Dialog`：maxWidth 400、自带右上 ✕、标题 13、正文 12、
          // 页脚按钮是默认档（14 / px16 / 实心确认）（`ModelsMatrixSection.tsx:450-473`）。
          ConfirmCard(
            title: t.t('platform.time_windows_import_confirm_title'),
            body: t.t('platform.time_windows_import_confirm_body', {
              'count': c.peak.length,
            }),
            confirmLabel: t.t('platform.time_windows_import_confirm_button'),
            maxWidth: 400,
            titleStyle: AidogType.title.copyWith(
              fontSize: 13,
              fontWeight: FontWeight.w600,
            ),
            bodyStyle: AidogType.micro.copyWith(
              fontSize: 12,
              letterSpacing: 0,
              color: theme.c.fg2,
            ),
            buttonFontSize: 14,
            buttonPadding: (16, 8),
            onClose: () => setState(() => _importPeakOpen = false),
            onCancel: () => setState(() => _importPeakOpen = false),
            onConfirm: () {
              c.setTimeModels([
                ...rules,
                TimeModelRule(
                  windows: [for (final w in c.peak) w.clone()],
                  models: const {},
                ),
              ]);
              setState(() => _importPeakOpen = false);
            },
          ),
      ],
    );
  }

  void _moveRule(int idx, int delta) {
    final next = [...c.timeModels];
    final tmp = next[idx];
    next[idx] = next[idx + delta];
    next[idx + delta] = tmp;
    c.setTimeModels(next);
  }

  void _removeRule(int idx) {
    c.setTimeModels([
      for (var i = 0; i < c.timeModels.length; i++)
        if (i != idx) c.timeModels[i],
    ]);
    if (_editingRuleIdx == idx) setState(() => _editingRuleIdx = null);
  }

  void _updateRuleModel(int idx, String slot, String value) {
    c.setTimeModels([
      for (var i = 0; i < c.timeModels.length; i++)
        if (i != idx)
          c.timeModels[i]
        else
          c.timeModels[i].copyWith(
            models: {
              for (final e in c.timeModels[i].models.entries)
                if (e.key != slot) e.key: e.value,
              if (value.isNotEmpty) slot: value,
            },
          ),
    ]);
  }

  // ── F10 手动预算 ───────────────────────────────────────────────

  Widget _manualBudgetsSection(I18nController t) {
    final tiers = c.planTiers;
    final selectedTier = c.selectedPlanTier;
    return FormSection(
      title: t.t('platform.manualBudgetTitle'),
      desc: t.t('platform.manualBudgetDesc'),
      action: Wrap(
        spacing: AidogSpace.sxs,
        runSpacing: AidogSpace.sxs,
        alignment: WrapAlignment.end,
        children: [
          if (tiers.length > 1)
            FormDropdown(
              width: 120,
              value: selectedTier?.id ?? '',
              options: [for (final x in tiers) x.id],
              labelOf: (id) => tiers.firstWhere((x) => x.id == id).name,
              onChanged: c.setPlanTierId,
            ),
          if (tiers.isNotEmpty)
            Tooltip(
              message: selectedTier?.sourceUrl ?? '',
              child: SmallButton(
                label: t.t('platform.manualBudgetUseTier', {
                  'tier': selectedTier?.name ?? '',
                }),
                onTap: selectedTier == null
                    ? null
                    : () => c.setManualBudgets(
                        PlatformFormController.tierToBudgets(selectedTier),
                      ),
              ),
            ),
          SmallButton(
            label: t.t('platform.manualBudgetAdd'),
            onTap: () =>
                c.setManualBudgets([...c.manualBudgets, newManualBudget()]),
          ),
        ],
      ),
      children: [
        if (c.manualBudgets.isEmpty)
          FormHint(t.t('platform.manualBudgetEmpty')),
        for (var i = 0; i < c.manualBudgets.length; i++)
          _budgetRow(t, i, c.manualBudgets[i]),
      ],
    );
  }

  Widget _budgetRow(I18nController t, int idx, ManualBudget b) {
    void update(ManualBudget next) => c.setManualBudgets([
      for (var i = 0; i < c.manualBudgets.length; i++)
        i == idx ? next : c.manualBudgets[i],
    ]);
    final needsWindow = b.kind == 'rolling' || b.kind == 'fixed';
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Wrap(
        spacing: AidogSpace.ssm,
        runSpacing: AidogSpace.sxs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          FormDropdown(
            width: 110,
            value: b.kind,
            options: kManualBudgetKinds,
            labelOf: (v) => switch (v) {
              'total' => t.t('platform.manualBudgetKindTotal'),
              'rolling' => t.t('platform.manualBudgetKindRolling'),
              'fixed' => t.t('platform.manualBudgetKindFixed'),
              _ => t.t('platform.manualBudgetKindDaily'),
            },
            onChanged: (kind) {
              final willNeed = kind == 'rolling' || kind == 'fixed';
              // 切到 rolling/fixed 且尚无窗口配置 → 给合理默认（7 天）。
              if (willNeed && (b.windowHours == null || b.windowHours! <= 0)) {
                update(
                  b.copyWith(kind: kind, windowHours: 7, windowUnit: 'day'),
                );
              } else {
                update(b.copyWith(kind: kind));
              }
            },
          ),
          FormDropdown(
            // 单位下拉 90（`formSections.tsx:482`）。
            width: 90,
            value: b.unit,
            options: kManualBudgetUnits,
            labelOf: (v) => switch (v) {
              'usd' => r'$ USD',
              'token' => t.t('platform.manualBudgetUnitToken'),
              _ => t.t('platform.manualBudgetUnitCount'),
            },
            onChanged: (v) => update(b.copyWith(unit: v)),
          ),
          // 数字键盘 + 解析失败保留原文（`formSections.tsx:491-501` 是
          // `type="number"`）。原先是 `?? 0`：打到一半的 `10.` 直接变 0，
          // 外层一重建还会把框里的字换成那个 0。
          DecimalField(
            width: 100,
            value: b.amount == 0 ? '' : '${b.amount}',
            hint: t.t('platform.manualBudgetAmount'),
            invalidText: t.t('platform.numberInvalid'),
            onParsed: (v) => update(b.copyWith(amount: v ?? 0)),
          ),
          if (needsWindow) ...[
            DecimalField(
              // 窗口数值框 80（`formSections.tsx:508`）。
              width: 80,
              value: b.windowHours == null ? '' : '${b.windowHours}',
              hint: t.t('platform.manualBudgetWindow'),
              invalidText: t.t('platform.numberInvalid'),
              onParsed: (v) => update(
                v == null
                    ? b.copyWith(clearWindowHours: true)
                    : b.copyWith(windowHours: v),
              ),
            ),
            FormDropdown(
              // 单位下拉 90（`formSections.tsx:517`）。
              width: 90,
              value: b.windowUnit,
              options: kWindowUnits,
              labelOf: (v) => switch (v) {
                'minute' => t.t('platform.windowUnitMinute'),
                'hour' => t.t('platform.windowUnitHour'),
                'day' => t.t('platform.windowUnitDay'),
                'week' => t.t('platform.windowUnitWeek'),
                _ => t.t('platform.windowUnitMonth'),
              },
              onChanged: (v) => update(b.copyWith(windowUnit: v)),
            ),
          ],
          // 复选框而不是高亮按钮（`formSections.tsx:530-536`）：选中与否
          // 只靠描边色的话，一排按钮里根本看不出哪几条启用了。
          _CheckRow(
            label: t.t('platform.manualBudgetEnabled'),
            value: b.enabled,
            onChanged: (v) => update(b.copyWith(enabled: v)),
          ),
          SmallButton(
            label: t.t('action.delete'),
            danger: true,
            onTap: () => c.setManualBudgets([
              for (var i = 0; i < c.manualBudgets.length; i++)
                if (i != idx) c.manualBudgets[i],
            ]),
          ),
        ],
      ),
    );
  }

  // ── F11 熔断覆盖 ───────────────────────────────────────────────

  Widget _breakerSection(I18nController t) {
    final d = c.breakerDefaults;
    String hintOf(int? n) => n == null
        ? t.t('platform.breakerInheritGeneric')
        : t.t('platform.breakerInherit', {'n': n});
    return FormSection(
      title: t.t('platform.breakerTitle'),
      desc: t.t('platform.breakerDesc'),
      // 标签左、输入右、三行对齐（`formSections.tsx:568-590` 的
      // `grid-template-columns: auto 1fr`）。原先标签在上输入在下，
      // 高度翻倍且三项不成列。输入一律数字型（同处 `type="number" min=0`）。
      children: [
        _BreakerRow(
          fieldKey: const ValueKey('breaker-failure'),
          label: t.t('platform.breakerFailureThreshold'),
          hint: hintOf(d?.failureThreshold),
          value: c.breakerFailureThreshold,
          onChanged: c.setBreakerFailureThreshold,
        ),
        _BreakerRow(
          fieldKey: const ValueKey('breaker-open-secs'),
          label: t.t('platform.breakerOpenSecs'),
          hint: hintOf(d?.openSecs),
          value: c.breakerOpenSecs,
          onChanged: c.setBreakerOpenSecs,
        ),
        _BreakerRow(
          fieldKey: const ValueKey('breaker-half-open-max'),
          label: t.t('platform.breakerHalfOpenMax'),
          hint: hintOf(d?.halfOpenMax),
          value: c.breakerHalfOpenMax,
          onChanged: c.setBreakerHalfOpenMax,
        ),
      ],
    );
  }

  // ── F12 高峰时段 ───────────────────────────────────────────────

  Widget _peakSection(I18nController t) {
    final theme = AidogTheme.of(context);
    final presetPeak = c.presetPeak;
    // 实时算（基于当前 windows + now）。无窗口 → 视为非高峰。
    final nowPeak = isCurrentlyPeak(
      c.peak,
      DateTime.now().millisecondsSinceEpoch,
    );
    return FormSection(
      title: t.t('platform.peak'),
      desc: t.t('platform.peak_desc'),
      action: Wrap(
        spacing: AidogSpace.sxs,
        runSpacing: AidogSpace.sxs,
        alignment: WrapAlignment.end,
        children: [
          // 两颗时区按钮包在一个分段容器里：`padding 2` + bg-glass + 1px 边 +
          // 圆角 8，按钮本身 `2px 8px` / 11（`formSections.tsx:732-744`）。
          Container(
            padding: const EdgeInsets.all(2),
            decoration: BoxDecoration(
              color: theme.c.surface,
              border: Border.all(color: theme.c.line),
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                SmallButton(
                  label: t.t('platform.timezone_local'),
                  padding: (8, 2),
                  active: c.windowsTz == TzMode.local,
                  onTap: () => c.setWindowsTz(TzMode.local),
                ),
                SmallButton(
                  label: t.t('platform.timezone_utc'),
                  padding: (8, 2),
                  active: c.windowsTz == TzMode.utc,
                  onTap: () => c.setWindowsTz(TzMode.utc),
                ),
              ],
            ),
          ),
          Tooltip(
            message: presetPeak.isEmpty ? t.t('platform.peak_no_default') : '',
            child: SmallButton(
              label: t.t('platform.peak_import_default'),
              onTap: presetPeak.isEmpty
                  ? null
                  : () => setState(() => _overwritePeakOpen = true),
            ),
          ),
        ],
      ),
      children: [
        // 高峰禁用开关 + 实时态。整行 `padding 8` + bg-glass + 边 + 圆角 8
        //（`formSections.tsx:760`）。
        Container(
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(
            color: theme.c.surface,
            border: Border.all(color: theme.c.line),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: Row(
          children: [
            // 同上：复选框（`formSections.tsx:760-767`）。
            _CheckRow(
              label: t.t('platform.disable_during_peak'),
              value: c.disableDuringPeak,
              onChanged: c.setDisableDuringPeak,
              bold: true,
            ),
            if (c.disableDuringPeak) ...[
              const SizedBox(width: AidogSpace.ssm),
              // 实时态是徽标不是裸字（`formSections.tsx:769-786`）：
              // 命中高峰时红底红边，一眼看得到。
              MiniBadge(
                text: nowPeak
                    ? t.t('platform.currently_peak')
                    : t.t('platform.currently_off_peak'),
                color: nowPeak ? theme.c.bad : theme.c.fg3,
              ),
            ],
          ],
          ),
        ),
        FormHint(t.t('platform.disable_during_peak_desc')),
        if (c.peak.isEmpty) FormHint(t.t('platform.peak_empty')),
        for (var i = 0; i < c.peak.length; i++) _peakWindowRow(t, i, c.peak[i]),
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: SmallButton(
            label: '+ ${t.t('platform.add_window')}',
            onTap: () => c.setPeak([
              ...c.peak,
              const TimeWindow(startHour: 0, endHour: 24, multiplier: 1.0),
            ]),
          ),
        ),
        if (_overwritePeakOpen)
          ConfirmCard(
            title: t.t('platform.peak_overwrite_confirm_title'),
            body: t.t('platform.peak_overwrite_confirm_body', {
              'count': presetPeak.length,
            }),
            confirmLabel: t.t('platform.peak_overwrite_confirm_button'),
            onCancel: () => setState(() => _overwritePeakOpen = false),
            onConfirm: () {
              c.setPeak([for (final w in presetPeak) w.clone()]);
              setState(() => _overwritePeakOpen = false);
            },
          ),
      ],
    );
  }

  Widget _peakWindowRow(I18nController t, int idx, TimeWindow w) {
    final theme = AidogTheme.of(context);
    void update(TimeWindow next) => c.setPeak([
      for (var i = 0; i < c.peak.length; i++) i == idx ? next : c.peak[i],
    ]);
    // 无 timezone（= UTC 存储）按 tzMode 双向换算；带 timezone 的存值即本地值。
    ({int hour, int minute}) toDisp(int h, int m) => w.timezone != null
        ? (hour: h, minute: m)
        : utcToDisplay(h, m, c.windowsTz);
    ({int hour, int minute}) fromDisp(int h, int m) => w.timezone != null
        ? (hour: h, minute: m)
        : displayToUtc(h, m, c.windowsTz);
    final startDisp = toDisp(w.startHour, w.startMinute ?? 0);
    final endDisp = toDisp(w.endHour, w.endMinute ?? 0);

    // 时 / 分 / 倍率都是数字（`formSections.tsx:813-860` 的 `type="number"`）：
    // 数字键盘 + 上下步进，不再是裸文本框。
    Widget numBox(
      double width,
      int value,
      ValueChanged<String> onChanged, {
      required int max,
      required String slot,
    }) => NumberField(
      key: ValueKey('peak-$idx-$slot'),
      width: width,
      value: '$value',
      min: 0,
      max: max,
      onChanged: onChanged,
    );

    return Container(
      margin: const EdgeInsets.only(bottom: AidogSpace.ssm),
      // 窗口卡 `padding: 8`、底 --bg-glass（= surface）（`formSections.tsx:797`）。
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        color: theme.c.surface,
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Wrap(
            spacing: AidogSpace.ssm,
            runSpacing: AidogSpace.sxs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Text(
                t.t('platform.start_hour'),
                style: AidogType.label.copyWith(color: theme.c.fg2),
              ),
              numBox(slot: 'start-hour', max: 23, 64, startDisp.hour, (v) {
                final u = fromDisp(clampInt(v, 0, 23), startDisp.minute);
                update(update0(w, startHour: u.hour, startMinute: u.minute));
              }),
              numBox(slot: 'start-minute', max: 59, 64, startDisp.minute, (v) {
                final u = fromDisp(startDisp.hour, clampInt(v, 0, 59));
                update(update0(w, startHour: u.hour, startMinute: u.minute));
              }),
              Text(
                t.t('platform.end_hour'),
                style: AidogType.label.copyWith(color: theme.c.fg2),
              ),
              numBox(slot: 'end-hour', max: 24, 64, endDisp.hour, (v) {
                final u = fromDisp(clampInt(v, 0, 24), endDisp.minute);
                update(update0(w, endHour: u.hour, endMinute: u.minute));
              }),
              numBox(slot: 'end-minute', max: 59, 64, endDisp.minute, (v) {
                final u = fromDisp(endDisp.hour, clampInt(v, 0, 59));
                update(update0(w, endHour: u.hour, endMinute: u.minute));
              }),
              Text(
                t.t('platform.multiplier'),
                style: AidogType.label.copyWith(color: theme.c.fg2),
              ),
              NumberField(
                key: ValueKey('peak-$idx-multiplier'),
                // 倍率框 84（`formSections.tsx:857`）。
                width: 84,
                value: '${w.multiplier}',
                min: 0,
                step: 0.1,
                onChanged: (v) => update(
                  w.copyWith(multiplier: double.tryParse(v.trim()) ?? 1),
                ),
              ),
              FormDropdown(
                width: 160,
                value: w.timezone ?? '__utc__',
                options: kWindowTimezones,
                labelOf: (tz) => tz == '__utc__'
                    ? t.t('platform.window_timezone_utc_default')
                    : tz,
                onChanged: (v) => update(
                  v == '__utc__'
                      ? w.copyWith(clearTimezone: true)
                      : w.copyWith(timezone: v),
                ),
              ),
              WeekdayToggles(
                selected: w.daysOfWeek ?? const [],
                tooltipOf: (_) => t.t('platform.days_of_week'),
                onToggle: (day) {
                  final cur = w.daysOfWeek ?? const <int>[];
                  final next = cur.contains(day)
                      ? [
                          for (final d in cur)
                            if (d != day) d,
                        ]
                      : ([...cur, day]..sort());
                  update(
                    next.isEmpty
                        ? w.copyWith(clearDaysOfWeek: true)
                        : w.copyWith(daysOfWeek: next),
                  );
                },
              ),
              SmallButton(
                label: '✕',
                danger: true,
                onTap: () => c.setPeak([
                  for (var i = 0; i < c.peak.length; i++)
                    if (i != idx) c.peak[i],
                ]),
              ),
            ],
          ),
          FormHint(formatWindowPreview(w, c.windowsTz, t)),
          // model scope：chip 多选 + 自由输入（支持 `prefix*` 通配）。
          Row(
            children: [
              Text(
                t.t('platform.peak_model_scope'),
                style: AidogType.caption.copyWith(color: theme.c.fg2),
              ),
              if (w.models == null || w.models!.isEmpty)
                Text(
                  ' · ${t.t('platform.peak_model_scope_all')}',
                  style: AidogType.caption.copyWith(color: theme.c.fg3),
                ),
            ],
          ),
          Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              // chip 本体 + 内嵌 ✕（`formSections.tsx:931-952`）：
              // 整颗都可点的话，想看看名字就把它删了。
              for (var mi = 0; mi < (w.models?.length ?? 0); mi++)
                _ModelChip(
                  label: w.models![mi],
                  removeTooltip: t.t('action.delete'),
                  onRemove: () {
                    final next = [
                      for (var j = 0; j < w.models!.length; j++)
                        if (j != mi) w.models![j],
                    ];
                    update(
                      next.isEmpty
                          ? w.copyWith(clearModels: true)
                          : w.copyWith(models: next),
                    );
                  },
                ),
              SizedBox(
                width: 180,
                child: _ModelScopeInput(
                  candidates: c.modelDropdownSource,
                  hint: t.t('platform.peak_model_placeholder'),
                  onSubmit: (raw) {
                    final v = raw.trim();
                    if (v.isEmpty) return;
                    final cur = w.models ?? const <String>[];
                    if (cur.contains(v)) return;
                    update(w.copyWith(models: [...cur, v]));
                  },
                ),
              ),
            ],
          ),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.ssm,
            runSpacing: AidogSpace.sxs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              SizedBox(
                // 生效起始 / 截止输入 180（`formSections.tsx:986-1001`）。
                width: 180,
                child: DateTimeField(
                  idPrefix: 'peak-$idx-start-at',
                  label: t.t('platform.peak_start_at'),
                  value: secToLocalInput(w.startAt),
                  invalidText: t.t('platform.dateTimeInvalid'),
                  pickTooltip: t.t('platform.pickDateTime'),
                  onChanged: (v) {
                    final sec = localInputToSec(v);
                    update(
                      sec == null
                          ? w.copyWith(clearStartAt: true)
                          : w.copyWith(startAt: sec),
                    );
                  },
                ),
              ),
              SizedBox(
                // 生效起始 / 截止输入 180（`formSections.tsx:986-1001`）。
                width: 180,
                child: DateTimeField(
                  idPrefix: 'peak-$idx-end-at',
                  label: t.t('platform.peak_end_at'),
                  value: secToLocalInput(w.endAt),
                  invalidText: t.t('platform.dateTimeInvalid'),
                  pickTooltip: t.t('platform.pickDateTime'),
                  onChanged: (v) {
                    final sec = localInputToSec(v);
                    update(
                      sec == null
                          ? w.copyWith(clearEndAt: true)
                          : w.copyWith(endAt: sec),
                    );
                  },
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  // ── F13 分组归属 ───────────────────────────────────────────────

  Widget _groupAssignSection(I18nController t) {
    final theme = AidogTheme.of(context);
    final gds = c.list.groupDetails;
    final locked = c.lockedGroupId;
    return FormSection(
      title: t.t('platform.groupAssignTitle'),
      desc: t.t('platform.groupAssignDesc'),
      children: [
        if (locked != null)
          Row(
            children: [
              Text(
                gds
                        .where((g) => g.group.id == locked)
                        .map((g) => g.group.name)
                        .firstOrNull ??
                    '#$locked',
                style: AidogType.label.copyWith(color: theme.c.fg),
              ),
              const SizedBox(width: AidogSpace.ssm),
              Text(
                t.t('platform.groupLocked'),
                style: AidogType.caption.copyWith(color: theme.c.fg3),
              ),
            ],
          )
        else if (c.editing == null)
          // 「创建默认分组」是创建时一次性判断，仅创建表单显示。
          Row(
            children: [
              Expanded(
                child: Text(
                  t.t('platform.groupAssignAuto'),
                  style: AidogType.label.copyWith(color: theme.c.fg),
                ),
              ),
              AidogSwitch(
                value: c.autoGroup,
                compact: true,
                onChanged: () => c.setAutoGroup(!c.autoGroup),
              ),
            ],
          ),
        if (locked == null && gds.isNotEmpty) ...[
          const SizedBox(height: AidogSpace.ssm),
          FieldLabel(t.t('platform.groupAssignJoin')),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            children: [
              for (final gd in gds)
                // 编辑态隐藏该平台自己的 auto 分组（由上方开关管理）。
                if (c.editing == null ||
                    gd.group.autoFromPlatform != '${c.editing!.id}')
                  SmallButton(
                    label: gd.group.name,
                    pill: true,
                    active: c.joinGroupIds.contains(gd.group.id),
                    onTap: () => c.toggleJoinGroup(gd.group.id),
                  ),
            ],
          ),
        ],
      ],
    );
  }

  // ── F14 过期时间 ───────────────────────────────────────────────

  Widget _expirySection(I18nController t) {
    final theme = AidogTheme.of(context);
    return FormSection(
      title: t.t('platform.expiresAt'),
      desc: t.t('platform.expiresAtHint'),
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                t.t('platform.expiresAtEnable'),
                style: AidogType.label.copyWith(color: theme.c.fg),
              ),
            ),
            AidogSwitch(
              value: c.expiryEnabled,
              compact: true,
              onChanged: () => c.setExpiryEnabled(!c.expiryEnabled),
            ),
          ],
        ),
        if (c.expiryEnabled)
          Row(
            children: [
              Expanded(
                child: DateTimeField(
                  idPrefix: 'expires-at',
                  value: c.expiresAt > 0 ? toDatetimeLocal(c.expiresAt) : '',
                  invalidText: t.t('platform.dateTimeInvalid'),
                  pickTooltip: t.t('platform.pickDateTime'),
                  onChanged: (v) =>
                      c.setExpiresAt(v.isEmpty ? 0 : datetimeLocalToMs(v) ?? 0),
                ),
              ),
              if (c.expiresAt > 0) ...[
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: t.t('platform.expiresAtClear'),
                  onTap: () => c.setExpiresAt(0),
                ),
                const SizedBox(width: AidogSpace.ssm),
                Text(
                  _expiryNote(t),
                  style: AidogType.caption.copyWith(color: theme.c.fg3),
                ),
              ],
            ],
          ),
      ],
    );
  }

  String _expiryNote(I18nController t) {
    final nowMs = DateTime.now().millisecondsSinceEpoch;
    if (nowMs >= c.expiresAt) return t.t('platform.expired');
    final inDay = c.expiresAt - nowMs < 86400000;
    final txt = formatDateTime(c.expiresAt);
    final shown = txt.isEmpty ? '-' : txt;
    return inDay ? t.t('platform.expiresAtSoon', {'time': shown}) : shown;
  }
}

/// `TimeWindow.copyWith` 不区分「不改」与「设成 null」，起止分钟又必须能设 0，
/// 所以这里单开一个只改四个必填时分字段的小工具（`update(idx, {...})` 的 Dart 版）。
TimeWindow update0(
  TimeWindow w, {
  int? startHour,
  int? startMinute,
  int? endHour,
  int? endMinute,
}) => w.copyWith(
  startHour: startHour,
  startMinute: startMinute,
  endHour: endHour,
  endMinute: endMinute,
);

/// 「受影响模型」的自由输入：回车 / **逗号** / 失焦提交成 chip，然后清空自己。
///
/// 逗号那条是票 31 ⑤：React 那边 Enter 与逗号都提交
/// （`formSections.tsx:960-973`）。改造前只认 Enter 和失焦，敲逗号会把逗号
/// 打进模型名，落出一个永远匹配不上的名字。
///
/// [candidates] 是票 31 ④：preset 的 `model_list`，取自已有的
/// `PlatformFormController.modelDropdownSource`，不新开取数。
class _ModelScopeInput extends StatefulWidget {
  const _ModelScopeInput({
    required this.hint,
    required this.onSubmit,
    this.candidates = const [],
  });

  final String hint;
  final ValueChanged<String> onSubmit;
  final List<String> candidates;

  @override
  State<_ModelScopeInput> createState() => _ModelScopeInputState();
}

class _ModelScopeInputState extends State<_ModelScopeInput> {
  final TextEditingController _ctrl = TextEditingController();
  final FocusNode _focus = FocusNode();
  bool _open = false;

  @override
  void initState() {
    super.initState();
    _focus.addListener(() {
      // 聚焦即出候选（票 31 ④：改造前用户根本不知道能填什么）。
      setState(() => _open = _focus.hasFocus);
      if (!_focus.hasFocus) _submit();
    });
  }

  /// 已输入的前缀过滤候选，与模型单元格同一条 `pinyinMatch`。
  List<String> get _filtered {
    final q = _ctrl.text.trim();
    if (q.isEmpty) return widget.candidates;
    return [
      for (final m in widget.candidates)
        if (pinyinMatch(q, m)) m,
    ];
  }

  void _submit([String? explicit]) {
    final v = (explicit ?? _ctrl.text).trim();
    if (v.isEmpty) return;
    widget.onSubmit(v);
    _ctrl.clear();
    setState(() {});
  }

  /// 逗号即提交：把逗号前的那截交出去，逗号本身不进模型名。
  void _onChanged(String v) {
    if (v.contains(',') || v.contains('，')) {
      final parts = v.split(RegExp('[,，]'));
      for (final part in parts.take(parts.length - 1)) {
        if (part.trim().isNotEmpty) widget.onSubmit(part.trim());
      }
      _ctrl.text = parts.last;
      _ctrl.selection = TextSelection.collapsed(offset: _ctrl.text.length);
    }
    setState(() {});
  }

  @override
  void dispose() {
    _focus.dispose();
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final filtered = _filtered;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        TextField(
          key: const ValueKey('model-scope-input'),
          controller: _ctrl,
          focusNode: _focus,
          style: AidogType.label.copyWith(color: theme.c.fg),
          decoration: InputDecoration(
            isDense: true,
            hintText: widget.hint,
            hintStyle: AidogType.label.copyWith(color: theme.c.fg3),
          ),
          onChanged: _onChanged,
          onSubmitted: (_) => _submit(),
        ),
        if (_open && filtered.isNotEmpty)
          Container(
            key: const ValueKey('model-scope-candidates'),
            constraints: const BoxConstraints(maxHeight: 160),
            margin: const EdgeInsets.only(top: 2),
            decoration: BoxDecoration(
              color: theme.c.surface2,
              border: Border.all(color: theme.c.line),
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: ListView(
              shrinkWrap: true,
              padding: const EdgeInsets.all(2),
              children: [
                for (final m in filtered)
                  InkWell(
                    key: ValueKey('model-scope-opt-$m'),
                    onTap: () => _submit(m),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: AidogSpace.ssm,
                        vertical: AidogSpace.sxs,
                      ),
                      child: Text(
                        m,
                        style: AidogType.micro.copyWith(color: theme.c.fg),
                      ),
                    ),
                  ),
              ],
            ),
          ),
      ],
    );
  }
}

/// F15：时段档窗口编辑器（对测试公开：React 的 `WindowsEditModal.test.tsx`
/// 也是直接挂这个组件测的）
///（`WindowsEditModal.tsx` 的页内格子版）。
///
/// 内部持 [_local] 副本，编辑不直接改上层；确认才 `onSave(local)`。
/// [_uiDim] 是**只在 UI 层存在**的维度选中态：数据层「选了周几但一天没勾」与
/// 「每天」同形，纯派生推不出中间态，所以显式维护，且**禁落盘**。
class WindowsEditor extends StatefulWidget {
  const WindowsEditor({
    super.key,
    required this.initial,
    required this.tzMode,
    required this.onTzMode,
    required this.onSave,
    required this.onCancel,
  });

  final List<TimeWindow> initial;
  final TzMode tzMode;
  final ValueChanged<TzMode> onTzMode;
  final ValueChanged<List<TimeWindow>> onSave;
  final VoidCallback onCancel;

  @override
  State<WindowsEditor> createState() => WindowsEditorState();
}

/// 单窗口维度三态（`WindowsEditModal.tsx:26::Dimension`）。
enum WindowDimension { none, week, month }

/// 从数据推维度（`WindowsEditModal.tsx:28::dimensionOf`）。
WindowDimension dimensionOf(TimeWindow w) {
  if (w.daysOfMonth != null && w.daysOfMonth!.isNotEmpty) {
    return WindowDimension.month;
  }
  if (w.daysOfWeek != null && w.daysOfWeek!.isNotEmpty) {
    return WindowDimension.week;
  }
  return WindowDimension.none;
}

class WindowsEditorState extends State<WindowsEditor> {
  late final List<TimeWindow> _local = [
    for (final w in widget.initial) w.clone(),
  ];
  late final List<WindowDimension> _uiDim = [
    for (final w in widget.initial) dimensionOf(w),
  ];

  void _update(int widx, TimeWindow next) =>
      setState(() => _local[widx] = next);

  /// 切维度：清空另一维度字段（互斥），同步更新 UI-only 选中态。
  void _switchDimension(int widx, WindowDimension dim) {
    setState(() {
      final w = _local[widx];
      _local[widx] = switch (dim) {
        WindowDimension.week => w.copyWith(
          daysOfWeek: (w.daysOfWeek != null && w.daysOfWeek!.isNotEmpty)
              ? w.daysOfWeek
              : null,
          clearDaysOfWeek: w.daysOfWeek == null || w.daysOfWeek!.isEmpty,
          clearDaysOfMonth: true,
        ),
        WindowDimension.month => w.copyWith(
          daysOfMonth: (w.daysOfMonth != null && w.daysOfMonth!.isNotEmpty)
              ? w.daysOfMonth
              : null,
          clearDaysOfMonth: w.daysOfMonth == null || w.daysOfMonth!.isEmpty,
          clearDaysOfWeek: true,
        ),
        WindowDimension.none => w.copyWith(
          clearDaysOfWeek: true,
          clearDaysOfMonth: true,
        ),
      };
      _uiDim[widx] = dim;
    });
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    // React 侧是普通 `Dialog`（`WindowsEditModal.tsx:123`，maxWidth 500），点遮罩可关。
    return AidogModal(
      maxWidth: 500,
      onBarrierTap: widget.onCancel,
      child: ModalCard(
        // `DialogContent` 自带 ✕。
        onClose: widget.onCancel,
        // 这个弹窗没挂 glass 类：圆角 `sm:rounded-lg` 16、padding 16、
        // 标题 14 w600（`WindowsEditModal.tsx:123-128`）。
        radius: AidogRadius.lg,
        padding: const EdgeInsets.all(16),
        titleStyle: AidogType.title.copyWith(
          fontSize: 14,
          fontWeight: FontWeight.w600,
        ),
        // `DialogTitle` 自带 `margin 0 0 12px`（`WindowsEditModal.tsx:125`），
        // 不是 `DialogContent` 的缺省 16。
        titleGap: 12,
        title: t.t('platform.windows_edit_title'),
        // tz 切换排在**标题行右端**，且两颗按钮包在一条 `padding 2` 的分段容器里
        //（同上 :125-142）。原先是裸 Row，排在正文第一行。
        titleTrailing: Container(
          padding: const EdgeInsets.all(2),
          decoration: BoxDecoration(
            color: theme.c.surface,
            border: Border.all(color: theme.c.line),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              // 每颗 `padding "2px 8px", fontSize 11, fontWeight 400`（同上 :136）。
              SmallButton(
                label: t.t('platform.timezone_local'),
                fontSize: 11,
                padding: (8, 2),
                fontWeight: FontWeight.w400,
                active: widget.tzMode == TzMode.local,
                onTap: () => widget.onTzMode(TzMode.local),
              ),
              const SizedBox(width: AidogSpace.sxs),
              SmallButton(
                label: t.t('platform.timezone_utc'),
                fontSize: 11,
                padding: (8, 2),
                fontWeight: FontWeight.w400,
                active: widget.tzMode == TzMode.utc,
                onTap: () => widget.onTzMode(TzMode.utc),
              ),
            ],
          ),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 空态 12 + 斜体 + 下距 8（同上 :147）。
            if (_local.isEmpty) ...[
              FormHint(
                t.t('platform.windows_edit_empty'),
                fontSize: 12,
                italic: true,
              ),
              const SizedBox(height: 8),
            ],
            for (var widx = 0; widx < _local.length; widx++)
              _windowCard(t, theme, widx),
            // 「添加窗口」`padding "4px 10px", fontSize 11, marginTop 10`（同上 :327-329）。
            Padding(
              padding: const EdgeInsets.only(top: 10),
              child: Align(
                alignment: AlignmentDirectional.centerStart,
                child: SmallButton(
                  label: '+ ${t.t('platform.windows_edit_add')}',
                  fontSize: 11,
                  padding: (10, 4),
                  onTap: () => setState(() {
                  _local.add(
                    const TimeWindow(
                      startHour: 0,
                      endHour: 24,
                      multiplier: 1.0,
                    ),
                  );
                  _uiDim.add(WindowDimension.none);
                }),
              ),
            ),
            ),
            // 页脚 `marginTop 14, gap 8`，两颗都是默认档按钮（36 高 / px16 / 14），
            // 确认是**实心**（`WindowsEditModal.tsx:336-342`）。
            const SizedBox(height: 14),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.cancel'),
                  fontSize: 14,
                  padding: (16, 8),
                  onTap: widget.onCancel,
                ),
                const SizedBox(width: 8),
                SmallButton(
                  label: t.t('action.confirm'),
                  fontSize: 14,
                  padding: (16, 8),
                  filled: true,
                  onTap: () => widget.onSave([
                    for (final w in _local)
                      // multiplier 不在本编辑器里改；time_windows 的窗口默认 1.0，
                      // 保留传入值兼容旧数据。
                      w.multiplier > 0 ? w : w.copyWith(multiplier: 1.0),
                  ]),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _windowCard(I18nController t, AidogTheme theme, int widx) {
    final w = _local[widx];
    final dim = widx < _uiDim.length ? _uiDim[widx] : dimensionOf(w);
    ({int hour, int minute}) toDisp(int h, int m) => w.timezone != null
        ? (hour: h, minute: m)
        : utcToDisplay(h, m, widget.tzMode);
    ({int hour, int minute}) fromDisp(int h, int m) => w.timezone != null
        ? (hour: h, minute: m)
        : displayToUtc(h, m, widget.tzMode);
    final startDisp = toDisp(w.startHour, w.startMinute ?? 0);
    final endDisp = toDisp(w.endHour, w.endMinute ?? 0);
    final isNextDay = w.endHour < w.startHour;

    // 时分输入是 `.input` 13（`WindowsEditModal.tsx:180`）。
    Widget numBox(int value, ValueChanged<String> onChanged) => SizedBox(
      width: 64,
      child: PlatformField(
        value: '$value',
        fontSize: 13,
        onChanged: onChanged,
      ),
    );

    // 「起」「止」标签 11 secondary，组内 `gap: 4`，时分之间还有一个冒号
    //（`WindowsEditModal.tsx:178-197`）。
    Widget timeGroup(String label, List<Widget> boxes) => Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          label,
          style: AidogType.caption.copyWith(fontSize: 11, color: theme.c.fg2),
        ),
        const SizedBox(width: 4),
        boxes[0],
        const SizedBox(width: 4),
        Text(':', style: TextStyle(fontSize: 11, color: theme.c.fg3)),
        const SizedBox(width: 4),
        boxes[1],
      ],
    );

    // 生效日是 radio 三选一（`WindowsEditModal.tsx:258-277` 的 `RadioGroup`），
    // 三颗描边按钮讲不出互斥语义。圆点 16、项内 `gap: 3`、字 11 secondary。
    Widget radioChoice(String label, bool selected, VoidCallback onTap) =>
        InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(AidogRadius.sm),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                selected ? Icons.radio_button_checked : Icons.radio_button_off,
                size: 16,
                color: selected ? theme.c.accentText : theme.c.fg3,
              ),
              const SizedBox(width: 3),
              Text(
                label,
                style: AidogType.caption.copyWith(
                  fontSize: 11,
                  color: selected ? theme.c.fg : theme.c.fg2,
                ),
              ),
            ],
          ),
        );

    return Container(
      // 卡间距 `gap: 10`、卡内衬 `padding: 10`、底 --bg-glass（= surface）
      //（`WindowsEditModal.tsx:153,160-161`）。
      // 末卡不带下距：列表的 10 是 `gap`，最后一张后面只剩按钮自己的 `marginTop 10`。
      margin: EdgeInsets.only(bottom: widx == _local.length - 1 ? 0 : 10),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: theme.c.surface,
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 起 / 止行：组间 `gap: 8`，删除 × 用 `marginLeft:auto` 贴行尾
          //（`WindowsEditModal.tsx:177,226`）。Wrap 管折行，× 单独靠右。
          Row(
            children: [
              Expanded(
                child: Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    timeGroup(t.t('platform.start_hour'), [
                      numBox(startDisp.hour, (v) {
                        final u = fromDisp(
                          clampInt(v, 0, 23),
                          startDisp.minute,
                        );
                        _update(
                          widx,
                          update0(w, startHour: u.hour, startMinute: u.minute),
                        );
                      }),
                      numBox(startDisp.minute, (v) {
                        final u = fromDisp(startDisp.hour, clampInt(v, 0, 59));
                        _update(
                          widx,
                          update0(w, startHour: u.hour, startMinute: u.minute),
                        );
                      }),
                    ]),
                    timeGroup(t.t('platform.end_hour'), [
                      numBox(endDisp.hour, (v) {
                        final u = fromDisp(clampInt(v, 0, 24), endDisp.minute);
                        _update(
                          widx,
                          update0(w, endHour: u.hour, endMinute: u.minute),
                        );
                      }),
                      numBox(endDisp.minute, (v) {
                        final u = fromDisp(endDisp.hour, clampInt(v, 0, 59));
                        _update(
                          widx,
                          update0(w, endHour: u.hour, endMinute: u.minute),
                        );
                      }),
                    ]),
                    // 「次日」10 + 斜体 tertiary（同上 :219）。
                    if (isNextDay)
                      Text(
                        '（${t.t('platform.peak_next_day')}）',
                        style: AidogType.caption.copyWith(
                          fontSize: 10,
                          fontStyle: FontStyle.italic,
                          color: theme.c.fg3,
                        ),
                      ),
                  ],
                ),
              ),
              // × 是 `padding "2px 6px", fontSize 10`、danger 色（同上 :223-231）。
              SmallButton(
                label: '×',
                danger: true,
                fontSize: 10,
                padding: (6, 2),
                onTap: () => setState(() {
                  _local.removeAt(widx);
                  if (widx < _uiDim.length) _uiDim.removeAt(widx);
                }),
              ),
            ],
          ),
          // 卡内竖向 `gap: 8`（同上 :162）。
          const SizedBox(height: 8),
          // 时区是**横排**：11 的标签 + `gap 4` + 150 宽的 `.input` 下拉（同上 :238-254）。
          Row(
            children: [
              Text(
                t.t('platform.window_timezone'),
                style: AidogType.caption.copyWith(
                  fontSize: 11,
                  color: theme.c.fg2,
                ),
              ),
              const SizedBox(width: 4),
              FormDropdown(
                width: 150,
                boxed: true,
                fontSize: 11,
                value: w.timezone ?? '__utc__',
                options: kWindowTimezones,
                labelOf: (tz) => tz == '__utc__'
                    ? t.t('platform.window_timezone_utc_default')
                    : tz,
                onChanged: (v) => _update(
                  widx,
                  v == '__utc__'
                      ? w.copyWith(clearTimezone: true)
                      : w.copyWith(timezone: v),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          Wrap(
            // 维度行内 `gap: 8`（同上 :261）。
            spacing: 8,
            runSpacing: 8,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              // 「生效日」11 w600 tertiary（同上 :263-265）。
              Text(
                t.t('platform.windows_dimension'),
                style: AidogType.caption.copyWith(
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                  color: theme.c.fg3,
                ),
              ),
              for (final opt in const [
                (WindowDimension.none, 'platform.windows_dim_none'),
                (WindowDimension.week, 'platform.windows_dim_week'),
                (WindowDimension.month, 'platform.windows_dim_month'),
              ])
                KeyedSubtree(
                  key: ValueKey('dim-$widx-${opt.$1.name}'),
                  child: radioChoice(
                    t.t(opt.$2),
                    dim == opt.$1,
                    () => _switchDimension(widx, opt.$1),
                  ),
                ),
            ],
          ),
          if (dim == WindowDimension.week) ...[
            const SizedBox(height: 8),
            WeekdayToggles(
              // 周几按钮 `padding "1px 5px", fontSize 10`（同上 :291）。
              fontSize: 10,
              padding: (5, 1),
              selected: w.daysOfWeek ?? const [],
              tooltipOf: (d) => t.t('platform.weekday_short.$d'),
              onToggle: (day) {
                final cur = w.daysOfWeek ?? const <int>[];
                final next = cur.contains(day)
                    ? [
                        for (final d in cur)
                          if (d != day) d,
                      ]
                    : ([...cur, day]..sort());
                _update(
                  widx,
                  next.isEmpty
                      ? w.copyWith(clearDaysOfWeek: true)
                      : w.copyWith(daysOfWeek: next),
                );
              },
            ),
          ],
          if (dim == WindowDimension.month) ...[
            const SizedBox(height: 8),
            Wrap(
              spacing: 2,
              runSpacing: 2,
              children: [
                for (var dom = 1; dom <= 31; dom++)
                  SmallButton(
                    label: '$dom',
                    // 每月几日 `padding "1px 5px", fontSize 10, minWidth 26`
                    //（`WindowsEditModal.tsx:312`），选中是实心 primary（同上 :310）。
                    fontSize: 10,
                    padding: (5, 1),
                    minWidth: 26,
                    filled: (w.daysOfMonth ?? const []).contains(dom),
                    onTap: () {
                      final cur = w.daysOfMonth ?? const <int>[];
                      final next = cur.contains(dom)
                          ? [
                              for (final d in cur)
                                if (d != dom) d,
                            ]
                          : ([...cur, dom]..sort());
                      _update(
                        widx,
                        next.isEmpty
                            ? w.copyWith(clearDaysOfMonth: true)
                            : w.copyWith(daysOfMonth: next),
                      );
                    },
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

/// 复选框 + 文字标签。对齐 React 表单里那几处 `<Checkbox>` + `<label>`
/// （`formSections.tsx:530-536,760-767`）：选中与否看得见勾，不靠描边色猜。
class _CheckRow extends StatelessWidget {
  const _CheckRow({
    required this.label,
    required this.value,
    required this.onChanged,
    this.bold = false,
  });

  final String label;
  final bool value;
  final ValueChanged<bool> onChanged;
  final bool bold;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    // React 那边勾和字包在同一个 `<label>` 里，点字也切。这里照做。
    return InkWell(
      onTap: () => onChanged(!value),
      borderRadius: BorderRadius.circular(AidogRadius.sm),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Checkbox(
            value: value,
            visualDensity: VisualDensity.compact,
            materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
            // 勾选态：亮底 + 深色勾（同 `MiniBadge(solid:)` 的路子）。
            // accent 当底色时方框本身对卡片只有 1.01:1，看不出勾没勾。
            activeColor: theme.c.accentText,
            checkColor: theme.c.bg,
            onChanged: (v) => onChanged(v == true),
          ),
          const SizedBox(width: AidogSpace.sxs),
          Text(
            label,
            style: AidogType.label.copyWith(
              color: bold ? theme.c.fg : theme.c.fg2,
              fontWeight: bold ? FontWeight.w600 : null,
            ),
          ),
        ],
      ),
    );
  }
}

/// 熔断三项的一行：标签左、输入右，三行靠固定标签宽对齐。
class _BreakerRow extends StatelessWidget {
  const _BreakerRow({
    required this.fieldKey,
    required this.label,
    required this.hint,
    required this.value,
    required this.onChanged,
  });

  final Key fieldKey;
  final String label;
  final String hint;
  final String value;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) => Padding(
    // 行间 10、列间 12、标签 12（`formSections.tsx:568-569`）。
    padding: const EdgeInsets.only(bottom: 10),
    child: Row(
      children: [
        SizedBox(
          width: 110,
          child: Text(
            label,
            style: AidogType.label.copyWith(
              fontSize: 12,
              color: AidogTheme.of(context).c.fg2,
            ),
          ),
        ),
        const SizedBox(width: 12),
        NumberField(
          key: fieldKey,
          width: 140,
          value: value,
          hint: hint,
          min: 0,
          onChanged: onChanged,
        ),
      ],
    ),
  );
}

/// 已选模型的 chip：名字本体不可点，只有内嵌的 ✕ 删。
/// 对齐 `formSections.tsx:931-952` —— 整颗都可点的话误触即删，没有后悔路。
class _ModelChip extends StatelessWidget {
  const _ModelChip({
    required this.label,
    required this.onRemove,
    required this.removeTooltip,
  });

  final String label;
  final VoidCallback onRemove;
  final String removeTooltip;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Container(
      // `.badge badge-muted`：10px、`2px 6px`、gap 4、圆角 6、底 --bg-glass
      //（`globals.css:542-560` + `formSections.tsx:934-935`）。
      padding: const EdgeInsets.only(left: 6, right: 3, top: 2, bottom: 2),
      decoration: BoxDecoration(
        color: theme.c.surface,
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            label,
            style: AidogType.micro.copyWith(
              fontSize: 10,
              letterSpacing: 0,
              color: theme.c.fg2,
            ),
          ),
          const SizedBox(width: 4),
          Tooltip(
            message: removeTooltip,
            child: InkWell(
              onTap: onRemove,
              borderRadius: BorderRadius.circular(AidogRadius.sm),
              child: Padding(
                padding: const EdgeInsets.all(2),
                child: Icon(Icons.close, size: 11, color: theme.c.fg3),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
