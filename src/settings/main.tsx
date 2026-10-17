import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import "./settings.css";

type Mapping = {
  translation: string;
  pronunciation: string;
  description: string;
  example: string;
  more_examples: string;
  synonym: string;
  situation: string;
  kind: string;
  status: string;
  added: string;
  last_reviewed: string;
};

type Settings = {
  database_input: string;
  data_source_id: string | null;
  data_source_name: string | null;
  mapping: Mapping;
  hidden_statuses: string[];
};

type SettingsView = Settings & { has_token: boolean };

type DataSourceOption = { id: string; name: string };
type SyncResult = { total: number; visible: number };
type Status = { kind: "idle" | "busy" | "ok" | "error"; text: string };

const MAPPING_FIELDS: Array<[keyof Mapping, string]> = [
  ["translation", "Переклад"],
  ["pronunciation", "Вимова"],
  ["description", "Опис"],
  ["example", "Приклад"],
  ["more_examples", "Інші приклади (по рядку)"],
  ["synonym", "Синонім"],
  ["situation", "Ситуації (multi-select)"],
  ["kind", "Тип (select)"],
  ["status", "Статус (select)"],
  ["added", "Додано (дата)"],
  ["last_reviewed", "Останнє повторення (дата)"],
];

function log(msg: string): void {
  invoke("debug_log", { msg: `[settings] ${msg}` }).catch(() => undefined);
}

