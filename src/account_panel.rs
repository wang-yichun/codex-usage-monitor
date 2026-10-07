use std::sync::Mutex;
use std::time::SystemTime;
use serde_json::{json, Value};
use crate::localization::LanguageId;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Row {
    pub title: String,
    pub value: String,
    pub section: bool,
    #[serde(default)]
    pub column: u8,
    #[serde(default)]
    pub card: bool,
    #[serde(default)]
    pub badge: String,
    #[serde(default)]
    pub badge_tone: u8,
    #[serde(default)]
    pub refresh_line: Option<usize>,
}

#[derive(Default)]
struct Snapshot {
    usage: Option<Value>,
    cards: Option<Value>,
    updated: Option<SystemTime>,
}
static ACCOUNT: Mutex<Snapshot> = Mutex::new(Snapshot { usage: None, cards: None, updated: None });

// Retain only display metadata, never identity, credentials or profile fields.
pub fn update_usage(raw: &Value) {
    let mut selected = serde_json::Map::new();
    for key in ["plan_type", "rate_limit", "credits", "model_usage", "additional_rate_limits", "code_review_rate_limit", "chatpass", "spend_control"] {
        if let Some(value) = raw.get(key) { selected.insert(key.into(), value.clone()); }
    }
    let mut snapshot = ACCOUNT.lock().unwrap();
    snapshot.usage = Some(Value::Object(selected));
    snapshot.updated = Some(SystemTime::now());
}

pub fn update_cards(raw: Option<&Value>) {
    let sanitized = raw.map(|raw| {
        let cards: Vec<Value> = raw["credits"].as_array().into_iter().flatten().map(|card| {
            let mut selected = serde_json::Map::new();
            for key in ["reset_type", "is_supported_by_plan", "status", "granted_at", "expires_at", "redeem_started_at", "redeemed_at"] {
                if let Some(value) = card.get(key) { selected.insert(key.into(), value.clone()); }
            }
            Value::Object(selected)
        }).collect();
        json!({ "credits": cards, "available_count": raw["available_count"],
            "total_earned_count": raw["total_earned_count"],
            "immediate_reset_purchase_eligible": raw["immediate_reset_purchase_eligible"],
            "history_enabled": raw["history_enabled"] })
    });
    ACCOUNT.lock().unwrap().cards = sanitized;
}

#[derive(Clone, Copy)]
pub enum Key {
    #[allow(dead_code)]
    Title, Local, Input, Cached, Output, Reasoning, Total, Sessions, Unreadable,
    Account, Plan, Allowed, Exhausted, Credits, Balance, Unlimited, Overage,
    Models, Available, EnableCredits, Extra, Review, Chatpass, Window, Reset,
    Spend, Reached, Limit, Cards, Count, Earned, Purchase, History, Card, Type,
    #[allow(dead_code)]
    Status, Granted, Expires, RedeemStart, Redeemed, Supported, Yes, No, Absent,
    Unknown, Updated, NoData, AvailableStatus, Redeeming, RedeemedStatus, Expired, TimeZone, Countdown,
}

