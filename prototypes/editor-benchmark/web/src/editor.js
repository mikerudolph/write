import { defaultKeymap, history, historyKeymap, undo } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { defaultHighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorSelection, EditorState, RangeSetBuilder, StateField } from "@codemirror/state";
import { Decoration, drawSelection, EditorView, keymap, WidgetType } from "@codemirror/view";
import katex from "katex";
import "katex/dist/katex.min.css";
import mermaid from "mermaid";

mermaid.initialize({ startOnLoad: false, securityLevel: "strict" });

const estimatedHeights = { math: 80, table: 110, mermaid: 280 };
let diagramQueue = Promise.resolve();
let diagramCount = 0;
let view = null;
let field = null;
let notifyOnFrame = false;

const stats = { diagramMs: [], diagramFailures: 0, longTasks: [] };
if (window.PerformanceObserver?.supportedEntryTypes?.includes("longtask")) {
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries()) stats.longTasks.push(entry.duration);
  }).observe({ entryTypes: ["longtask"] });
}

const post = (message) => window.webkit.messageHandlers.bench.postMessage(message);
const nextFrame = () => new Promise((resolve) => requestAnimationFrame(resolve));

function renderTable(source, element) {
  const rows = source.trim().split("\n").filter((_, index) => index !== 1);
  const table = document.createElement("table");
  rows.forEach((row, rowIndex) => {
    const line = table.insertRow();
    row.replace(/^\||\|$/g, "").split("|").forEach((cell) => {
      const item = document.createElement(rowIndex === 0 ? "th" : "td");
      item.textContent = cell.trim();
      line.appendChild(item);
    });
  });
  element.appendChild(table);
}

function renderDiagram(source, element) {
  const body = source.replace(/^```mermaid\n/, "").replace(/\n?```\s*$/, "");
  diagramCount += 1;
  const id = `diagram-${diagramCount}`;
  diagramQueue = diagramQueue.then(async () => {
    if (!element.isConnected) return;
    const start = performance.now();
    const { svg } = await mermaid.render(id, body);
    element.innerHTML = svg;
    stats.diagramMs.push(performance.now() - start);
    view?.requestMeasure();
  }).catch(() => {
    stats.diagramFailures += 1;
  });
}

class RichWidget extends WidgetType {
  constructor(kind, source) {
    super();
    this.kind = kind;
    this.source = source;
  }
  eq(other) {
    return other.kind === this.kind && other.source === this.source;
  }
  get estimatedHeight() {
    return estimatedHeights[this.kind] ?? 100;
  }
  toDOM() {
    const element = document.createElement("div");
    element.className = "rich-block";
    if (this.kind === "math") {
      const tex = this.source.trim().replace(/^\$\$/, "").replace(/\$\$$/, "").trim();
      katex.render(tex, element, { displayMode: true, throwOnError: false });
    } else if (this.kind === "table") {
      renderTable(this.source, element);
    } else if (this.kind === "mermaid") {
      element.style.minHeight = `${this.estimatedHeight}px`;
      queueMicrotask(() => renderDiagram(this.source, element));
    }
    return element;
  }
}

function scanBlocks(text) {
  const found = [];
  const patterns = [
    ["math", /^\$\$\n[\s\S]*?\n\$\$$/gm],
    ["mermaid", /^```mermaid\n[\s\S]*?\n```$/gm],
    ["table", /^\|.*\|\n\|[ :|-]+\|(?:\n\|.*\|)*$/gm],
  ];
  for (const [kind, pattern] of patterns) {
    for (const match of text.matchAll(pattern)) {
      found.push({ kind, start: match.index, end: match.index + match[0].length });
    }
  }
  return found.sort((left, right) => left.start - right.start);
}

function decorations(text, blocks) {
  const builder = new RangeSetBuilder();
  for (const block of blocks) {
    const widget = new RichWidget(block.kind, text.slice(block.start, block.end));
    builder.add(block.start, block.end, Decoration.replace({ widget, block: true }));
  }
  return builder.finish();
}

