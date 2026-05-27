/**
 * Mock @tauri-apps/api/event for standalone browser testing.
 */

export type EventCallback<T> = (event: { payload: T }) => void;
export type UnlistenFn = () => void;

export async function listen<T>(
  _event: string,
  _handler: EventCallback<T>,
): Promise<UnlistenFn> {
  return () => {};
}
