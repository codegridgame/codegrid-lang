import * as fs from 'fs';
import * as path from 'path';
import Mocha from 'mocha';

export function run(): Promise<void> {
  const mocha = new Mocha({ grep: process.env.CODEGRID_TEST_LOCALE ? /^Localization/ : undefined, ui: 'tdd', color: true, timeout: 30000 });
  return new Promise((resolve, reject) => {
    const files = fs
      .readdirSync(__dirname)
      .filter((name) => name.endsWith('.test.js'))
      .filter((name) => !process.env.CODEGRID_TEST_LOCALE || name === 'localization.test.js')
      .sort();
    for (const file of files) {
      mocha.addFile(path.resolve(__dirname, file));
    }
    mocha.run((failures) => {
      if (failures > 0) {
        reject(new Error(`${failures} test(s) failed.`));
      } else {
        resolve();
      }
    });
  });
}