function touchesBlock(value, transaction) {
  let touched = false;
  transaction.changes.iterChanges((from, to, inserted, insertedTo, text) => {
    if (text.length > 1 || (to > from)) touched = true;
    value.between(from, to, () => {
      touched = true;
    });
  });
  return touched;
}

function richField(text) {
  return StateField.define({
    create: () => decorations(text, scanBlocks(text)),
    update(value, transaction) {
      if (!transaction.docChanged) return value;
      if (touchesBlock(value, transaction)) {
        const current = transaction.state.doc.toString();
        return decorations(current, scanBlocks(current));
      }
      return value.map(transaction.changes);
    },
    provide: (field) => EditorView.decorations.from(field),
  });
}

const fontTheme = EditorView.theme({
  ".cm-scroller": { fontFamily: "-apple-system, sans-serif", fontSize: "15px", lineHeight: "1.5" },
});

const frameNotifier = EditorView.updateListener.of((update) => {
  if (update.docChanged && notifyOnFrame) {
    requestAnimationFrame(() => {
      const channel = new MessageChannel();
      const length = update.state.doc.length;
      channel.port1.onmessage = () => post(length);
      channel.port2.postMessage(0);
    });
  }
});

window.bench = {
  async load(text) {
    field = richField(text);
    view = new EditorView({
      parent: document.getElementById("editor"),
      state: EditorState.create({
        doc: text,
        extensions: [
          history(),
          drawSelection(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          markdown(),
          syntaxHighlighting(defaultHighlightStyle),
          EditorView.lineWrapping,
          fontTheme,
          field,
          frameNotifier,
        ],
      }),
    });
    view.focus();
    await nextFrame();
    await nextFrame();
    return view.state.doc.length;
  },
  async place(position) {
    view.dispatch({
      selection: EditorSelection.cursor(position),
      effects: EditorView.scrollIntoView(position, { y: "center" }),
    });
    view.focus();
    await nextFrame();
    await nextFrame();
    return true;
  },
  arm(enabled) {
    notifyOnFrame = enabled;
    return true;
  },
  length() {
    return view.state.doc.length;
  },
  async jumpToEnd() {
    const start = performance.now();
    const end = view.state.doc.length;
    view.dispatch({
      selection: EditorSelection.cursor(end),
      effects: EditorView.scrollIntoView(end, { y: "end" }),
    });
    await nextFrame();
    await nextFrame();
    return performance.now() - start;
  },
  async scrollRun(ticks, step) {
    view.scrollDOM.scrollTop = 0;
    await nextFrame();
    const deltas = [];
    let last = performance.now();
    for (let tick = 0; tick < ticks; tick += 1) {
      view.scrollDOM.scrollTop += step;
      await nextFrame();
      const now = performance.now();
      deltas.push(now - last);
      last = now;
    }
    return { deltas, scrollTop: view.scrollDOM.scrollTop };
  },
  selectionTest(from, to) {
    const countBlocks = () => {
      let count = 0;
      view.state.field(field).between(from, to + 1, () => {
        count += 1;
      });
      return count;
    };
    const expected = view.state.sliceDoc(from, to);
    const richBlocksBefore = countBlocks();
    view.dispatch({ selection: EditorSelection.range(from, to) });
    const selected = view.state.sliceDoc(view.state.selection.main.from, view.state.selection.main.to);
    const copyMatches = selected === expected;
    const lengthBefore = view.state.doc.length;
    view.dispatch(view.state.replaceSelection("X"));
    const replaced = view.state.doc.length === lengthBefore - (to - from) + 1;
    undo(view);
    const restored = view.state.sliceDoc(from, to) === expected;
    return { copyMatches, replaced, restored, richBlocksBefore, richBlocksAfterUndo: countBlocks() };
  },
  stats() {
    const sum = (values) => values.reduce((total, value) => total + value, 0);
    return {
      diagrams: stats.diagramMs.length,
      diagramMaxMs: Math.max(0, ...stats.diagramMs),
      diagramTotalMs: sum(stats.diagramMs),
      diagramFailures: stats.diagramFailures,
      longTasks: stats.longTasks.length,
      longTaskMaxMs: Math.max(0, ...stats.longTasks),
    };
  },
};
post("ready");
