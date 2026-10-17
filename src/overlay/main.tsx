import { StrictMode, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent, RefObject } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import "./overlay.css";

type Rect = { x: number; y: number; width: number; height: number };

type Layout = { visible: boolean; expanded: boolean; hide_translation: boolean };

type Drag = {
  pointerId: number;
  startX: number;
  startY: number;
  ratio: number;
  origin: { x: number; y: number } | null;
  pending: { x: number; y: number } | null;
  frame: number;
};

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

const TOPMOST_INTERVAL_MS = 5000;
const EDIT_IDLE_MS = 60_000;
const MIN_FONT_SCALE = 0.7;
const FONT_SCALE_STEP = 0.05;

function log(msg: string): void {
  invoke("debug_log", { msg }).catch((e) => console.error("debug_log failed", e));
}

let queue: Promise<void> = Promise.resolve();

function serial(task: () => Promise<void>): Promise<void> {
  const next = queue.then(task).catch((e) => log(`overlay task failed: ${String(e)}`));
  queue = next;
  return next;
}

async function applyRect(fromCurrent: boolean): Promise<void> {
  const win = getCurrentWindow();
  const r = await invoke<Rect>("overlay_rect", { fromCurrent });
  const position = new PhysicalPosition(r.x, r.y);
  await win.setPosition(position);
  await win.setSize(new PhysicalSize(r.width, r.height));
  await win.setPosition(position);
}

function placeOverlay(fromCurrent: boolean): Promise<void> {
  return applyRect(fromCurrent).catch((e) => log(`position failed: ${String(e)}`));
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

function useFitText(ref: RefObject<HTMLDivElement | null>, deps: unknown[]): void {
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const overflows = () => el.scrollHeight > el.clientHeight + 1;
    const fit = () => {
      let scale = 1;
      delete el.dataset.tight;
      el.style.setProperty("--font-scale", "1");
      while (overflows() && scale > MIN_FONT_SCALE) {
        scale = Math.max(MIN_FONT_SCALE, scale - FONT_SCALE_STEP);
        el.style.setProperty("--font-scale", scale.toFixed(2));
      }
      if (overflows()) el.dataset.tight = "1";
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(el);
    return () => observer.disconnect();
  }, deps);
}

