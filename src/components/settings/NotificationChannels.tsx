// NotificationChannels — 外部渠道配置区（通知体系票 03/04）。
// 7 渠道：无配置=旁路，配置即启用；每渠道「测试发送」走真实投递（终态落 notification_delivery）。
// 凭据（bot_token / SMTP password）后端回显哨兵 ••••：保存时原样回传 = 保留旧值（system_cmd 合并）。
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { notificationApi, type NotificationSettings } from "../../services/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

type Persist = (f: (prev: NotificationSettings) => NotificationSettings) => void;

const CHANNELS = ["webhook", "telegram", "bark", "discord", "slack", "feishu", "email"] as const;
type ChannelKey = (typeof CHANNELS)[number];

interface TestState { busy: boolean; result?: string }

function Field({ label, value, onChange, placeholder, password }: {
  label: string; value: string; onChange: (v: string) => void; placeholder?: string; password?: boolean;
}) {
  return (
    <label style={{ display: "flex", flexDirection: "column", gap: 3, fontSize: 11, minWidth: 180, flex: 1 }}>
      <span className="text-secondary">{label}</span>
      <Input
        className="input"
        type={password ? "password" : "text"}
        style={{ fontSize: 12 }}
        value={value}
        placeholder={placeholder}
        onChange={e => onChange(e.target.value)}
      />
    </label>
  );
}

