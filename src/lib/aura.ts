import { convertFileSrc } from "@tauri-apps/api/core";
import { placeholderColors } from "./format";
import type { Game } from "./types";

/** Whether boards are tinted with their covers' colours (default on). */
export function boardAuraOn(): boolean {
  try {
    return localStorage.getItem("boardAura") !== "false";
  } catch {
    return true;
  }
}

export function setBoardAura(on: boolean) {
  try {
    localStorage.setItem("boardAura", String(on));
  } catch {
    // Storage unavailable: the choice lasts for this session only.
  }
}

type Rgb = [number, number, number];

const KEY = "auraCache";
let cache: Record<string, Rgb> | null = null;
const pending = new Map<string, Promise<Rgb>>();

function load(): Record<string, Rgb> {
  if (cache) return cache;
  try {
    cache = JSON.parse(localStorage.getItem(KEY) ?? "{}") ?? {};
  } catch {
    cache = {};
  }
  return cache!;
}

function store(key: string, rgb: Rgb) {
  const c = load();
  c[key] = rgb;
  try {
    localStorage.setItem(KEY, JSON.stringify(c));
  } catch {
    // Storage full or unavailable: keep it in memory.
  }
}

/** The colour a game's cover gives off, toned down so it stays pleasant. */
export function auraOf(game: Game): Promise<Rgb> {
  const key = game.coverPath ?? `name:${game.name}`;
  const hit = load()[key];
  if (hit) return Promise.resolve(hit);
  let p = pending.get(key);
  if (!p) {
    p = (game.coverPath ? sample(convertFileSrc(game.coverPath)) : Promise.resolve(null))
      .catch(() => null)
      .then((rgb) => soften(rgb ?? parseHsl(placeholderColors(game.name)[0])))
      .then((rgb) => {
        store(key, rgb);
        pending.delete(key);
        return rgb;
      });
    pending.set(key, p);
  }
  return p;
}

/** Average colour of the cover, weighted towards its vivid pixels. */
function sample(src: string): Promise<Rgb | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      try {
        const w = 24;
        const h = 36;
        const canvas = document.createElement("canvas");
        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext("2d", { willReadFrequently: true });
        if (!ctx) return resolve(null);
        ctx.drawImage(img, 0, 0, w, h);
        const d = ctx.getImageData(0, 0, w, h).data;
        let r = 0;
        let g = 0;
        let b = 0;
        let total = 0;
        for (let i = 0; i < d.length; i += 4) {
          const [pr, pg, pb] = [d[i], d[i + 1], d[i + 2]];
          const max = Math.max(pr, pg, pb);
          const min = Math.min(pr, pg, pb);
          const sat = max === 0 ? 0 : (max - min) / max;
          const light = (max + min) / 510;
          // Near-black and near-white pixels say little about the mood.
          const weight = 0.05 + sat * sat * (light > 0.08 && light < 0.92 ? 1 : 0.1);
          r += pr * weight;
          g += pg * weight;
          b += pb * weight;
          total += weight;
        }
        resolve(total ? [r / total, g / total, b / total] : null);
      } catch {
        resolve(null);
      }
    };
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

function parseHsl(s: string): Rgb {
  const m = s.match(/hsl\((\d+)\s+(\d+)%\s+(\d+)%\)/);
  return m ? hslToRgb(+m[1] / 360, +m[2] / 100, +m[3] / 100) : [90, 90, 120];
}

/** Caps saturation and keeps lightness in a calm middle band. */
function soften([r, g, b]: Rgb): Rgb {
  const [h, s, l] = rgbToHsl(r, g, b);
  return hslToRgb(h, Math.min(s, 0.6), Math.min(Math.max(l, 0.42), 0.58)).map(Math.round) as Rgb;
}

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h / 6, s, l];
}

function hslToRgb(h: number, s: number, l: number): Rgb {
  const f = (n: number) => {
    const k = (n + h * 12) % 12;
    const a = s * Math.min(l, 1 - l);
    return 255 * (l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1)));
  };
  return [f(0), f(8), f(4)];
}

/** `rgb(r g b / a)` for CSS. */
export const rgba = ([r, g, b]: Rgb, a: number) => `rgb(${r} ${g} ${b} / ${a})`;

/** A soft left-to-right wash of the field's games, by column. */
export function fieldWash(cols: number, colours: { col: number; rgb: Rgb }[], alpha = 0.16): string | null {
  if (!colours.length) return null;
  const byCol = new Map<number, Rgb[]>();
  for (const c of colours) byCol.set(c.col, [...(byCol.get(c.col) ?? []), c.rgb]);
  const stops = [...byCol.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([col, list]) => {
      const avg = list.reduce((s, c) => [s[0] + c[0], s[1] + c[1], s[2] + c[2]], [0, 0, 0] as Rgb).map((v) => Math.round(v / list.length)) as Rgb;
      return `${rgba(avg, alpha)} ${Math.round(((col + 0.5) / cols) * 100)}%`;
    });
  if (stops.length === 1) stops.push(stops[0]);
  return `linear-gradient(90deg, ${stops.join(", ")})`;
}
