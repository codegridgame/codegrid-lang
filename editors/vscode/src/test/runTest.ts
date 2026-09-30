import * as path from 'path';
import { runTests } from '@vscode/test-electron';

async function main(): Promise<void> {
  const extensionDevelopmentPath = path.resolve(__dirname, '../..');
  const extensionTestsPath = path.resolve(__dirname, './index');
  const locale = process.env.CODEGRID_TEST_LOCALE;
  const localeArgs = locale ? [
    '--locale=' + locale,
    '--extensions-dir=' + path.join(extensionDevelopmentPath, '.vscode-test/localization-extensions'),
    '--user-data-dir=' + path.join(extensionDevelopmentPath, '.vscode-test/localization-user'),
  ] : ['--disable-extensions'];
  await runTests({
    cachePath: path.join(extensionDevelopmentPath, '.vscode-test'),
    extensionDevelopmentPath,
    extensionTestsPath,
    launchArgs: [extensionDevelopmentPath, '--disable-gpu', ...localeArgs],
  });
}

main().catch((err) => {
  console.error('Failed to run tests:', err);
  process.exitCode = 1;
});