function Overlay() {
  const [cards, setCards] = useState<Card[]>(SAMPLE_CARDS);
  const [index, setIndex] = useState(0);
  const [visible, setVisible] = useState(true);
  const [expanded, setExpanded] = useState(false);
  const [editing, setEditing] = useState(false);
  const [hideTranslation, setHideTranslation] = useState(false);
  const [revealed, setRevealed] = useState(false);
  const expandedRef = useRef(false);
  const editingRef = useRef(false);
  const idleTimer = useRef(0);
  const drag = useRef<Drag | null>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const card = cards[index % cards.length];
  const translationShown = hideTranslation !== revealed;

  useFitText(cardRef, [card, expanded, editing, visible, translationShown]);

  const bumpIdle = () => {
    if (!editingRef.current) return;
    window.clearTimeout(idleTimer.current);
    idleTimer.current = window.setTimeout(() => {
      getCurrentWindow()
        .setIgnoreCursorEvents(true)
        .catch((e) => log(`idle click-through failed: ${String(e)}`));
      void serial(() => editHandlers.current.leaveEdit("бездіяльність 60 с", true));
    }, EDIT_IDLE_MS);
  };

  const enterEdit = async () => {
    if (editingRef.current) return;
    const win = getCurrentWindow();
    editingRef.current = true;
    setEditing(true);
    bumpIdle();
    try {
      await win.setIgnoreCursorEvents(false);
      await win.setResizable(true);
      log("edit mode: увімкнено (click-through вимкнено)");
    } catch (e) {
      log(`edit mode: помилка входу: ${String(e)}`);
    }
  };

  const cancelDrag = () => {
    const state = drag.current;
    if (state) {
      if (state.frame) window.cancelAnimationFrame(state.frame);
      state.frame = 0;
      state.pending = null;
    }
    drag.current = null;
  };

  const leaveEdit = async (reason: string, save: boolean) => {
    if (!editingRef.current) return;
    const win = getCurrentWindow();
    editingRef.current = false;
    setEditing(false);
    window.clearTimeout(idleTimer.current);
    cancelDrag();
    try {
      await win.setResizable(false);
      if (save) await invoke("save_overlay_rect", { expanded: expandedRef.current });
    } catch (e) {
      log(`edit mode: помилка збереження: ${String(e)}`);
    } finally {
      try {
        await win.setIgnoreCursorEvents(true);
        log(`edit mode: вимкнено (${reason}, збереження: ${save}), click-through увімкнено`);
      } catch (e) {
        log(`edit mode: setIgnoreCursorEvents(true) failed: ${String(e)}`);
      }
    }
  };

  const editHandlers = useRef({ enterEdit, leaveEdit, bumpIdle });
  editHandlers.current = { enterEdit, leaveEdit, bumpIdle };

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

    void reassertTopmost();
    const timer = window.setInterval(() => void reassertTopmost(), TOPMOST_INTERVAL_MS);
    cleanups.push(() => window.clearInterval(timer));
    cleanups.push(() => window.clearTimeout(idleTimer.current));

    let seq = 0;
    const reload = () => {
      const mine = ++seq;
      void loadCards().then((loaded) => {
        if (disposed || mine !== seq) return;
        setCards(loaded);
        setIndex(0);
        setRevealed(false);
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
      setRevealed(false);
    }).then(keep);

    listen("toggle-translation", () => {
      setRevealed((r) => !r);
      log("toggle-translation");
    }).then(keep);

    const layoutListeners = [
      listen<boolean>("overlay-expanded", ({ payload }) => {
        void serial(async () => {
          if (payload === expandedRef.current) return;
          if (editingRef.current) {
            await invoke("save_overlay_rect", { expanded: expandedRef.current }).catch((e) =>
              log(`save_overlay_rect failed: ${String(e)}`),
            );
          }
          expandedRef.current = payload;
          setExpanded(payload);
          log(`overlay-expanded -> ${payload}`);
          await placeOverlay(true);
        });
      }).then(keep),
      listen<boolean>("overlay-visible", ({ payload }) => {
        setVisible(payload);
        log(`overlay-visible -> ${payload}`);
      }).then(keep),
      listen<boolean>("overlay-hide-translation", ({ payload }) => {
        setHideTranslation(payload);
        setRevealed(false);
        log(`overlay-hide-translation -> ${payload}`);
      }).then(keep),
    ];

    Promise.allSettled(layoutListeners).finally(() => {
      if (disposed) return;
      void serial(async () => {
        try {
          const layout = await invoke<Layout>("get_layout");
          if (disposed) return;
          expandedRef.current = layout.expanded;
          setExpanded(layout.expanded);
          setVisible(layout.visible);
          setHideTranslation(layout.hide_translation);
          log(
            `layout: visible=${layout.visible} expanded=${layout.expanded} hide_translation=${layout.hide_translation}`,
          );
        } catch (e) {
          log(`get_layout failed: ${String(e)}`);
        }
        if (!disposed) await placeOverlay(false);
      });
    });

    listen("toggle-edit", () => {
      void serial(() => {
        const h = editHandlers.current;
        return editingRef.current ? h.leaveEdit("Ctrl+Shift+F12", true) : h.enterEdit();
      });
    }).then(keep);

    listen("layout-reset", () => {
      void serial(async () => {
        log("layout-reset -> ставлю оверлей заново");
        await editHandlers.current.leaveEdit("скидання позиції", false);
        await placeOverlay(false);
      });
    }).then(keep);

    win.onMoved(() => editHandlers.current.bumpIdle()).then(keep);
    win.onResized(() => editHandlers.current.bumpIdle()).then(keep);

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

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!editingRef.current || e.button !== 0) return;
    bumpIdle();
    e.currentTarget.setPointerCapture(e.pointerId);
    const state: Drag = {
      pointerId: e.pointerId,
      startX: e.screenX,
      startY: e.screenY,
      ratio: window.devicePixelRatio || 1,
      origin: null,
      pending: null,
      frame: 0,
    };
    drag.current = state;
    getCurrentWindow()
      .outerPosition()
      .then((p) => {
        if (drag.current === state) state.origin = { x: p.x, y: p.y };
      })
      .catch((err) => log(`drag: outerPosition failed: ${String(err)}`));
    log("drag: старт");
  };

  const endDrag = (e: ReactPointerEvent<HTMLDivElement>) => {
    const state = drag.current;
    if (!state || state.pointerId !== e.pointerId) return;
    drag.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    log(`drag: кінець${state.pending ? ` -> ${state.pending.x},${state.pending.y}` : ""}`);
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    bumpIdle();
    const state = drag.current;
    if (!state || state.pointerId !== e.pointerId) return;
    if ((e.buttons & 1) === 0) {
      endDrag(e);
      return;
    }
    if (!state.origin) return;
    state.pending = {
      x: Math.round(state.origin.x + (e.screenX - state.startX) * state.ratio),
      y: Math.round(state.origin.y + (e.screenY - state.startY) * state.ratio),
    };
    if (state.frame) return;
    state.frame = window.requestAnimationFrame(() => {
      state.frame = 0;
      const target = state.pending;
      if (!target || !editingRef.current) return;
      void getCurrentWindow()
        .setPosition(new PhysicalPosition(target.x, target.y))
        .catch((err) => log(`drag: setPosition failed: ${String(err)}`));
    });
  };

  const shown = visible || editing;
  const isMistakeFix = card.kind?.toLowerCase() === "mistake fix";
  const classes = ["card"];
  if (isMistakeFix) classes.push("mistake");
  if (expanded) classes.push("expanded");
  if (editing) classes.push("editing");

  return (
    <div className={shown ? "stage" : "stage hidden"}>
      {shown && (
        <div
          ref={cardRef}
          className={classes.join(" ")}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          onLostPointerCapture={endDrag}
        >
          {isMistakeFix && <div className="label">Mistake fix</div>}
          <div className="text">{card.phrase}</div>
          {card.pronunciation && <div className="pron">{card.pronunciation}</div>}
          {card.translation &&
            (translationShown ? (
              <div className="tr">{card.translation}</div>
            ) : (
              <div className="tr masked">переклад · Ctrl+Shift+F7</div>
            ))}
          {expanded ? (
            <>
              {card.synonym && <div className="syn">≈ {card.synonym}</div>}
              {card.description && <div className="desc">{card.description}</div>}
              {card.examples.map((ex, i) => (
                <div className="ex" key={i}>
                  {ex}
                </div>
              ))}
            </>
          ) : (
            card.examples[0] && <div className="ex">{card.examples[0]}</div>
          )}
          {editing && <div className="edit-hint">Перетягніть картку. Ctrl+Shift+F12 — готово</div>}
        </div>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Overlay />
  </StrictMode>,
);