pub fn label(language: LanguageId, key: Key) -> &'static str {
    use LanguageId::*;
    if matches!(key, Key::TimeZone) {
        return match language {
            English => "All times Beijing time (UTC+8)",
            SimplifiedChinese => "时间均为北京时间（UTC+8）",
            TraditionalChinese => "時間均為北京時間（UTC+8）",
            Dutch => "Alle tijden in Beijing-tijd (UTC+8)",
            Spanish => "Todas las horas en hora de Pekín (UTC+8)",
            French => "Toutes les heures en heure de Pékin (UTC+8)",
            German => "Alle Zeiten in Peking-Zeit (UTC+8)",
            Japanese => "時刻は北京時間（UTC+8）",
            Korean => "모든 시간은 베이징 시간 (UTC+8)",
            Russian => "Время указано по Пекину (UTC+8)",
            PortugueseBrazil => "Todos os horários no fuso de Pequim (UTC+8)",
        };
    }
    if matches!(key, Key::Countdown) {
        return match language {
            English => "Countdown",
            SimplifiedChinese => "倒计时",
            TraditionalChinese => "倒數計時",
            Dutch => "Aftellen",
            Spanish => "Cuenta atrás",
            French => "Compte à rebours",
            German => "Countdown",
            Japanese => "カウントダウン",
            Korean => "카운트다운",
            Russian => "Обратный отсчёт",
            PortugueseBrazil => "Contagem regressiva",
        };
    }
    let labels = match language {
        English => "Codex overview|Local sessions started today · Tokens|Input|Cached input (included)|Output|Reasoning output (included)|Total|Sessions|Unreadable logs|Account|Plan|Usage allowed|Quota exhausted|Extra credits|Balance|Unlimited|Overage limit reached|Model availability|Available|Credits can enable|Additional quota windows|Code review quota|Chatpass quota|Window|Resets at|Spend control|Limit reached|Individual limit|Reset cards|Available count|Total earned|Purchase eligible|History enabled|Card|Reset type|Status|Granted at|Expires at|Redemption started|Redeemed at|Supported by plan|Yes|No|None|Unknown|Last account update|No data|Available|Redeeming|Redeemed|Expired|All times Beijing time (UTC+8)|Countdown",
        SimplifiedChinese => "Codex 信息概览|本机今日开始的会话 · Token|输入|其中缓存输入|输出|其中推理输出|总计|会话数|未能读取的日志|账户|套餐类型|允许使用额度|额度已耗尽|额外 credits|余额|无限额度|超额额度已耗尽|模型可用状态|可用|可用 credits 解锁|额外额度窗口|代码审查额度|Chatpass 额度|时间窗口|重置时间|消费限制|已触发限制|个人消费上限|重置卡|可用数量|累计获得|可购买|启用历史记录|卡片|重置类型|状态|发放时间|到期时间|开始兑换时间|兑换完成时间|套餐支持|是|否|无|未知|账户数据更新时间|暂无数据|可用|兑换中|已兑换|已过期|时间均为北京时间（UTC+8）|倒计时",
        TraditionalChinese => "Codex 資訊概覽|本機今日開始的對話 · Token|輸入|其中快取輸入|輸出|其中推理輸出|合計|對話數|無法讀取的記錄|帳戶|方案類型|允許使用額度|額度已耗盡|額外 credits|餘額|無限額度|超額額度已耗盡|模型可用狀態|可用|可用 credits 解鎖|額外額度視窗|程式碼審查額度|Chatpass 額度|時間視窗|重設時間|消費限制|已觸發限制|個人消費上限|重設卡|可用數量|累計獲得|可購買|啟用歷史記錄|卡片|重設類型|狀態|發放時間|到期時間|開始兌換時間|兌換完成時間|方案支援|是|否|無|未知|帳戶資料更新時間|暫無資料|可用|兌換中|已兌換|已過期|時間均為北京時間（UTC+8）|倒數計時",
        Dutch => "Codex-overzicht|Lokale sessies gestart vandaag · Tokens|Invoer|Gecachte invoer (inbegrepen)|Uitvoer|Redeneeruitvoer (inbegrepen)|Totaal|Sessies|Onleesbare logboeken|Account|Abonnement|Gebruik toegestaan|Quotum opgebruikt|Extra credits|Saldo|Onbeperkt|Overschrijdingslimiet bereikt|Modelbeschikbaarheid|Beschikbaar|Credits kunnen activeren|Extra quotumvensters|Quotum voor codereview|Chatpass-quotum|Tijdvenster|Reset op|Bestedingslimiet|Limiet bereikt|Persoonlijke limiet|Resetkaarten|Beschikbaar aantal|Totaal verdiend|Aankoop mogelijk|Geschiedenis ingeschakeld|Kaart|Resettype|Status|Toegekend op|Verloopt op|Inwisseling gestart|Ingewisseld op|Ondersteund door abonnement|Ja|Nee|Geen|Onbekend|Laatste accountupdate|Geen gegevens|Beschikbaar|Wordt ingewisseld|Ingewisseld|Verlopen|Aftellen",
        Spanish => "Resumen de Codex|Sesiones locales iniciadas hoy · Tokens|Entrada|Entrada en caché (incluida)|Salida|Salida de razonamiento (incluida)|Total|Sesiones|Registros ilegibles|Cuenta|Plan|Uso permitido|Cuota agotada|Créditos adicionales|Saldo|Ilimitado|Límite de excedente alcanzado|Disponibilidad de modelos|Disponible|Los créditos pueden habilitar|Ventanas de cuota adicionales|Cuota de revisión de código|Cuota de Chatpass|Ventana|Se restablece|Control de gasto|Límite alcanzado|Límite individual|Tarjetas de reinicio|Cantidad disponible|Total obtenido|Compra disponible|Historial habilitado|Tarjeta|Tipo de reinicio|Estado|Fecha de concesión|Fecha de vencimiento|Inicio del canje|Fecha del canje|Compatible con el plan|Sí|No|Ninguno|Desconocido|Última actualización de cuenta|Sin datos|Disponible|Canje en curso|Canjeado|Vencido|Cuenta atrás",
        French => "Aperçu de Codex|Sessions locales démarrées aujourd’hui · Tokens|Entrée|Entrée en cache (incluse)|Sortie|Sortie de raisonnement (incluse)|Total|Sessions|Journaux illisibles|Compte|Abonnement|Utilisation autorisée|Quota épuisé|Crédits supplémentaires|Solde|Illimité|Limite de dépassement atteinte|Disponibilité des modèles|Disponible|Activation possible avec crédits|Fenêtres de quota supplémentaires|Quota de revue de code|Quota Chatpass|Fenêtre|Réinitialisation|Contrôle des dépenses|Limite atteinte|Limite individuelle|Cartes de réinitialisation|Nombre disponible|Total obtenu|Achat possible|Historique activé|Carte|Type de réinitialisation|État|Date d’attribution|Date d’expiration|Début de l’utilisation|Date d’utilisation|Compatible avec l’abonnement|Oui|Non|Aucun|Inconnu|Dernière mise à jour du compte|Aucune donnée|Disponible|Utilisation en cours|Utilisé|Expiré|Compte à rebours",
        German => "Codex-Übersicht|Heute gestartete lokale Sitzungen · Tokens|Eingabe|Gecachte Eingabe (enthalten)|Ausgabe|Reasoning-Ausgabe (enthalten)|Gesamt|Sitzungen|Unlesbare Protokolle|Konto|Tarif|Nutzung erlaubt|Kontingent aufgebraucht|Zusätzliche Credits|Guthaben|Unbegrenzt|Mehrverbrauchslimit erreicht|Modellverfügbarkeit|Verfügbar|Mit Credits aktivierbar|Zusätzliche Kontingentfenster|Kontingent für Codeprüfung|Chatpass-Kontingent|Zeitfenster|Zurücksetzung|Ausgabenlimit|Limit erreicht|Individuelles Limit|Reset-Karten|Verfügbare Anzahl|Insgesamt erhalten|Kauf möglich|Verlauf aktiviert|Karte|Reset-Typ|Status|Erhalten am|Gültig bis|Einlösung begonnen|Eingelöst am|Vom Tarif unterstützt|Ja|Nein|Keine|Unbekannt|Letzte Kontoaktualisierung|Keine Daten|Verfügbar|Wird eingelöst|Eingelöst|Abgelaufen|Countdown",
        Japanese => "Codex の概要|本日開始したローカル会話 · Token|入力|キャッシュ入力（内数）|出力|推論出力（内数）|合計|会話数|読み取れないログ|アカウント|プラン|利用可能|上限に到達|追加クレジット|残高|無制限|超過利用の上限に到達|モデルの利用可否|利用可能|クレジットで有効化可能|追加の利用枠|コードレビューの利用枠|Chatpass の利用枠|期間|リセット日時|支出制限|制限に到達|個人の支出上限|リセットカード|利用可能数|累計獲得数|購入可能|履歴が有効|カード|リセットの種類|状態|付与日時|有効期限|交換開始日時|交換完了日時|プランで対応|はい|いいえ|なし|不明|アカウント情報の更新日時|データなし|利用可能|交換中|交換済み|期限切れ|カウントダウン",
        Korean => "Codex 개요|오늘 시작한 로컬 대화 · Token|입력|캐시 입력 (포함)|출력|추론 출력 (포함)|합계|대화 수|읽을 수 없는 로그|계정|요금제|사용 가능|할당량 소진|추가 크레딧|잔액|무제한|초과 사용 한도 도달|모델 사용 가능 여부|사용 가능|크레딧으로 활성화 가능|추가 할당량 기간|코드 검토 할당량|Chatpass 할당량|기간|초기화 시간|지출 제한|한도 도달|개인 지출 한도|초기화 카드|사용 가능 수|누적 획득|구매 가능|기록 활성화|카드|초기화 유형|상태|지급 시간|만료 시간|교환 시작 시간|교환 완료 시간|요금제 지원|예|아니요|없음|알 수 없음|계정 정보 갱신 시간|데이터 없음|사용 가능|교환 중|교환 완료|만료됨|카운트다운",
        Russian => "Обзор Codex|Локальные сеансы, начатые сегодня · Токены|Вход|Кэшированный вход (включён)|Выход|Выход рассуждений (включён)|Всего|Сеансы|Нечитаемые журналы|Аккаунт|Тариф|Использование разрешено|Квота исчерпана|Дополнительные кредиты|Баланс|Без ограничений|Лимит превышения достигнут|Доступность моделей|Доступно|Можно включить кредитами|Дополнительные окна квоты|Квота проверки кода|Квота Chatpass|Окно|Время сброса|Контроль расходов|Лимит достигнут|Личный лимит|Карты сброса|Доступное количество|Всего получено|Покупка доступна|История включена|Карта|Тип сброса|Статус|Время выдачи|Срок действия|Начало обмена|Время обмена|Поддерживается тарифом|Да|Нет|Нет|Неизвестно|Обновление данных аккаунта|Нет данных|Доступно|Обмен выполняется|Использовано|Истекло|Обратный отсчёт",
        PortugueseBrazil => "Visão geral do Codex|Sessões locais iniciadas hoje · Tokens|Entrada|Entrada em cache (incluída)|Saída|Saída de raciocínio (incluída)|Total|Sessões|Logs ilegíveis|Conta|Plano|Uso permitido|Cota esgotada|Créditos adicionais|Saldo|Ilimitado|Limite de excedente atingido|Disponibilidade dos modelos|Disponível|Créditos podem habilitar|Janelas de cota adicionais|Cota de revisão de código|Cota do Chatpass|Janela|Redefinição em|Controle de gastos|Limite atingido|Limite individual|Cartões de redefinição|Quantidade disponível|Total obtido|Compra disponível|Histórico ativado|Cartão|Tipo de redefinição|Status|Concedido em|Expira em|Início do resgate|Resgatado em|Compatível com o plano|Sim|Não|Nenhum|Desconhecido|Última atualização da conta|Sem dados|Disponível|Resgate em andamento|Resgatado|Expirado|Contagem regressiva",
    };
    labels.split('|').nth(key as usize).unwrap_or("?")
}

