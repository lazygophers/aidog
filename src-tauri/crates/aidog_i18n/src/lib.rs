/// Proxy 错误消息多语言支持。
/// 从 DB 读取 app locale，返回对应语言的错误消息。
/// 支持的语言标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    ZhCn,
    #[default]
    EnUs,
    JaJp,
    FrFr,
    DeDe,
    RuRu,
    ArSa,
    EsEs,
}

impl Lang {
    /// 从 DB 存储的 locale 字符串解析
    pub fn from_locale(locale: &str) -> Self {
        match locale.trim().to_lowercase().as_str() {
            "zh-cn" | "zh_cn" | "zh-hans" | "zh_hans" | "zh" => Self::ZhCn,
            "ja-jp" | "ja_jp" | "ja" => Self::JaJp,
            "fr-fr" | "fr_fr" | "fr" => Self::FrFr,
            "de-de" | "de_de" | "de" => Self::DeDe,
            "ru-ru" | "ru_ru" | "ru" => Self::RuRu,
            "ar-sa" | "ar_sa" | "ar" => Self::ArSa,
            "es-es" | "es_es" | "es" => Self::EsEs,
            _ => Self::EnUs,
        }
    }

    /// 规范 locale 标识（8 语言 locale JSON 与 registry `platform.json` 的 `name` 键同一套）。
    pub fn locale_key(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-Hans",
            Self::EnUs => "en-US",
            Self::JaJp => "ja-JP",
            Self::FrFr => "fr-FR",
            Self::DeDe => "de-DE",
            Self::RuRu => "ru-RU",
            Self::ArSa => "ar-SA",
            Self::EsEs => "es-ES",
        }
    }
}

// ── 错误消息键 ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum ErrorKey {
    /// 读取请求体失败
    ReadBody,
    /// 无匹配分组
    NoMatchingGroup,
    /// 解析 JSON 失败
    ParseJson,
    /// 解析请求失败
    ParseRequest,
    /// 路由错误
    Route,
    /// 手动预算耗尽
    BudgetExhausted,
    /// 预算每日重置提示
    BudgetResetDaily,
    /// 预算滚动窗口重置提示
    BudgetResetRolling,
    /// 预算固定窗口重置提示
    BudgetResetFixed,
    /// 预算总额耗尽提示
    BudgetResetTotal,
    /// 上游错误
    Upstream,
    /// 中间件入站拦截
    MiddlewareBlocked,
    /// 代理启动失败：端口被占用（`{{port}}` 占位）
    ProxyStartPortInUse,
    /// 代理启动失败：其它绑定错误（`{{port}}` 占位）
    ProxyStartOther,
    /// 首次关窗：仍在托盘运行
    TrayStillRunning,
}

/// 返回翻译后的错误消息
pub fn t(lang: Lang, key: ErrorKey) -> &'static str {
    match lang {
        Lang::ZhCn => t_zh(key),
        Lang::EnUs => t_en(key),
        Lang::JaJp => t_ja(key),
        Lang::FrFr => t_fr(key),
        Lang::DeDe => t_de(key),
        Lang::RuRu => t_ru(key),
        Lang::ArSa => t_ar(key),
        Lang::EsEs => t_es(key),
    }
}

fn t_en(key: ErrorKey) -> &'static str {
    match key {
        ErrorKey::ReadBody => "read body",
        ErrorKey::NoMatchingGroup => "no matching group",
        ErrorKey::ParseJson => "parse json",
        ErrorKey::ParseRequest => "failed to parse request",
        ErrorKey::Route => "route",
        ErrorKey::BudgetExhausted => "Manual budget exhausted",
        ErrorKey::BudgetResetDaily => "Budget will reset at local midnight.",
        ErrorKey::BudgetResetRolling => "Budget will reset after the rolling window elapses.",
        ErrorKey::BudgetResetFixed => "Budget will reset at the next fixed window boundary.",
        ErrorKey::BudgetResetTotal => {
            "Total budget exhausted; increase or reset the limit to resume."
        }
        ErrorKey::Upstream => "upstream",
        ErrorKey::MiddlewareBlocked => "Request blocked by a middleware rule",
        ErrorKey::ProxyStartPortInUse => "Port {{port}} is already in use, proxy did not start",
        ErrorKey::ProxyStartOther => "Failed to bind port {{port}}, proxy did not start",
        ErrorKey::TrayStillRunning => "AiDog is still running in the tray",
    }
}