function SettingsApp() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [hasToken, setHasToken] = useState(false);
  const [tokenInput, setTokenInput] = useState("");
  const [hiddenText, setHiddenText] = useState("");
  const [options, setOptions] = useState<DataSourceOption[]>([]);
  const [status, setStatus] = useState<Status>({ kind: "idle", text: "" });
  const busy = status.kind === "busy";

  useEffect(() => {
    invoke<SettingsView>("get_settings")
      .then((view) => {
        const { has_token, ...rest } = view;
        setSettings(rest);
        setHasToken(has_token);
        setHiddenText(rest.hidden_statuses.join(", "));
      })
      .catch((e) => setStatus({ kind: "error", text: String(e) }));
  }, []);

  if (!settings) {
    return <main className="page">{status.text || "Завантаження…"}</main>;
  }

  const current = (patch: Partial<Settings> = {}): Settings => ({
    ...settings,
    hidden_statuses: hiddenText
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean),
    ...patch,
  });

  const save = async (patch: Partial<Settings> = {}): Promise<Settings> => {
    const next = current(patch);
    await invoke("set_settings", { newSettings: next });
    setSettings(next);
    return next;
  };

  const run = async (busyText: string, action: () => Promise<string>) => {
    setStatus({ kind: "busy", text: busyText });
    try {
      setStatus({ kind: "ok", text: await action() });
    } catch (e) {
      setStatus({ kind: "error", text: String(e) });
    }
  };

  const saveToken = () =>
    run("Зберігаю токен…", async () => {
      await invoke("set_token", { token: tokenInput });
      const saved = tokenInput.trim() !== "";
      setTokenInput("");
      setHasToken(saved);
      return saved ? "Токен збережено." : "Токен видалено.";
    });

  const pick = (opt: DataSourceOption) => save({ data_source_id: opt.id, data_source_name: opt.name });

  const find = () =>
    run("Шукаю базу в Notion…", async () => {
      const found = await invoke<DataSourceOption[]>("resolve_data_source", {
        input: settings.database_input,
      });
      setOptions(found);
      if (found.length === 1) {
        await pick(found[0]);
        return `Знайдено: «${found[0].name || found[0].id}». Можна синхронізувати.`;
      }
      await save({ data_source_id: null, data_source_name: null });
      return `У бази ${found.length} джерела даних — виберіть потрібне нижче.`;
    });

  const sync = () =>
    run("Синхронізую…", async () => {
      await save();
      const r = await invoke<SyncResult>("sync_notion");
      log(`sync ok: ${r.total}`);
      return `Готово: ${r.total} карток з Notion, в оверлеї — ${r.visible}.`;
    });

  const saveAll = () =>
    run("Зберігаю…", async () => {
      await save();
      return "Налаштування збережено.";
    });

  const setMapping = (key: keyof Mapping, value: string) =>
    setSettings({ ...settings, mapping: { ...settings.mapping, [key]: value } });

  return (
    <main className="page">
      <h1>Word Overlay — налаштування</h1>

      <section>
        <h2>1. Токен Notion</h2>
        <p className="hint">
          Internal integration token з notion.so/my-integrations (доступ лише «Read content»).
        </p>
        <div className="row">
          <input
            type="password"
            autoComplete="off"
            placeholder={hasToken ? "Токен збережено — вставте новий, щоб замінити" : "ntn_…"}
            value={tokenInput}
            onChange={(e) => setTokenInput(e.target.value)}
          />
          <button onClick={saveToken} disabled={busy || tokenInput.trim() === ""}>
            Зберегти
          </button>
        </div>
        <p className={hasToken ? "badge ok" : "badge"}>
          {hasToken ? "Токен збережено" : "Токен ще не задано"}
        </p>
      </section>

      <section>
        <h2>2. База</h2>
        <p className="hint">Посилання на базу Notion або її ID (також підходить ID data source).</p>
        <div className="row">
          <input
            type="text"
            placeholder="https://www.notion.so/…"
            value={settings.database_input}
            onChange={(e) => setSettings({ ...settings, database_input: e.target.value })}
          />
          <button
            onClick={find}
            disabled={busy || !hasToken || settings.database_input.trim() === ""}
          >
            Знайти
          </button>
        </div>
        {options.length > 1 && (
          <select
            value={settings.data_source_id ?? ""}
            onChange={(e) => {
              const opt = options.find((o) => o.id === e.target.value);
              if (!opt) return;
              void run("Зберігаю…", async () => {
                await pick(opt);
                return `Вибрано «${opt.name || opt.id}». Можна синхронізувати.`;
              });
            }}
          >
            <option value="" disabled>
              Виберіть джерело даних…
            </option>
            {options.map((o) => (
              <option key={o.id} value={o.id}>
                {o.name || o.id}
              </option>
            ))}
          </select>
        )}
        <p className="hint">
          Вибрано:{" "}
          {settings.data_source_id
            ? `${settings.data_source_name || "без назви"} (${settings.data_source_id})`
            : "—"}
        </p>
      </section>

      <section>
        <h2>3. Що показувати</h2>
        <label className="field">
          <span>Не показувати статуси (через кому)</span>
          <input type="text" value={hiddenText} onChange={(e) => setHiddenText(e.target.value)} />
        </label>
        <details>
          <summary>Колонки Notion (для іншої бази)</summary>
          <p className="hint">
            Фраза — колонка типу title, знаходиться автоматично. Порожнє поле = не використовувати.
          </p>
          {MAPPING_FIELDS.map(([key, label]) => (
            <label className="field" key={key}>
              <span>{label}</span>
              <input
                type="text"
                value={settings.mapping[key]}
                onChange={(e) => setMapping(key, e.target.value)}
              />
            </label>
          ))}
        </details>
      </section>

      <section className="actions">
        <button className="primary" onClick={sync} disabled={busy || !hasToken || !settings.data_source_id}>
          Синхронізувати
        </button>
        <button onClick={saveAll} disabled={busy}>
          Зберегти налаштування
        </button>
      </section>

      <p className={`status ${status.kind}`} role="status">
        {status.text}
      </p>

      <footer>
        <span className="hint">Ctrl+Shift+F8 — відкрити це вікно · Ctrl+Shift+F9 — наступна фраза</span>
        <button className="quiet" onClick={() => void invoke("quit_app")}>
          Вийти з програми
        </button>
      </footer>
    </main>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <SettingsApp />
  </StrictMode>,
);
