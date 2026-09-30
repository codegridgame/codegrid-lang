import { run as runSuite } from './suite/index';

export async function run(): Promise<void> {
  await runSuite();
}