fn t_zh(key: ErrorKey) -> &'static str {
    match key {
        ErrorKey::ReadBody => "读取请求体",
        ErrorKey::NoMatchingGroup => "未匹配到分组",
        ErrorKey::ParseJson => "JSON 解析失败",
        ErrorKey::ParseRequest => "请求解析失败",
        ErrorKey::Route => "路由",
        ErrorKey::BudgetExhausted => "手动预算已耗尽",
        ErrorKey::BudgetResetDaily => "预算将在本地午夜重置。",
        ErrorKey::BudgetResetRolling => "预算将在滚动窗口到期后重置。",
        ErrorKey::BudgetResetFixed => "预算将在下一个固定窗口边界重置。",
        ErrorKey::BudgetResetTotal => "总预算已耗尽，请增加或重置限额以恢复。",
        ErrorKey::Upstream => "上游",
        ErrorKey::MiddlewareBlocked => "请求被中间件规则拦截",
        ErrorKey::ProxyStartPortInUse => "端口 {{port}} 被占用，代理未启动",
        ErrorKey::ProxyStartOther => "端口 {{port}} 绑定失败，代理未启动",
        ErrorKey::TrayStillRunning => "aidog 仍在托盘运行",
    }
}

fn t_ja(key: ErrorKey) -> &'static str {
    match key {
        ErrorKey::ReadBody => "リクエストボディの読み取り",
        ErrorKey::NoMatchingGroup => "一致するグループがありません",
        ErrorKey::ParseJson => "JSON解析エラー",
        ErrorKey::ParseRequest => "リクエストの解析に失敗",
        ErrorKey::Route => "ルーティング",
        ErrorKey::BudgetExhausted => "手動予算を使い切りました",
        ErrorKey::BudgetResetDaily => "予算は現地の深夜にリセットされます。",
        ErrorKey::BudgetResetRolling => "予算はローリングウィンドウ経過後にリセットされます。",
        ErrorKey::BudgetResetFixed => "予算は次の固定ウィンドウ境界でリセットされます。",
        ErrorKey::BudgetResetTotal => {
            "総予算を使い切りました。制限を増やすかリセットしてください。"
        }
        ErrorKey::Upstream => "アップストリーム",
        ErrorKey::MiddlewareBlocked => "リクエストはミドルウェアルールによってブロックされました",
        ErrorKey::ProxyStartPortInUse => "ポート {{port}} は使用中のため、プロキシは起動しませんでした",
        ErrorKey::ProxyStartOther => "ポート {{port}} のバインドに失敗したため、プロキシは起動しませんでした",
        ErrorKey::TrayStillRunning => "aidog はトレイで実行中です",
    }
}

fn t_fr(key: ErrorKey) -> &'static str {
    match key {
        ErrorKey::ReadBody => "lecture du corps",
        ErrorKey::NoMatchingGroup => "aucun groupe correspondant",
        ErrorKey::ParseJson => "erreur d'analyse JSON",
        ErrorKey::ParseRequest => "échec de l'analyse de la requête",
        ErrorKey::Route => "routage",
        ErrorKey::BudgetExhausted => "Budget manuel épuisé",
        ErrorKey::BudgetResetDaily => "Le budget sera réinitialisé à minuit.",
        ErrorKey::BudgetResetRolling => "Le budget sera réinitialisé après la fenêtre glissante.",
        ErrorKey::BudgetResetFixed => {
            "Le budget sera réinitialisé à la prochaine limite de fenêtre fixe."
        }
        ErrorKey::BudgetResetTotal => "Budget total épuisé ; augmentez ou réinitialisez la limite.",
        ErrorKey::Upstream => "amont",
        ErrorKey::MiddlewareBlocked => "Requête bloquée par une règle de middleware",
        ErrorKey::ProxyStartPortInUse => "Le port {{port}} est déjà utilisé, le proxy n'a pas démarré",
        ErrorKey::ProxyStartOther => "Échec de la liaison au port {{port}}, le proxy n'a pas démarré",
        ErrorKey::TrayStillRunning => "aidog continue de s'exécuter dans la barre d'état",
    }
}

fn t_de(key: ErrorKey) -> &'static str {
    match key {
        ErrorKey::ReadBody => "Body lesen",
        ErrorKey::NoMatchingGroup => "keine passende Gruppe",
        ErrorKey::ParseJson => "JSON-Parse-Fehler",
        ErrorKey::ParseRequest => "Anfrage konnte nicht geparst werden",
        ErrorKey::Route => "Routing",
        ErrorKey::BudgetExhausted => "Manuelles Budget aufgebraucht",
        ErrorKey::BudgetResetDaily => "Das Budget wird um Mitternacht zurückgesetzt.",
        ErrorKey::BudgetResetRolling => {
            "Das Budget wird nach Ablauf des rollierenden Fensters zurückgesetzt."
        }
        ErrorKey::BudgetResetFixed => {
            "Das Budget wird an der nächsten festen Fenstergrenze zurückgesetzt."
        }
        ErrorKey::BudgetResetTotal => {
            "Gesamtbudget aufgebraucht; erhöhen oder setzen Sie das Limit zurück."
        }
        ErrorKey::Upstream => "Upstream",
        ErrorKey::MiddlewareBlocked => "Anfrage durch eine Middleware-Regel blockiert",
        ErrorKey::ProxyStartPortInUse => "Port {{port}} wird bereits verwendet, Proxy wurde nicht gestartet",
        ErrorKey::ProxyStartOther => "Bindung an Port {{port}} fehlgeschlagen, Proxy wurde nicht gestartet",
        ErrorKey::TrayStillRunning => "aidog läuft weiterhin im Tray",
    }
}

