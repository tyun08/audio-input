/**
 * Mock @tauri-apps/api/app for standalone browser testing.
 */

let mockVersion = "0.4.10-standalone";

export async function getVersion(): Promise<string> {
  return mockVersion;
}

export function setMockVersion(v: string): void {
  mockVersion = v;
}
