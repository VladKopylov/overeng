import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, primaryMonitor } from "@tauri-apps/api/window";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import "./overlay.css";

type Card = {
  id: string;
  phrase: string;
  translation: string | null;
  pronunciation: string | null;
  description: string | null;
  examples: string[];
  synonym: string | null;
  situations: string[];
  kind: string | null;
  status: string | null;
  added: string | null;
  last_reviewed: string | null;
};

const SAMPLE_CARDS: Card[] = [
  {
    phrase: "Whether we like it or not, …",
    pronunciation: "/ˈweðər wi laɪk ɪt ɔːr nɑːt/",
    translation: "подобається нам чи ні, …",
  },
  {
    phrase: "It goes without saying",
    pronunciation: "/ɪt ɡoʊz wɪˈðaʊt ˈseɪɪŋ/",
    translation: "само собою зрозуміло",
  },
  {
    phrase: "To be on the same page",
    pronunciation: "/tə bi ɑːn ðə seɪm peɪdʒ/",
    translation: "розуміти одне одного, бути в курсі",
  },
].map((p, i) => ({
  ...p,
  id: `sample-${i}`,
  description: null,
  examples: [],
  synonym: null,
  situations: [],
  kind: null,
  status: null,
  added: null,
  last_reviewed: null,
}));

function shuffle<T>(items: T[]): T[] {
  const a = [...items];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

const MARGIN_PX = 16;
const TOPMOST_INTERVAL_MS = 5000;

function log(msg: string): void {
  invoke("debug_log", { msg }).catch((e) => console.error("debug_log failed", e));
}

async function positionBottomRight(): Promise<void> {
  const win = getCurrentWindow();
  const m = await primaryMonitor();
  if (!m) {
    log("position: primaryMonitor() = null");
    return;
  }
  const wa = m.workArea;
  const size = await win.outerSize();
  const x = wa.position.x + wa.size.width - size.width - MARGIN_PX;
  const y = wa.position.y + wa.size.height - size.height - MARGIN_PX;
  await win.setPosition(new PhysicalPosition(x, y));
  log(
    `position: monitor=${m.name ?? "?"} scale=${m.scaleFactor} ` +
      `workArea=${wa.position.x},${wa.position.y} ${wa.size.width}x${wa.size.height} ` +
      `window=${size.width}x${size.height} -> ${x},${y}`,
  );
}

async function reassertTopmost(): Promise<void> {
  try {
    await getCurrentWindow().setAlwaysOnTop(true);
  } catch (e) {
    log(`setAlwaysOnTop failed: ${String(e)}`);
  }
  try {
    await invoke("reassert_topmost");
  } catch (e) {
    log(`reassert_topmost failed: ${String(e)}`);
  }
}

async function loadCards(): Promise<Card[]> {
  try {
    const cards = await invoke<Card[]>("get_cards");
    if (cards.length > 0) {
      log(`get_cards: ${cards.length} карток`);
      return shuffle(cards);
    }
    log("get_cards: кеш порожній — показую запасні фрази");
  } catch (e) {
    log(`get_cards failed: ${String(e)} — показую запасні фрази`);
  }
  return SAMPLE_CARDS;
}

function Overlay() {
  const [cards, setCards] = useState<Card[]>(SAMPLE_CARDS);
  const [index, setIndex] = useState(0);
  const card = cards[index % cards.length];

  useEffect(() => {
    const win = getCurrentWindow();
    const cleanups: Array<() => void> = [];
    let disposed = false;
    const keep = (unlisten: () => void) => (disposed ? unlisten() : cleanups.push(unlisten));

    log("overlay: старт");
    win
      .setIgnoreCursorEvents(true)
      .then(() => log("setIgnoreCursorEvents(true) ok"))
      .catch((e) => log(`setIgnoreCursorEvents failed: ${String(e)}`));

    positionBottomRight().catch((e) => log(`position failed: ${String(e)}`));

    void reassertTopmost();
    const timer = window.setInterval(() => void reassertTopmost(), TOPMOST_INTERVAL_MS);
    cleanups.push(() => window.clearInterval(timer));

    let seq = 0;
    const reload = () => {
      const mine = ++seq;
      void loadCards().then((loaded) => {
        if (disposed || mine !== seq) return;
        setCards(loaded);
        setIndex(0);
      });
    };
    listen("cards-updated", () => {
      log("cards-updated -> перечитую картки");
      reload();
    })
      .then(keep)
      .catch((e) => log(`listen cards-updated failed: ${String(e)}`))
      .finally(() => {
        if (!disposed) reload();
      });

    listen("cycle-phrase", () => {
      setIndex((i) => i + 1);
    }).then(keep);

    win
      .onFocusChanged(({ payload: focused }) => {
        log(`focus changed: overlay focused=${focused}`);
        if (focused) void reassertTopmost();
      })
      .then(keep);

    return () => {
      disposed = true;
      cleanups.forEach((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (index !== 0) log(`cycle-phrase -> ${index % cards.length}`);
  }, [index, cards.length]);

  const isMistakeFix = card.kind?.toLowerCase() === "mistake fix";
  const example = card.examples[0];

  return (
    <div className={isMistakeFix ? "card mistake" : "card"}>
      {isMistakeFix && <div className="label">Mistake fix</div>}
      <div className="text">{card.phrase}</div>
      {card.pronunciation && <div className="pron">{card.pronunciation}</div>}
      {card.translation && <div className="tr">{card.translation}</div>}
      {example && <div className="ex">{example}</div>}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Overlay />
  </StrictMode>,
);