fn t_ru(key: ErrorKey) -> &'static str {
    match key {
        ErrorKey::ReadBody => "чтение тела запроса",
        ErrorKey::NoMatchingGroup => "нет подходящей группы",
        ErrorKey::ParseJson => "ошибка парсинга JSON",
        ErrorKey::ParseRequest => "не удалось разобрать запрос",
        ErrorKey::Route => "маршрутизация",
        ErrorKey::BudgetExhausted => "Ручной бюджет исчерпан",
        ErrorKey::BudgetResetDaily => "Бюджет будет сброшен в полночь.",
        ErrorKey::BudgetResetRolling => "Бюджет будет сброшен после окончания скользящего окна.",
        ErrorKey::BudgetResetFixed => {
            "Бюджет будет сброшен на следующей границе фиксированного окна."
        }
        ErrorKey::BudgetResetTotal => "Общий бюджет исчерпан; увеличьте или сбросьте лимит.",
        ErrorKey::Upstream => "вышестоящий",
        ErrorKey::MiddlewareBlocked => "Запрос заблокирован правилом промежуточного слоя",
        ErrorKey::ProxyStartPortInUse => "Порт {{port}} уже занят, прокси не запущен",
        ErrorKey::ProxyStartOther => "Не удалось привязаться к порту {{port}}, прокси не запущен",
        ErrorKey::TrayStillRunning => "aidog продолжает работать в трее",
    }
}

fn t_ar(key: ErrorKey) -> &'static str {
    match key {
        ErrorKey::ReadBody => "قراءة نص الطلب",
        ErrorKey::NoMatchingGroup => "لا توجد مجموعة مطابقة",
        ErrorKey::ParseJson => "خطأ في تحليل JSON",
        ErrorKey::ParseRequest => "فشل تحليل الطلب",
        ErrorKey::Route => "التوجيه",
        ErrorKey::BudgetExhausted => "تم استنفاد الميزانية اليدوية",
        ErrorKey::BudgetResetDaily => "سيتم إعادة تعيين الميزانية في منتصف الليل.",
        ErrorKey::BudgetResetRolling => "سيتم إعادة تعيين الميزانية بعد انتهاء النافذة المتدحرجة.",
        ErrorKey::BudgetResetFixed => {
            "سيتم إعادة تعيين الميزانية عند حدود النافذة الثابتة التالية."
        }
        ErrorKey::BudgetResetTotal => {
            "تم استنفاد الميزانية الإجمالية؛ قم بزيادة الحد أو إعادة تعيينه."
        }
        ErrorKey::Upstream => "المنبع",
        ErrorKey::MiddlewareBlocked => "تم حظر الطلب بواسطة قاعدة الوسيط",
        ErrorKey::ProxyStartPortInUse => "المنفذ {{port}} مستخدم بالفعل، لم يبدأ الوكيل",
        ErrorKey::ProxyStartOther => "فشل ربط المنفذ {{port}}، لم يبدأ الوكيل",
        ErrorKey::TrayStillRunning => "aidog لا يزال يعمل في شريط النظام",
    }
}

fn t_es(key: ErrorKey) -> &'static str {
    match key {
        ErrorKey::ReadBody => "lectura del cuerpo de la solicitud",
        ErrorKey::NoMatchingGroup => "ningún grupo coincidente",
        ErrorKey::ParseJson => "error al analizar JSON",
        ErrorKey::ParseRequest => "error al analizar la solicitud",
        ErrorKey::Route => "enrutamiento",
        ErrorKey::BudgetExhausted => "Presupuesto manual agotado",
        ErrorKey::BudgetResetDaily => "El presupuesto se restablecerá a medianoche local.",
        ErrorKey::BudgetResetRolling => {
            "El presupuesto se restablecerá tras finalizar la ventana móvil."
        }
        ErrorKey::BudgetResetFixed => {
            "El presupuesto se restablecerá en el próximo límite de ventana fija."
        }
        ErrorKey::BudgetResetTotal => {
            "Presupuesto total agotado; aumente o restablezca el límite para continuar."
        }
        ErrorKey::Upstream => "origen",
        ErrorKey::MiddlewareBlocked => "Solicitud bloqueada por una regla de middleware",
        ErrorKey::ProxyStartPortInUse => "El puerto {{port}} ya está en uso, el proxy no se inició",
        ErrorKey::ProxyStartOther => "No se pudo vincular al puerto {{port}}, el proxy no se inició",
        ErrorKey::TrayStillRunning => "aidog sigue ejecutándose en la bandeja",
    }
}

#[cfg(test)]
#[path = "test_i18n.rs"]
mod test_i18n;
