/** Shared presentation data; generated from resources/codegrid-error-messages.json. */
export const errorCatalog: {
  locales: string[];
  fallback: Record<string, string>;
  errors: Record<string, { layer: string; code: string | number; messages: Record<string, string> }>;
} = require('../../l10n/codegrid-error-messages.json');

export function normalizeErrorLocale(locale: string): string {
  const normalized = locale.toLowerCase().replace(/_/g, '-');
  if (normalized === 'zh' || normalized.startsWith('zh-')) {
    return normalized.split('-').some(part => ['hant', 'tw', 'hk', 'mo'].includes(part)) ? 'zh-tw' : 'zh-cn';
  }
  const base = normalized.split('-')[0];
  if (base === 'pt') return 'pt-br';
  return errorCatalog.locales.includes(base) ? base : 'en';
}

/** Unknown numbers use the shared generic fallback, never another error's text. */
export function errorMessage(number: string, locale: string): string {
  const language = normalizeErrorLocale(locale);
  return errorCatalog.errors[number]?.messages[language] ?? errorCatalog.fallback[language];
}

export function errorMessageByCode(layer: string, code: string, locale: string): string {
  const entry = Object.values(errorCatalog.errors).find(row => row.layer === layer && String(row.code) === code);
  const language = normalizeErrorLocale(locale);
  return entry?.messages[language] ?? errorCatalog.fallback[language];
}
