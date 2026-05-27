/**
 * Mock @tauri-apps/api/window for standalone browser testing.
 */

const mockWindow = {
  startDragging: async () => {},
  show: async () => {},
  hide: async () => {},
  setSize: async () => {},
  setPosition: async () => {},
  center: async () => {},
  outerPosition: async () => ({ x: 0, y: 0 }),
  scaleFactor: async () => 1,
  setResizable: async () => {},
};

export function getCurrentWindow() {
  return mockWindow;
}

export { mockWindow };
