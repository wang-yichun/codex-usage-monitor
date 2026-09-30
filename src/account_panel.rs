use std::sync::Mutex;
use std::time::SystemTime;
use serde_json::{json, Value};
use crate::localization::LanguageId;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Row {
    pub title: String,
    pub value: String,
    pub section: bool,
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
    Title, Local, Input, Cached, Output, Reasoning, Total, Sessions, Unreadable,
    Account, Plan, Allowed, Exhausted, Credits, Balance, Unlimited, Overage,
    Models, Available, EnableCredits, Extra, Review, Chatpass, Window, Reset,
    Spend, Reached, Limit, Cards, Count, Earned, Purchase, History, Card, Type,
    Status, Granted, Expires, RedeemStart, Redeemed, Supported, Yes, No, Absent,
    Unknown, Updated, NoData, AvailableStatus, Redeeming, RedeemedStatus, Expired,
}

pub fn label(language: LanguageId, key: Key) -> &'static str {
    use LanguageId::*;
    let labels = match language {
        English => "Codex overview|Local sessions started today · Tokens|Input|Cached input (included)|Output|Reasoning output (included)|Total|Sessions|Unreadable logs|Account|Plan|Usage allowed|Quota exhausted|Extra credits|Balance|Unlimited|Overage limit reached|Model availability|Available|Credits can enable|Additional quota windows|Code review quota|Chatpass quota|Window|Resets at|Spend control|Limit reached|Individual limit|Reset cards|Available count|Total earned|Purchase eligible|History enabled|Card|Reset type|Status|Granted at|Expires at|Redemption started|Redeemed at|Supported by plan|Yes|No|None|Unknown|Last account update|No data|Available|Redeeming|Redeemed|Expired",
        SimplifiedChinese => "Codex 信息概览|本机今日开始的会话 · Token|输入|其中缓存输入|输出|其中推理输出|总计|会话数|未能读取的日志|账户|套餐类型|允许使用额度|额度已耗尽|额外 credits|余额|无限额度|超额额度已耗尽|模型可用状态|可用|可用 credits 解锁|额外额度窗口|代码审查额度|Chatpass 额度|时间窗口|重置时间|消费限制|已触发限制|个人消费上限|重置卡|可用数量|累计获得|可购买|启用历史记录|卡片|重置类型|状态|发放时间|到期时间|开始兑换时间|兑换完成时间|套餐支持|是|否|无|未知|账户数据更新时间|暂无数据|可用|兑换中|已兑换|已过期",
        TraditionalChinese => "Codex 資訊概覽|本機今日開始的對話 · Token|輸入|其中快取輸入|輸出|其中推理輸出|總計|對話數|無法讀取的記錄|帳戶|方案類型|允許使用額度|額度已耗盡|額外 credits|餘額|無限額度|超額額度已耗盡|模型可用狀態|可用|可用 credits 解鎖|額外額度視窗|程式碼審查額度|Chatpass 額度|時間視窗|重設時間|消費限制|已觸發限制|個人消費上限|重設卡|可用數量|累計獲得|可購買|啟用歷史記錄|卡片|重設類型|狀態|發放時間|到期時間|開始兌換時間|兌換完成時間|方案支援|是|否|無|未知|帳戶資料更新時間|暫無資料|可用|兌換中|已兌換|已過期",
        Dutch => "Codex-overzicht|Lokale sessies gestart vandaag · Tokens|Invoer|Gecachte invoer (inbegrepen)|Uitvoer|Redeneeruitvoer (inbegrepen)|Totaal|Sessies|Onleesbare logboeken|Account|Abonnement|Gebruik toegestaan|Quotum opgebruikt|Extra credits|Saldo|Onbeperkt|Overschrijdingslimiet bereikt|Modelbeschikbaarheid|Beschikbaar|Credits kunnen activeren|Extra quotumvensters|Quotum voor codereview|Chatpass-quotum|Tijdvenster|Reset op|Bestedingslimiet|Limiet bereikt|Persoonlijke limiet|Resetkaarten|Beschikbaar aantal|Totaal verdiend|Aankoop mogelijk|Geschiedenis ingeschakeld|Kaart|Resettype|Status|Toegekend op|Verloopt op|Inwisseling gestart|Ingewisseld op|Ondersteund door abonnement|Ja|Nee|Geen|Onbekend|Laatste accountupdate|Geen gegevens|Beschikbaar|Wordt ingewisseld|Ingewisseld|Verlopen",
        Spanish => "Resumen de Codex|Sesiones locales iniciadas hoy · Tokens|Entrada|Entrada en caché (incluida)|Salida|Salida de razonamiento (incluida)|Total|Sesiones|Registros ilegibles|Cuenta|Plan|Uso permitido|Cuota agotada|Créditos adicionales|Saldo|Ilimitado|Límite de excedente alcanzado|Disponibilidad de modelos|Disponible|Los créditos pueden habilitar|Ventanas de cuota adicionales|Cuota de revisión de código|Cuota de Chatpass|Ventana|Se restablece|Control de gasto|Límite alcanzado|Límite individual|Tarjetas de reinicio|Cantidad disponible|Total obtenido|Compra disponible|Historial habilitado|Tarjeta|Tipo de reinicio|Estado|Fecha de concesión|Fecha de vencimiento|Inicio del canje|Fecha del canje|Compatible con el plan|Sí|No|Ninguno|Desconocido|Última actualización de cuenta|Sin datos|Disponible|Canje en curso|Canjeado|Vencido",
        French => "Aperçu de Codex|Sessions locales démarrées aujourd’hui · Tokens|Entrée|Entrée en cache (incluse)|Sortie|Sortie de raisonnement (incluse)|Total|Sessions|Journaux illisibles|Compte|Abonnement|Utilisation autorisée|Quota épuisé|Crédits supplémentaires|Solde|Illimité|Limite de dépassement atteinte|Disponibilité des modèles|Disponible|Activation possible avec crédits|Fenêtres de quota supplémentaires|Quota de revue de code|Quota Chatpass|Fenêtre|Réinitialisation|Contrôle des dépenses|Limite atteinte|Limite individuelle|Cartes de réinitialisation|Nombre disponible|Total obtenu|Achat possible|Historique activé|Carte|Type de réinitialisation|État|Date d’attribution|Date d’expiration|Début de l’utilisation|Date d’utilisation|Compatible avec l’abonnement|Oui|Non|Aucun|Inconnu|Dernière mise à jour du compte|Aucune donnée|Disponible|Utilisation en cours|Utilisé|Expiré",
        German => "Codex-Übersicht|Heute gestartete lokale Sitzungen · Tokens|Eingabe|Gecachte Eingabe (enthalten)|Ausgabe|Reasoning-Ausgabe (enthalten)|Gesamt|Sitzungen|Unlesbare Protokolle|Konto|Tarif|Nutzung erlaubt|Kontingent aufgebraucht|Zusätzliche Credits|Guthaben|Unbegrenzt|Mehrverbrauchslimit erreicht|Modellverfügbarkeit|Verfügbar|Mit Credits aktivierbar|Zusätzliche Kontingentfenster|Kontingent für Codeprüfung|Chatpass-Kontingent|Zeitfenster|Zurücksetzung|Ausgabenlimit|Limit erreicht|Individuelles Limit|Reset-Karten|Verfügbare Anzahl|Insgesamt erhalten|Kauf möglich|Verlauf aktiviert|Karte|Reset-Typ|Status|Erhalten am|Gültig bis|Einlösung begonnen|Eingelöst am|Vom Tarif unterstützt|Ja|Nein|Keine|Unbekannt|Letzte Kontoaktualisierung|Keine Daten|Verfügbar|Wird eingelöst|Eingelöst|Abgelaufen",
        Japanese => "Codex の概要|本日開始したローカル会話 · Token|入力|キャッシュ入力（内数）|出力|推論出力（内数）|合計|会話数|読み取れないログ|アカウント|プラン|利用可能|上限に到達|追加クレジット|残高|無制限|超過利用の上限に到達|モデルの利用可否|利用可能|クレジットで有効化可能|追加の利用枠|コードレビューの利用枠|Chatpass の利用枠|期間|リセット日時|支出制限|制限に到達|個人の支出上限|リセットカード|利用可能数|累計獲得数|購入可能|履歴が有効|カード|リセットの種類|状態|付与日時|有効期限|交換開始日時|交換完了日時|プランで対応|はい|いいえ|なし|不明|アカウント情報の更新日時|データなし|利用可能|交換中|交換済み|期限切れ",
        Korean => "Codex 개요|오늘 시작한 로컬 대화 · Token|입력|캐시 입력 (포함)|출력|추론 출력 (포함)|합계|대화 수|읽을 수 없는 로그|계정|요금제|사용 가능|할당량 소진|추가 크레딧|잔액|무제한|초과 사용 한도 도달|모델 사용 가능 여부|사용 가능|크레딧으로 활성화 가능|추가 할당량 기간|코드 검토 할당량|Chatpass 할당량|기간|초기화 시간|지출 제한|한도 도달|개인 지출 한도|초기화 카드|사용 가능 수|누적 획득|구매 가능|기록 활성화|카드|초기화 유형|상태|지급 시간|만료 시간|교환 시작 시간|교환 완료 시간|요금제 지원|예|아니요|없음|알 수 없음|계정 정보 갱신 시간|데이터 없음|사용 가능|교환 중|교환 완료|만료됨",
        Russian => "Обзор Codex|Локальные сеансы, начатые сегодня · Токены|Вход|Кэшированный вход (включён)|Выход|Выход рассуждений (включён)|Всего|Сеансы|Нечитаемые журналы|Аккаунт|Тариф|Использование разрешено|Квота исчерпана|Дополнительные кредиты|Баланс|Без ограничений|Лимит превышения достигнут|Доступность моделей|Доступно|Можно включить кредитами|Дополнительные окна квоты|Квота проверки кода|Квота Chatpass|Окно|Время сброса|Контроль расходов|Лимит достигнут|Личный лимит|Карты сброса|Доступное количество|Всего получено|Покупка доступна|История включена|Карта|Тип сброса|Статус|Время выдачи|Срок действия|Начало обмена|Время обмена|Поддерживается тарифом|Да|Нет|Нет|Неизвестно|Обновление данных аккаунта|Нет данных|Доступно|Обмен выполняется|Использовано|Истекло",
        PortugueseBrazil => "Visão geral do Codex|Sessões locais iniciadas hoje · Tokens|Entrada|Entrada em cache (incluída)|Saída|Saída de raciocínio (incluída)|Total|Sessões|Logs ilegíveis|Conta|Plano|Uso permitido|Cota esgotada|Créditos adicionais|Saldo|Ilimitado|Limite de excedente atingido|Disponibilidade dos modelos|Disponível|Créditos podem habilitar|Janelas de cota adicionais|Cota de revisão de código|Cota do Chatpass|Janela|Redefinição em|Controle de gastos|Limite atingido|Limite individual|Cartões de redefinição|Quantidade disponível|Total obtido|Compra disponível|Histórico ativado|Cartão|Tipo de redefinição|Status|Concedido em|Expira em|Início do resgate|Resgatado em|Compatível com o plano|Sim|Não|Nenhum|Desconhecido|Última atualização da conta|Sem dados|Disponível|Resgate em andamento|Resgatado|Expirado",
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

fn date(v: &Value, language: LanguageId) -> String {
    let time = v.as_i64().filter(|n| *n >= 0).map(|n| std::time::UNIX_EPOCH + std::time::Duration::from_secs(n as u64))
        .or_else(|| v.as_str().and_then(|s| time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok())
            .and_then(|t| u64::try_from(t.unix_timestamp()).ok()).map(|n| std::time::UNIX_EPOCH + std::time::Duration::from_secs(n)));
    time.and_then(crate::native_interop::system_time_to_local).map(|t| format!("{:04}-{:02}-{:02} {:02}:{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute))
        .unwrap_or_else(|| value(v, language))
}

pub fn row(language: LanguageId, key: Key, value: String) -> Row {
    Row { title: label(language, key).into(), value, section: false }
}
pub fn section(language: LanguageId, key: Key) -> Row {
    Row { title: label(language, key).into(), value: String::new(), section: true }
}

pub fn rows(language: LanguageId) -> Vec<Row> {
    let snapshot = ACCOUNT.lock().unwrap();
    build_rows(snapshot.usage.as_ref(), snapshot.cards.as_ref(), snapshot.updated, language)
}

fn build_rows(usage: Option<&Value>, cards: Option<&Value>, updated: Option<SystemTime>, lang: LanguageId) -> Vec<Row> {
    use Key::*;
    let mut rows = vec![section(lang, Account)];
    let Some(u) = usage else { rows.push(row(lang, Account, label(lang, NoData).into())); return rows; };
    if let Some(t) = updated.and_then(crate::native_interop::system_time_to_local) {
        rows.push(row(lang, Updated, format!("{:02}/{:02} {:02}:{:02}", t.wMonth, t.wDay, t.wHour, t.wMinute)));
    }
    rows.push(row(lang, Plan, value(&u["plan_type"], lang)));
    window_rows(&mut rows, &u["rate_limit"], lang);
    rows.push(section(lang, Credits));
    rows.push(row(lang, Credits, value(&u["credits"]["has_credits"], lang)));
    for (key, field) in [(Balance, "balance"), (Unlimited, "unlimited"), (Overage, "overage_limit_reached")] {
        rows.push(row(lang, key, value(&u["credits"][field], lang)));
    }
    rows.push(section(lang, Models));
    if let Some(models) = u["model_usage"].as_object().filter(|m| !m.is_empty()) {
        for (name, model) in models {
            rows.push(Row { title: name.clone(), value: format!("{}: {} · {}: {}", label(lang, Available), value(&model["available"], lang), label(lang, EnableCredits), value(&model["credits_would_enable"], lang)), section: false });
            if !model["available_at"].is_null() { rows.push(row(lang, Available, date(&model["available_at"], lang))); }
        }
    } else { rows.push(row(lang, Models, label(lang, NoData).into())); }
    for (key, field) in [(Extra, "additional_rate_limits"), (Review, "code_review_rate_limit"), (Chatpass, "chatpass")] {
        rows.push(section(lang, key));
        let data = &u[field];
        if data.is_null() || data.as_array().is_some_and(|a| a.is_empty()) {
            rows.push(row(lang, Window, label(lang, NoData).into()));
        } else if let Some(entries) = data.as_array() {
            for entry in entries { window_rows(&mut rows, entry, lang); }
        } else { window_rows(&mut rows, data, lang); }
    }
    rows.push(section(lang, Spend));
    rows.push(row(lang, Reached, value(&u["spend_control"]["reached"], lang)));
    rows.push(row(lang, Limit, value(&u["spend_control"]["individual_limit"], lang)));
    rows.push(section(lang, Cards));
    let Some(c) = cards else { rows.push(row(lang, Cards, label(lang, NoData).into())); return rows; };
    for (key, field) in [(Count, "available_count"), (Earned, "total_earned_count"), (Purchase, "immediate_reset_purchase_eligible"), (History, "history_enabled")] {
        rows.push(row(lang, key, value(&c[field], lang)));
    }
    for (i, card) in c["credits"].as_array().into_iter().flatten().enumerate() {
        rows.push(Row { title: format!("{} {}", label(lang, Card), i + 1), value: String::new(), section: true });
        let reset_type = match card["reset_type"].as_str() {
            Some("five_hour") | Some("5h") => "5h".into(),
            Some("seven_day") | Some("7d") => "7d".into(),
            _ => value(&card["reset_type"], lang),
        };
        rows.push(row(lang, Type, reset_type));
        let status = match card["status"].as_str() {
            Some("available") => label(lang, AvailableStatus).into(),
            Some("redeeming") => label(lang, Redeeming).into(),
            Some("redeemed") => label(lang, RedeemedStatus).into(),
            Some("expired") => label(lang, Expired).into(),
            _ => value(&card["status"], lang),
        };
        rows.push(row(lang, Status, status));
        rows.push(row(lang, Supported, value(&card["is_supported_by_plan"], lang)));
        for (key, field) in [(Granted, "granted_at"), (Expires, "expires_at"), (RedeemStart, "redeem_started_at"), (Redeemed, "redeemed_at")] {
            rows.push(row(lang, key, date(&card[field], lang)));
        }
    }
    rows
}

fn window_rows(rows: &mut Vec<Row>, data: &Value, lang: LanguageId) {
    if let Some(name) = data["limit_name"].as_str() {
        rows.push(Row { title: name.into(), value: String::new(), section: true });
    }
    for (key, field) in [(Key::Allowed, "allowed"), (Key::Exhausted, "limit_reached")] {
        if let Some(v) = data.get(field) { rows.push(row(lang, key, value(v, lang))); }
    }
    let windows: Vec<&Value> = if let Some(windows) = data["windows"].as_array() { windows.iter().collect() }
        else if data.get("rate_limit").is_some() { return window_rows(rows, &data["rate_limit"], lang); }
        else if data.get("used_percent").is_some() { vec![data] }
        else { [data.get("primary_window"), data.get("secondary_window")].into_iter().flatten().filter(|v| !v.is_null()).collect() };
    if windows.is_empty() { rows.push(row(lang, Key::Window, label(lang, Key::NoData).into())); }
    for window in windows {
        let seconds = window["limit_window_seconds"].as_u64();
        let duration = seconds.map(|s| if s % 86400 == 0 { format!("{}d", s / 86400) } else { format!("{}h", s / 3600) }).unwrap_or_else(|| label(lang, Key::Unknown).into());
        rows.push(row(lang, Key::Window, format!("{duration} · {}%", value(&window["used_percent"], lang))));
        rows.push(row(lang, Key::Reset, date(&window["reset_at"], lang)));
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
        assert!(build_rows(None, None, None, LanguageId::English).iter().any(|r| r.value == "No data"));
        let data = json!({"plan_type":"plus", "chatpass":{"windows":[{"used_percent":12,"limit_window_seconds":18000,"reset_at":1790797502}]}});
        let rows = build_rows(Some(&data), None, None, LanguageId::English);
        assert!(rows.iter().any(|r| r.value == "5h · 12%"));
        assert!(rows.iter().any(|r| r.title == "Balance" && r.value == "None"));
    }
}