fn value(v: &Value, language: LanguageId) -> String {
    match v {
        Value::Null => label(language, Key::Absent).into(),
        Value::Bool(b) => label(language, if *b { Key::Yes } else { Key::No }).into(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => label(language, Key::Unknown).into(),
    }
}

fn timestamp(v: &Value) -> Option<i64> {
    v.as_i64().filter(|n| *n >= 0)
        .or_else(|| v.as_str().and_then(|s| time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok())
            .map(|t| t.unix_timestamp()).filter(|n| *n >= 0))
}

fn beijing_time(unix_seconds: i64) -> Option<time::OffsetDateTime> {
    let offset = time::UtcOffset::from_hms(8, 0, 0).ok()?;
    time::OffsetDateTime::from_unix_timestamp(unix_seconds).ok().map(|t| t.to_offset(offset))
}

fn format_timestamp(unix_seconds: i64) -> Option<String> {
    let t = beijing_time(unix_seconds)?;
    Some(format!("{:04}-{:02}-{:02} {:02}:{:02}", t.year(), u8::from(t.month()), t.day(), t.hour(), t.minute()))
}

fn date(v: &Value, language: LanguageId) -> String {
    timestamp(v).and_then(format_timestamp).unwrap_or_else(|| value(v, language))
}

fn countdown(v: &Value, language: LanguageId) -> Option<String> {
    let target = timestamp(v)?;
    let now = SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    let remaining = target.saturating_sub(now).max(0) as u64;
    let days = remaining / 86_400;
    let hours = remaining % 86_400 / 3_600;
    let minutes = remaining % 3_600 / 60;
    let seconds = remaining % 60;
    Some(match language {
        LanguageId::SimplifiedChinese | LanguageId::TraditionalChinese => format!("{days}天 {hours:02}:{minutes:02}:{seconds:02}"),
        _ => format!("{days}d {hours:02}:{minutes:02}:{seconds:02}"),
    })
}

pub fn row(language: LanguageId, key: Key, value: String) -> Row {
    Row { title: label(language, key).into(), value, section: false, column: 0, card: false, badge: String::new(), badge_tone: 0, refresh_line: None }
}
pub fn section(language: LanguageId, key: Key) -> Row {
    Row { title: label(language, key).into(), value: String::new(), section: true, column: 0, card: false, badge: String::new(), badge_tone: 0, refresh_line: None }
}

fn display_text(text: String, language: LanguageId) -> Option<String> {
    let normalized = text.trim().to_ascii_lowercase();
    if text == label(language, Key::NoData) || text == label(language, Key::Absent)
        || matches!(normalized.as_str(), "none" | "no data" | "null" | "n/a")
    { None } else { Some(text) }
}

fn display_value(raw: &Value, language: LanguageId) -> Option<String> {
    display_text(value(raw, language), language)
}

fn paired_row(language: LanguageId, pairs: Vec<(Key, String)>, column: u8) -> Option<Row> {
    if pairs.is_empty() { return None; }
    Some(Row {
        title: pairs.iter().map(|(key, _)| label(language, *key)).collect::<Vec<_>>().join(" / "),
        value: pairs.into_iter().map(|(_, value)| value).collect::<Vec<_>>().join(" / "),
        section: false,
        column,
        card: false,
        badge: String::new(),
        badge_tone: 0,
        refresh_line: None,
    })
}

fn prune_empty_sections(rows: Vec<Row>) -> Vec<Row> {
    let mut result = Vec::new();
    let mut pending_sections = Vec::new();
    for row in rows {
        if row.section {
            pending_sections.push(row);
        } else {
            result.append(&mut pending_sections);
            result.push(row);
        }
    }
    result
}

pub fn rows(language: LanguageId) -> Vec<Row> {
    let snapshot = ACCOUNT.lock().unwrap();
    build_rows(snapshot.usage.as_ref(), snapshot.cards.as_ref(), snapshot.updated, language)
}

fn build_rows(usage: Option<&Value>, cards: Option<&Value>, updated: Option<SystemTime>, lang: LanguageId) -> Vec<Row> {
    use Key::*;
    let Some(u) = usage else { return Vec::new(); };
    let mut rows = Vec::new();
    let mut account_rows = Vec::new();
    if let Some(t) = updated.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|t| i64::try_from(t.as_secs()).ok()).and_then(beijing_time) {
        account_rows.push(row(lang, Updated, format!("{:02}/{:02} {:02}:{:02}", u8::from(t.month()), t.day(), t.hour(), t.minute())));
    }
    if let Some(v) = display_value(&u["plan_type"], lang) { account_rows.push(row(lang, Plan, v)); }
    window_rows(&mut account_rows, &u["rate_limit"], lang);
    if account_rows.iter().any(|r| !r.section) {
        rows.push(section(lang, Account));
        rows.extend(account_rows);
    }

    let credits = &u["credits"];
    if credits["has_credits"].as_bool() != Some(false) {
        let mut credit_rows = Vec::new();
        if let Some(v) = display_value(&credits["has_credits"], lang) { credit_rows.push(row(lang, Credits, v)); }
        for (key, field) in [(Balance, "balance"), (Unlimited, "unlimited"), (Overage, "overage_limit_reached")] {
            if let Some(v) = display_value(&credits[field], lang) { credit_rows.push(row(lang, key, v)); }
        }
        if !credit_rows.is_empty() {
            rows.push(section(lang, Credits));
            rows.extend(credit_rows);
        }
    }

    let mut model_rows = Vec::new();
    if let Some(models) = u["model_usage"].as_object().filter(|m| !m.is_empty()) {
        for (name, model) in models {
            let details = [
                display_value(&model["available"], lang).map(|v| format!("{}: {v}", label(lang, Available))),
                display_value(&model["credits_would_enable"], lang).map(|v| format!("{}: {v}", label(lang, EnableCredits))),
            ].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            if !details.is_empty() {
                model_rows.push(Row { title: name.clone(), value: details, section: false, column: 0, card: false, badge: String::new(), badge_tone: 0, refresh_line: None });
            }
            if let Some(v) = display_text(date(&model["available_at"], lang), lang) {
                model_rows.push(row(lang, Available, v));
            }
            if let Some(v) = countdown(&model["available_at"], lang) { model_rows.push(row(lang, Countdown, v)); }
        }
    }
    if model_rows.iter().any(|r| !r.section) {
        rows.push(section(lang, Models));
        rows.extend(model_rows);
    }

    for (key, field) in [(Extra, "additional_rate_limits"), (Review, "code_review_rate_limit"), (Chatpass, "chatpass")] {
        let data = &u[field];
        let mut limit_rows = Vec::new();
        if let Some(entries) = data.as_array() {
            for entry in entries { window_rows(&mut limit_rows, entry, lang); }
        } else if !data.is_null() {
            window_rows(&mut limit_rows, data, lang);
        }
        if limit_rows.iter().any(|r| !r.section) {
            rows.push(section(lang, key));
            rows.extend(limit_rows);
        }
    }
    let mut spend_rows = Vec::new();
    if u["spend_control"]["reached"].as_bool() != Some(false) {
        if let Some(v) = display_value(&u["spend_control"]["reached"], lang) { spend_rows.push(row(lang, Reached, v)); }
    }
    if let Some(v) = display_value(&u["spend_control"]["individual_limit"], lang) { spend_rows.push(row(lang, Limit, v)); }
    if !spend_rows.is_empty() {
        rows.push(section(lang, Spend));
        rows.extend(spend_rows);
    }

    let Some(c) = cards else { return prune_empty_sections(rows); };
    let mut card_rows = Vec::new();
    let mut counts = Vec::new();
    if let Some(v) = display_value(&c["available_count"], lang) { counts.push((Count, v)); }
    if let Some(v) = display_value(&c["total_earned_count"], lang) { counts.push((Earned, v)); }
    if let Some(row) = paired_row(lang, counts, 1) { card_rows.push(row); }
    let mut flags = Vec::new();
    if let Some(v) = display_value(&c["immediate_reset_purchase_eligible"], lang) { flags.push((Purchase, v)); }
    if let Some(v) = display_value(&c["history_enabled"], lang) { flags.push((History, v)); }
    if let Some(row) = paired_row(lang, flags, 1) { card_rows.push(row); }

    for (i, card) in c["credits"].as_array().into_iter().flatten().enumerate() {
        let reset_type = match card["reset_type"].as_str() {
            Some("five_hour") | Some("5h") => Some("5h".into()),
            Some("seven_day") | Some("7d") => Some("7d".into()),
            Some("codex_rate_limits") => Some(match lang {
                LanguageId::SimplifiedChinese => "Codex 额度重置".into(),
                LanguageId::TraditionalChinese => "Codex 額度重設".into(),
                LanguageId::English => "Codex rate-limit reset".into(),
                LanguageId::Dutch => "Codex-limietreset".into(),
                LanguageId::Spanish => "Restablecimiento de límite de Codex".into(),
                LanguageId::French => "Réinitialisation de limite Codex".into(),
                LanguageId::German => "Codex-Kontingent zurücksetzen".into(),
                LanguageId::Japanese => "Codex 利用枠のリセット".into(),
                LanguageId::Korean => "Codex 사용량 초기화".into(),
                LanguageId::Russian => "Сброс лимита Codex".into(),
                LanguageId::PortugueseBrazil => "Redefinição do limite do Codex".into(),
            }),
            _ => display_value(&card["reset_type"], lang),
        };
        let (status, badge_tone) = match card["status"].as_str() {
            Some("available") => (Some(label(lang, AvailableStatus).into()), 1),
            Some("redeeming") => (Some(label(lang, Redeeming).into()), 2),
            Some("redeemed") => (Some(label(lang, RedeemedStatus).into()), 3),
            Some("expired") => (Some(label(lang, Expired).into()), 3),
            _ => (display_value(&card["status"], lang), 0),
        };
        let part = |key: Key, value: Option<String>| value.map(|v| format!("{} {}", label(lang, key), v));
        let line = |parts: Vec<Option<String>>| {
            let text = parts.into_iter().flatten().collect::<Vec<_>>().join("    ·    ");
            (!text.is_empty()).then_some(text)
        };
        let detail_lines = vec![
            line(vec![part(Type, reset_type)]),
            line(vec![part(Supported, display_value(&card["is_supported_by_plan"], lang))]),
            line(vec![part(Granted, display_text(date(&card["granted_at"], lang), lang)), part(Expires, display_text(date(&card["expires_at"], lang), lang))]),
            countdown(&card["expires_at"], lang).map(|v| format!("{} {v}", label(lang, Expires))),
            line(vec![part(RedeemStart, display_text(date(&card["redeem_started_at"], lang), lang)), part(Redeemed, display_text(date(&card["redeemed_at"], lang), lang))]),
        ];
        let refresh_line = detail_lines[3].as_ref().map(|_| detail_lines[..3].iter().flatten().count());
        let details = detail_lines.into_iter().flatten().collect::<Vec<_>>().join("\n");
        if !details.is_empty() {
            card_rows.push(Row {
                title: format!("{} {}", label(lang, Card), i + 1),
                value: details,
                section: false,
                column: 1,
                card: true,
                badge: status.unwrap_or_default(),
                badge_tone,
                refresh_line,
            });
        }
    }
    if !card_rows.is_empty() {
        rows.push(Row { title: label(lang, Cards).into(), value: String::new(), section: true, column: 1, card: false, badge: String::new(), badge_tone: 0, refresh_line: None });
        rows.extend(card_rows);
    }
    prune_empty_sections(rows)
}

