/** Valid single-board examples for the new-file command and snippets. */

export interface CodeGridTemplate {
  id: string;
  label: string;
  detail: string;
  body: string;
}

export const TEMPLATES: CodeGridTemplate[] = [
  {
    id: 'main',
    label: 'Main board (5x5)',
    detail: 'A single Main Board with one entry',
    body: '@main\n@size 5x5\n~v _ _ _ _\n_  + > . _\n_  _ _ _ _\n_  _ _ _ _\n_  _ _ _ _\n@end\n',
  },
  {
    id: 'empty',
    label: 'Minimal board',
    detail: 'A one-row implicit Main Board with HALT',
    body: '~> ;\n',
  },
];

export function findTemplate(id: string): CodeGridTemplate | undefined {
  return TEMPLATES.find((template) => template.id === id);
}