function ChannelCard({ channel, settings, persist, test, setTest }: {
  channel: ChannelKey; settings: NotificationSettings; persist: Persist;
  test: TestState; setTest: (t: TestState) => void;
}) {
  const { t } = useTranslation();
  const ch = settings.channels;
  const setCfg = (f: (draft: NotificationSettings) => void) =>
    persist(prev => {
      const next = { ...prev, channels: { ...prev.channels } };
      f(next);
      return next;
    });

  const handleTest = async () => {
    setTest({ busy: true });
    try {
      const r = await notificationApi.testChannel(channel);
      setTest({ busy: false, result: r.ok ? `✓ ${r.duration_ms}ms` : `✗ ${r.error}` });
    } catch (e) {
      setTest({ busy: false, result: `✗ ${String(e)}` });
    }
  };

  // 已配置判定（与后端 channels.rs configured 对称）：url / token+chat / smtp+from+to
  const configured =
    channel === "telegram" ? !!ch.telegram?.bot_token && !!ch.telegram?.chat_id
    : channel === "email" ? !!ch.email?.smtp_host && !!ch.email?.from && !!ch.email?.to
    : channel !== "webhook" ? !!(ch as Record<string, { url: string } | null>)[channel]?.url
    : !!ch.webhook?.url;

  return (
    <div id={`notif-channel-${channel}`} data-configured={configured ? "yes" : "no"}
      className="glass-surface" style={{ padding: "10px 14px", display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <span style={{ fontSize: 12, fontWeight: 600 }}>
          {t(`notif.channel.${channel}`, channel)}{configured ? "" : ` · ${t("notif.channels.bypass", "未配置=旁路")}`}
        </span>
        <Button variant="ghost" size="sm" style={{ fontSize: 11, height: "auto", padding: "2px 8px" }}
          disabled={test.busy} onClick={handleTest}>
          {test.busy ? t("notif.channels.testing", "发送中…") : t("notif.channels.test", "测试发送")}
        </Button>
      </div>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
        {channel === "webhook" && (<>
          <Field label={t("notif.channels.url", "URL")} value={ch.webhook?.url ?? ""}
            onChange={v => setCfg(n => { n.channels.webhook = { url: v, template: n.channels.webhook?.template ?? "" }; })} />
          <Field label={t("notif.channels.template", "JSON 模板（可选）")} value={ch.webhook?.template ?? ""}
            placeholder={'{"title":"{title}","body":"{body}"}'}
            onChange={v => setCfg(n => { n.channels.webhook = { url: n.channels.webhook?.url ?? "", template: v }; })} />
        </>)}
        {channel === "telegram" && (<>
          <Field label={t("notif.channels.botToken", "Bot Token")} value={ch.telegram?.bot_token ?? ""} password
            onChange={v => setCfg(n => { n.channels.telegram = { bot_token: v, chat_id: n.channels.telegram?.chat_id ?? "", api_base: n.channels.telegram?.api_base ?? "" }; })} />
          <Field label={t("notif.channels.chatId", "Chat ID")} value={ch.telegram?.chat_id ?? ""}
            onChange={v => setCfg(n => { n.channels.telegram = { bot_token: n.channels.telegram?.bot_token ?? "", chat_id: v, api_base: n.channels.telegram?.api_base ?? "" }; })} />
          <Field label={t("notif.channels.apiBase", "API 反代（可选）")} value={ch.telegram?.api_base ?? ""}
            placeholder="https://api.telegram.org"
            onChange={v => setCfg(n => { n.channels.telegram = { bot_token: n.channels.telegram?.bot_token ?? "", chat_id: n.channels.telegram?.chat_id ?? "", api_base: v }; })} />
        </>)}
        {(channel === "bark" || channel === "discord" || channel === "slack" || channel === "feishu") && (
          <Field label={t("notif.channels.url", "URL")}
            value={(ch as Record<string, { url: string } | null>)[channel]?.url ?? ""}
            onChange={v => setCfg(n => { (n.channels as Record<string, { url: string } | null>)[channel] = { url: v }; })} />
        )}
        {channel === "email" && (<>
          <Field label={t("notif.channels.smtpHost", "SMTP 主机")} value={ch.email?.smtp_host ?? ""}
            onChange={v => setCfg(n => { n.channels.email = { ...(n.channels.email ?? { smtp_host: "", smtp_port: 0, username: "", password: "", from: "", to: "" }), smtp_host: v }; })} />
          <Field label={t("notif.channels.smtpPort", "端口（0=587）")} value={String(ch.email?.smtp_port ?? 0)}
            onChange={v => setCfg(n => { n.channels.email = { ...(n.channels.email ?? { smtp_host: "", smtp_port: 0, username: "", password: "", from: "", to: "" }), smtp_port: Number(v) || 0 }; })} />
          <Field label={t("notif.channels.username", "用户名")} value={ch.email?.username ?? ""}
            onChange={v => setCfg(n => { n.channels.email = { ...(n.channels.email ?? { smtp_host: "", smtp_port: 0, username: "", password: "", from: "", to: "" }), username: v }; })} />
          <Field label={t("notif.channels.password", "密码")} value={ch.email?.password ?? ""} password
            onChange={v => setCfg(n => { n.channels.email = { ...(n.channels.email ?? { smtp_host: "", smtp_port: 0, username: "", password: "", from: "", to: "" }), password: v }; })} />
          <Field label={t("notif.channels.from", "发件人")} value={ch.email?.from ?? ""}
            placeholder="aidog@example.com"
            onChange={v => setCfg(n => { n.channels.email = { ...(n.channels.email ?? { smtp_host: "", smtp_port: 0, username: "", password: "", from: "", to: "" }), from: v }; })} />
          <Field label={t("notif.channels.to", "收件人")} value={ch.email?.to ?? ""}
            onChange={v => setCfg(n => { n.channels.email = { ...(n.channels.email ?? { smtp_host: "", smtp_port: 0, username: "", password: "", from: "", to: "" }), to: v }; })} />
        </>)}
      </div>
      {test.result && <div style={{ fontSize: 11, color: test.result.startsWith("✓") ? "var(--status-ok, #4ade80)" : "var(--status-error, #f87171)" }}>{test.result}</div>}
    </div>
  );
}

export function NotificationChannelsSection({ settings, persist }: { settings: NotificationSettings; persist: Persist }) {
  const { t } = useTranslation();
  const [tests, setTests] = useState<Record<string, TestState>>({});
  return (
    <div className="glass-surface" style={{ padding: "14px 18px", display: "flex", flexDirection: "column", gap: 10 }}>
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("notif.channels.title", "外部渠道")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("notif.channels.desc", "无配置=旁路，配置即启用；投递失败自动重试（≤3），记录见收件箱「投递记录」")}
        </div>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {CHANNELS.map(c => (
          <ChannelCard key={c} channel={c} settings={settings} persist={persist}
            test={tests[c] ?? { busy: false }} setTest={t2 => setTests(prev => ({ ...prev, [c]: t2 }))} />
        ))}
      </div>
    </div>
  );
}
