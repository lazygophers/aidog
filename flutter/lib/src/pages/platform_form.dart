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

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../utils/formatters.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'platform_defaults.dart';
import 'platform_extra.dart';
import 'platform_form_bits.dart';
import 'platform_form_logic.dart';
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
  final tzLabel = w.timezone ??
      (tzMode == TzMode.local
          ? t.t('platform.timezone_local')
          : t.t('platform.timezone_utc'));
  final isNextDay = w.endHour < w.startHour;
  final nextDayLabel = isNextDay ? '（${t.t('platform.peak_next_day')}）' : '';
  return '$startStr - $endStr（$tzLabel）$nextDayLabel';
}

/// 单窗口的紧凑描述（`ModelsMatrixSection.tsx:40::describeWindow`）。
String describeWindow(TimeWindow w, TzMode tzMode, I18nController t) {
  final isFullDay = w.startHour == 0 &&
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
  const PlatformEditForm({super.key, required this.controller});

  final PlatformFormController controller;

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

  PlatformFormController get c => widget.controller;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        _header(t),
        const SizedBox(height: AidogSpace.smd),
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
        if (c.saveError.isNotEmpty)
          ToastBar(text: c.saveError, ok: false),
      ],
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
        SmallButton(label: '← ${t.t('action.back')}', onTap: c.resetForm),
        const SizedBox(width: AidogSpace.smd),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                editing != null ? editing.name : t.t('platform.add'),
                style: AidogType.title.copyWith(color: theme.c.fg),
              ),
              if (editing != null)
                Text(
                  '${c.protocolLabelMap[editing.platformType] ?? editing.platformType}'
                  ' · ${_primaryBaseUrlOf(editing.platformType)}',
                  style: AidogType.caption.copyWith(color: theme.c.fg3),
                ),
            ],
          ),
        ),
        SmallButton(label: t.t('action.cancel'), onTap: c.resetForm),
        const SizedBox(width: AidogSpace.ssm),
        SmallButton(
          label: saveLabel,
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
              Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: AidogSpace.ssm,
                  vertical: 2,
                ),
                decoration: BoxDecoration(
                  color: theme.c.accentWash,
                  borderRadius: BorderRadius.circular(AidogRadius.sm),
                ),
                child: Text(
                  c.protocolLabelMap[c.protocol] ?? c.protocol,
                  style: AidogType.micro.copyWith(color: theme.c.accentText),
                ),
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
    Widget num1(String label, int value, ValueChanged<int> onChanged) =>
        Padding(
          padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
          child: PlatformField(
            label: label,
            value: '$value',
            onChanged: (v) => onChanged(int.tryParse(v.trim()) ?? 0),
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
            options: [
              for (final v in variants) v.id,
              kQuotaCustomVariant,
            ],
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
      PlatformField(
        label: t.t('platform.devinTimeout'),
        hint: t.t('platform.devinTimeoutPlaceholder'),
        value: c.devinConfig.devinTimeout,
        onChanged: (v) => c.setDevinConfig(c.devinConfig.copyWith(devinTimeout: v)),
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
            maxLines: multiline
                ? c.apiKey.split('\n').length.clamp(2, 6)
                : 1,
            obscure: !multiline && !c.showKey,
            onChanged: c.setApiKey,
          ),
        ),
        if (!multiline) ...[
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
          : SmallButton(
              label: '+ ${t.t('platform.addEndpoint')}',
              onTap: c.addEndpoint,
            ),
      children: [
        if (c.endpoints.isEmpty) FormHint(t.t('platform.noEndpoints')),
        for (var i = 0; i < c.endpoints.length; i++)
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: [
                Row(
                  children: [
                    FormDropdown(
                      width: 150,
                      value: c.endpoints[i].protocol,
                      options: [
                        for (final p in kEndpointProtocols) p.value,
                      ],
                      labelOf: (v) {
                        for (final p in kEndpointProtocols) {
                          if (p.value == v) return p.label;
                        }
                        return v;
                      },
                      onChanged: locked
                          ? null
                          : (v) => c.setEndpointProtocol(i, v),
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
                  ],
                ),
                const SizedBox(height: AidogSpace.sxs),
                Row(
                  children: [
                    Expanded(
                      child: FormDropdown(
                        label: t.t('platform.clientType'),
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
                    // Coding Plan 开关（React 是那个方块 "C" 按钮）。
                    SmallButton(
                      label: 'C',
                      active: c.endpoints[i].codingPlan,
                      onTap: locked ? null : () => c.toggleEndpointCodingPlan(i),
                    ),
                    if (!locked) ...[
                      const SizedBox(width: AidogSpace.sxs),
                      SmallButton(
                        label: t.t('action.delete'),
                        danger: true,
                        onTap: () => c.removeEndpoint(i),
                      ),
                    ],
                  ],
                ),
              ],
            ),
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
    return FormSection(
      title: t.t('platform.batch.previewTitle', {'count': keys.length}),
      desc: t.t('platform.batch.previewHint', {'base': '{base}'}),
      children: [
        for (var i = 0; i < keys.length; i++)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 2),
            child: Row(
              children: [
                SizedBox(
                  width: 28,
                  child: Text(
                    '#${i + 1}',
                    style: AidogType.caption.copyWith(color: theme.c.fg3),
                  ),
                ),
                Expanded(
                  flex: 3,
                  child: Text(
                    i < names.length ? names[i] : '',
                    overflow: TextOverflow.ellipsis,
                    style: AidogType.label.copyWith(color: theme.c.fg),
                  ),
                ),
                Expanded(
                  flex: 2,
                  child: Text(
                    c.protocol.toUpperCase(),
                    overflow: TextOverflow.ellipsis,
                    style: AidogType.micro.copyWith(color: theme.c.fg2),
                  ),
                ),
                Expanded(
                  flex: 3,
                  child: Text(
                    baseUrl.isEmpty ? '—' : baseUrl,
                    overflow: TextOverflow.ellipsis,
                    style: AidogType.caption.copyWith(color: theme.c.fg2),
                  ),
                ),
                Expanded(
                  flex: 2,
                  child: Text(
                    maskTail(keys[i]),
                    overflow: TextOverflow.ellipsis,
                    style: AidogType.numSm.copyWith(color: theme.c.fg3),
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }

  // ── F9 + F15 模型矩阵（默认列 + 时段档列）──────────────────────

  Widget _modelsMatrixSection(I18nController t) {
    final theme = AidogTheme.of(context);
    final rules = c.timeModels;
    final candidates = c.modelDropdownSource;
    const cellW = 200.0;
    const labelW = 64.0;

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
      action: Wrap(
        spacing: AidogSpace.sxs,
        runSpacing: AidogSpace.sxs,
        alignment: WrapAlignment.end,
        children: [
          Tooltip(
            message: t.t('platform.fillAllHint'),
            child: SmallButton(
              label: t.t('platform.fillAll'),
              onTap: c.models['default']!.trim().isEmpty
                  ? null
                  : c.handleFillAll,
            ),
          ),
          SmallButton(
            label: c.fetching
                ? t.t('status.loading')
                : t.t('platform.fetchModels'),
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
              onTap: c.peak.isEmpty
                  ? null
                  : () => setState(() => _importPeakOpen = true),
            ),
          ),
          SmallButton(
            label: '+ ${t.t('platform.time_windows_add_rule')}',
            onTap: () =>
                c.setTimeModels([...rules, const TimeModelRule(windows: [], models: {})]),
          ),
        ],
      ),
      children: [
        if (c.fetchError.isNotEmpty) FormHint(c.fetchError, danger: true),
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              // 列头行
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const SizedBox(width: labelW),
                  SizedBox(
                    width: cellW,
                    child: Center(
                      child: Text(
                        t.t('platform.modelDefault'),
                        style: AidogType.tile.copyWith(color: theme.c.fg2),
                      ),
                    ),
                  ),
                  for (var ri = 0; ri < rules.length; ri++)
                    SizedBox(
                      width: cellW,
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Tooltip(
                            message: t.t('platform.time_windows_edit_windows'),
                            child: SmallButton(
                              label: describeWindows(
                                rules[ri].windows,
                                c.windowsTz,
                                t,
                              ),
                              onTap: () =>
                                  setState(() => _editingRuleIdx = ri),
                            ),
                          ),
                          const SizedBox(height: 2),
                          Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              SmallButton(
                                label: '↑',
                                onTap: ri == 0 ? null : () => _moveRule(ri, -1),
                              ),
                              const SizedBox(width: 2),
                              SmallButton(
                                label: '↓',
                                onTap: ri == rules.length - 1
                                    ? null
                                    : () => _moveRule(ri, 1),
                              ),
                              const SizedBox(width: 2),
                              SmallButton(
                                label: '×',
                                danger: true,
                                onTap: () => _removeRule(ri),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                ],
              ),
              const SizedBox(height: AidogSpace.ssm),
              // 5 槽行
              for (final slot in kModelSlots)
                Padding(
                  padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      SizedBox(
                        width: labelW,
                        child: Text(
                          t.t(slot.labelKey),
                          textAlign: TextAlign.right,
                          style: AidogType.label.copyWith(color: theme.c.fg3),
                        ),
                      ),
                      cell(
                        c.models[slot.key] ?? '',
                        (v) => c.setModel(slot.key, v),
                      ),
                      for (var ri = 0; ri < rules.length; ri++)
                        cell(
                          rules[ri].models[slot.key] ?? '',
                          (v) => _updateRuleModel(ri, slot.key, v),
                        ),
                    ],
                  ),
                ),
              if (rules.isEmpty) FormHint(t.t('platform.time_windows_empty')),
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
          ConfirmCard(
            title: t.t('platform.time_windows_import_confirm_title'),
            body: t.t('platform.time_windows_import_confirm_body', {
              'count': c.peak.length,
            }),
            confirmLabel: t.t('platform.time_windows_import_confirm_button'),
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
              labelOf: (id) =>
                  tiers.firstWhere((x) => x.id == id).name,
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
                update(b.copyWith(kind: kind, windowHours: 7, windowUnit: 'day'));
              } else {
                update(b.copyWith(kind: kind));
              }
            },
          ),
          FormDropdown(
            width: 100,
            value: b.unit,
            options: kManualBudgetUnits,
            labelOf: (v) => switch (v) {
              'usd' => r'$ USD',
              'token' => t.t('platform.manualBudgetUnitToken'),
              _ => t.t('platform.manualBudgetUnitCount'),
            },
            onChanged: (v) => update(b.copyWith(unit: v)),
          ),
          SizedBox(
            width: 100,
            child: PlatformField(
              value: b.amount == 0 ? '' : '${b.amount}',
              hint: t.t('platform.manualBudgetAmount'),
              onChanged: (v) =>
                  update(b.copyWith(amount: double.tryParse(v.trim()) ?? 0)),
            ),
          ),
          if (needsWindow) ...[
            SizedBox(
              width: 80,
              child: PlatformField(
                value: b.windowHours == null ? '' : '${b.windowHours}',
                hint: t.t('platform.manualBudgetWindow'),
                onChanged: (v) => update(
                  v.trim().isEmpty
                      ? b.copyWith(clearWindowHours: true)
                      : b.copyWith(windowHours: double.tryParse(v.trim()) ?? 0),
                ),
              ),
            ),
            FormDropdown(
              width: 100,
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
          SmallButton(
            label: t.t('platform.manualBudgetEnabled'),
            active: b.enabled,
            onTap: () => update(b.copyWith(enabled: !b.enabled)),
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
      children: [
        PlatformField(
          label: t.t('platform.breakerFailureThreshold'),
          hint: hintOf(d?.failureThreshold),
          value: c.breakerFailureThreshold,
          onChanged: c.setBreakerFailureThreshold,
        ),
        PlatformField(
          label: t.t('platform.breakerOpenSecs'),
          hint: hintOf(d?.openSecs),
          value: c.breakerOpenSecs,
          onChanged: c.setBreakerOpenSecs,
        ),
        PlatformField(
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
    final nowPeak = isCurrentlyPeak(c.peak, DateTime.now().millisecondsSinceEpoch);
    return FormSection(
      title: t.t('platform.peak'),
      desc: t.t('platform.peak_desc'),
      action: Wrap(
        spacing: AidogSpace.sxs,
        runSpacing: AidogSpace.sxs,
        alignment: WrapAlignment.end,
        children: [
          SmallButton(
            label: t.t('platform.timezone_local'),
            active: c.windowsTz == TzMode.local,
            onTap: () => c.setWindowsTz(TzMode.local),
          ),
          SmallButton(
            label: t.t('platform.timezone_utc'),
            active: c.windowsTz == TzMode.utc,
            onTap: () => c.setWindowsTz(TzMode.utc),
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
        // 高峰禁用开关 + 实时态。
        Row(
          children: [
            SmallButton(
              label: t.t('platform.disable_during_peak'),
              active: c.disableDuringPeak,
              onTap: () => c.setDisableDuringPeak(!c.disableDuringPeak),
            ),
            if (c.disableDuringPeak) ...[
              const SizedBox(width: AidogSpace.ssm),
              Text(
                nowPeak
                    ? t.t('platform.currently_peak')
                    : t.t('platform.currently_off_peak'),
                style: AidogType.micro.copyWith(
                  color: nowPeak ? theme.c.bad : theme.c.fg3,
                ),
              ),
            ],
          ],
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

    Widget numBox(double width, int value, ValueChanged<String> onChanged) =>
        SizedBox(
          width: width,
          child: PlatformField(value: '$value', onChanged: onChanged),
        );

    return Container(
      margin: const EdgeInsets.only(bottom: AidogSpace.ssm),
      padding: const EdgeInsets.all(AidogSpace.ssm),
      decoration: BoxDecoration(
        color: theme.c.surface2,
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
              numBox(64, startDisp.hour, (v) {
                final u = fromDisp(clampInt(v, 0, 23), startDisp.minute);
                update(update0(w, startHour: u.hour, startMinute: u.minute));
              }),
              numBox(64, startDisp.minute, (v) {
                final u = fromDisp(startDisp.hour, clampInt(v, 0, 59));
                update(update0(w, startHour: u.hour, startMinute: u.minute));
              }),
              Text(
                t.t('platform.end_hour'),
                style: AidogType.label.copyWith(color: theme.c.fg2),
              ),
              numBox(64, endDisp.hour, (v) {
                final u = fromDisp(clampInt(v, 0, 24), endDisp.minute);
                update(update0(w, endHour: u.hour, endMinute: u.minute));
              }),
              numBox(64, endDisp.minute, (v) {
                final u = fromDisp(endDisp.hour, clampInt(v, 0, 59));
                update(update0(w, endHour: u.hour, endMinute: u.minute));
              }),
              Text(
                t.t('platform.multiplier'),
                style: AidogType.label.copyWith(color: theme.c.fg2),
              ),
              SizedBox(
                width: 84,
                child: PlatformField(
                  value: '${w.multiplier}',
                  onChanged: (v) => update(
                    w.copyWith(multiplier: double.tryParse(v.trim()) ?? 1),
                  ),
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
              for (var mi = 0; mi < (w.models?.length ?? 0); mi++)
                SmallButton(
                  label: '${w.models![mi]} ✕',
                  onTap: () {
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
                width: 200,
                child: PlatformField(
                  label: t.t('platform.peak_start_at'),
                  value: secToLocalInput(w.startAt),
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
                width: 200,
                child: PlatformField(
                  label: t.t('platform.peak_end_at'),
                  value: secToLocalInput(w.endAt),
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
              Switch(
                value: c.autoGroup,
                onChanged: c.setAutoGroup,
                activeThumbColor: theme.c.accent,
                inactiveTrackColor: theme.c.surface2,
                inactiveThumbColor: theme.c.fg3,
              ),
            ],
          ),
        if (locked == null && gds.isNotEmpty) ...[
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('platform.groupAssignJoin')),
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
            Switch(
              value: c.expiryEnabled,
              onChanged: c.setExpiryEnabled,
              activeThumbColor: theme.c.accent,
              inactiveTrackColor: theme.c.surface2,
              inactiveThumbColor: theme.c.fg3,
            ),
          ],
        ),
        if (c.expiryEnabled)
          Row(
            children: [
              Expanded(
                child: PlatformField(
                  value: c.expiresAt > 0 ? toDatetimeLocal(c.expiresAt) : '',
                  hint: 'YYYY-MM-DDTHH:MM',
                  onChanged: (v) {
                    if (v.trim().isEmpty) {
                      c.setExpiresAt(0);
                      return;
                    }
                    final ms = datetimeLocalToMs(v);
                    if (ms != null) c.setExpiresAt(ms);
                  },
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
    return inDay
        ? t.t('platform.expiresAtSoon', {'time': shown})
        : shown;
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

/// 「受影响模型」的自由输入：回车 / 失焦提交成 chip，然后清空自己。
class _ModelScopeInput extends StatefulWidget {
  const _ModelScopeInput({required this.hint, required this.onSubmit});

  final String hint;
  final ValueChanged<String> onSubmit;

  @override
  State<_ModelScopeInput> createState() => _ModelScopeInputState();
}

class _ModelScopeInputState extends State<_ModelScopeInput> {
  final TextEditingController _ctrl = TextEditingController();
  final FocusNode _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() {
      if (!_focus.hasFocus) _submit();
    });
  }

  void _submit() {
    final v = _ctrl.text.trim();
    if (v.isEmpty) return;
    widget.onSubmit(v);
    _ctrl.clear();
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
    return TextField(
      controller: _ctrl,
      focusNode: _focus,
      style: AidogType.label.copyWith(color: theme.c.fg),
      decoration: InputDecoration(
        isDense: true,
        hintText: widget.hint,
        hintStyle: AidogType.label.copyWith(color: theme.c.fg3),
      ),
      onSubmitted: (_) => _submit(),
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
  late final List<TimeWindow> _local = [for (final w in widget.initial) w.clone()];
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
        WindowDimension.none =>
          w.copyWith(clearDaysOfWeek: true, clearDaysOfMonth: true),
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
      child: Tile(
        title: t.t('platform.windows_edit_title'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                SmallButton(
                  label: t.t('platform.timezone_local'),
                  active: widget.tzMode == TzMode.local,
                  onTap: () => widget.onTzMode(TzMode.local),
                ),
                const SizedBox(width: AidogSpace.sxs),
                SmallButton(
                  label: t.t('platform.timezone_utc'),
                  active: widget.tzMode == TzMode.utc,
                  onTap: () => widget.onTzMode(TzMode.utc),
                ),
              ],
            ),
            if (_local.isEmpty) FormHint(t.t('platform.windows_edit_empty')),
            for (var widx = 0; widx < _local.length; widx++)
              _windowCard(t, theme, widx),
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: SmallButton(
                label: '+ ${t.t('platform.windows_edit_add')}',
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
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.cancel'),
                  onTap: widget.onCancel,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: t.t('action.confirm'),
                  onTap: () => widget.onSave([
                    for (final w in _local)
                      // multiplier 不在本编辑器里改；time_windows 的窗口默认 1.0，
                      // 保留传入值兼容旧数据。
                      w.multiplier > 0
                          ? w
                          : w.copyWith(multiplier: 1.0),
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

    Widget numBox(int value, ValueChanged<String> onChanged) => SizedBox(
      width: 64,
      child: PlatformField(value: '$value', onChanged: onChanged),
    );

    return Container(
      margin: const EdgeInsets.only(bottom: AidogSpace.ssm),
      padding: const EdgeInsets.all(AidogSpace.ssm),
      decoration: BoxDecoration(
        color: theme.c.surface2,
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
                style: AidogType.caption.copyWith(color: theme.c.fg2),
              ),
              numBox(startDisp.hour, (v) {
                final u = fromDisp(clampInt(v, 0, 23), startDisp.minute);
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
              Text(
                t.t('platform.end_hour'),
                style: AidogType.caption.copyWith(color: theme.c.fg2),
              ),
              numBox(endDisp.hour, (v) {
                final u = fromDisp(clampInt(v, 0, 24), endDisp.minute);
                _update(widx, update0(w, endHour: u.hour, endMinute: u.minute));
              }),
              numBox(endDisp.minute, (v) {
                final u = fromDisp(endDisp.hour, clampInt(v, 0, 59));
                _update(widx, update0(w, endHour: u.hour, endMinute: u.minute));
              }),
              if (isNextDay)
                Text(
                  '（${t.t('platform.peak_next_day')}）',
                  style: AidogType.caption.copyWith(color: theme.c.fg3),
                ),
              SmallButton(
                label: '×',
                danger: true,
                onTap: () => setState(() {
                  _local.removeAt(widx);
                  if (widx < _uiDim.length) _uiDim.removeAt(widx);
                }),
              ),
            ],
          ),
          const SizedBox(height: AidogSpace.sxs),
          FormDropdown(
            label: t.t('platform.window_timezone'),
            width: 160,
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
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Text(
                t.t('platform.windows_dimension'),
                style: AidogType.caption.copyWith(color: theme.c.fg3),
              ),
              for (final opt in const [
                (WindowDimension.none, 'platform.windows_dim_none'),
                (WindowDimension.week, 'platform.windows_dim_week'),
                (WindowDimension.month, 'platform.windows_dim_month'),
              ])
                SmallButton(
                  key: ValueKey('dim-$widx-${opt.$1.name}'),
                  label: t.t(opt.$2),
                  active: dim == opt.$1,
                  onTap: () => _switchDimension(widx, opt.$1),
                ),
            ],
          ),
          if (dim == WindowDimension.week) ...[
            const SizedBox(height: AidogSpace.sxs),
            WeekdayToggles(
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
            const SizedBox(height: AidogSpace.sxs),
            Wrap(
              spacing: 2,
              runSpacing: 2,
              children: [
                for (var dom = 1; dom <= 31; dom++)
                  SmallButton(
                    label: '$dom',
                    active: (w.daysOfMonth ?? const []).contains(dom),
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
