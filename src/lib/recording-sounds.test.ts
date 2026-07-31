import { describe, expect, it, vi } from "vitest";
import { RecordingSoundPlayer, type AudioContextLike } from "./recording-sounds.js";

function makeAudioContext(initialState: AudioContextState = "suspended") {
  let state = initialState;
  const oscillatorStarts: number[] = [];
  const constantStarts: number[] = [];
  const constantStops: number[] = [];

  const parameter = {
    setValueAtTime: vi.fn(),
    linearRampToValueAtTime: vi.fn(),
  };
  const gain = {
    gain: parameter,
    connect: vi.fn(),
  };
  const context = {
    get state() {
      return state;
    },
    currentTime: 10,
    destination: {},
    resume: vi.fn(async () => {
      state = "running";
    }),
    close: vi.fn(async () => {
      state = "closed";
    }),
    createGain: vi.fn(() => gain),
    createConstantSource: vi.fn(() => ({
      connect: vi.fn(),
      start: vi.fn((when = 0) => constantStarts.push(when)),
      stop: vi.fn((when = 0) => constantStops.push(when)),
    })),
    createOscillator: vi.fn(() => ({
      type: "sine",
      frequency: parameter,
      connect: vi.fn(),
      start: vi.fn((when = 0) => oscillatorStarts.push(when)),
      stop: vi.fn(),
    })),
  } as unknown as AudioContextLike;

  return { context, oscillatorStarts, constantStarts, constantStops };
}

describe("RecordingSoundPlayer", () => {
  it("warms and keeps one silent output graph alive", async () => {
    const fake = makeAudioContext();
    const player = new RecordingSoundPlayer(() => fake.context);

    await Promise.all([player.warm(), player.warm()]);

    expect(fake.context.resume).toHaveBeenCalledTimes(1);
    expect(fake.context.createConstantSource).toHaveBeenCalledTimes(1);
    expect(fake.constantStarts).toEqual([10]);
  });

  it("resumes a suspended context before scheduling the start cue", async () => {
    const fake = makeAudioContext();
    const player = new RecordingSoundPlayer(() => fake.context);

    await player.playStart();

    expect(fake.context.resume).toHaveBeenCalledTimes(1);
    expect(fake.oscillatorStarts).toEqual([10]);
  });

  it("disposes the keep-alive source and audio context", async () => {
    const fake = makeAudioContext("running");
    const player = new RecordingSoundPlayer(() => fake.context);
    await player.warm();

    await player.dispose();

    expect(fake.constantStops).toEqual([10]);
    expect(fake.context.close).toHaveBeenCalledTimes(1);
  });
});
