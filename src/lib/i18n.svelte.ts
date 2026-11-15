import { en } from "./i18n-en";

export type Lang = "de" | "en";

function initial(): Lang {
  try {
    const saved = localStorage.getItem("lang");
    if (saved === "de" || saved === "en") return saved;
  } catch {
    // Storage unavailable: fall back to the system language.
  }
  return typeof navigator !== "undefined" && !navigator.language.toLowerCase().startsWith("de") ? "en" : "de";
}

class I18n {
  lang = $state<Lang>(initial());
}

export const i18n = new I18n();
if (typeof document !== "undefined") document.documentElement.lang = i18n.lang;

export function setLang(lang: Lang) {
  i18n.lang = lang;
  document.documentElement.lang = lang;
  try {
    localStorage.setItem("lang", lang);
  } catch {
    // Storage unavailable: the choice lasts for this session only.
  }
}

/**
 * Translates a UI text. The German text is the key, so a missing English
 * entry still shows German rather than a code. `{name}` placeholders are
 * filled from `params`.
 */
export function t(de: string, params?: Record<string, string | number>): string {
  let s = i18n.lang === "en" ? (en[de] ?? de) : de;
  if (params) s = s.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m));
  return s;
}

/** Locale for number and date formatting. */
export const locale = () => (i18n.lang === "en" ? "en-GB" : "de-DE");