fn window_rows(rows: &mut Vec<Row>, data: &Value, lang: LanguageId) {
    if let Some(name) = data["limit_name"].as_str() {
        rows.push(Row { title: name.into(), value: String::new(), section: true, column: 0, card: false, badge: String::new(), badge_tone: 0, refresh_line: None });
    }
    for (key, field) in [(Key::Allowed, "allowed"), (Key::Exhausted, "limit_reached")] {
        if let Some(v) = data.get(field).and_then(|v| display_value(v, lang)) { rows.push(row(lang, key, v)); }
    }
    let windows: Vec<&Value> = if let Some(windows) = data["windows"].as_array() { windows.iter().collect() }
        else if data.get("rate_limit").is_some() { return window_rows(rows, &data["rate_limit"], lang); }
        else if data.get("used_percent").is_some() { vec![data] }
        else { [data.get("primary_window"), data.get("secondary_window")].into_iter().flatten().filter(|v| !v.is_null()).collect() };
    for window in windows {
        let seconds = window["limit_window_seconds"].as_u64();
        let duration = seconds.map(|s| if s % 86400 == 0 { format!("{}d", s / 86400) } else { format!("{}h", s / 3600) });
        if let Some(used) = display_value(&window["used_percent"], lang) {
            let display = duration.map(|d| format!("{d} · {used}%")).unwrap_or_else(|| format!("{used}%"));
            rows.push(row(lang, Key::Window, display));
        }
        if let Some(reset) = display_text(date(&window["reset_at"], lang), lang) { rows.push(row(lang, Key::Reset, reset)); }
        if let Some(remaining) = countdown(&window["reset_at"], lang) { rows.push(row(lang, Key::Countdown, remaining)); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_language_has_all_labels() {
        for lang in LanguageId::ALL { assert_ne!(label(lang, Key::Expired), "?"); }
    }
    #[test]
    fn unknown_account_does_not_claim_zero_and_nested_windows_are_rendered() {
        assert!(build_rows(None, None, None, LanguageId::English).is_empty());
        let data = json!({"plan_type":"plus", "chatpass":{"windows":[{"used_percent":12,"limit_window_seconds":18000,"reset_at":1790797502}]}});
        let rows = build_rows(Some(&data), None, None, LanguageId::English);
        assert!(rows.iter().any(|r| r.value == "5h · 12%"));
        assert!(!rows.iter().any(|r| r.value == "None" || r.value == "No data"));
    }
}
