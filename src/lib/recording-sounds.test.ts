import { describe, expect, it, vi } from "vitest";
import { RecordingSoundPlayer, type AudioContextLike } from "./recording-sounds.js";

function makeAudioContext(initialState: AudioContextState = "suspended", deferResume = false) {
  let state = initialState;
  const oscillatorStarts: number[] = [];
  const oscillatorStops: number[] = [];
  const gainSetValues: number[] = [];
  const gainRampValues: number[] = [];
  let resolveResume: (() => void) | null = null;

  const frequencyParameter = {
    setValueAtTime: vi.fn(),
    linearRampToValueAtTime: vi.fn(),
  };
  const context = {
    get state() {
      return state;
    },
    currentTime: 10,
    destination: {},
    resume: vi.fn(() => {
      if (!deferResume) {
        state = "running";
        return Promise.resolve();
      }
      return new Promise<void>((resolve) => {
        resolveResume = () => {
          state = "running";
          resolve();
        };
      });
    }),
    close: vi.fn(async () => {
      state = "closed";
    }),
    createGain: vi.fn(() => ({
      gain: {
        setValueAtTime: vi.fn((value: number) => gainSetValues.push(value)),
        linearRampToValueAtTime: vi.fn((value: number) => gainRampValues.push(value)),
      },
      connect: vi.fn(),
    })),
    createOscillator: vi.fn(() => ({
      type: "sine",
      frequency: frequencyParameter,
      connect: vi.fn(),
      start: vi.fn((when = 0) => oscillatorStarts.push(when)),
      stop: vi.fn((when = 0) => oscillatorStops.push(when)),
    })),
  } as unknown as AudioContextLike;

  return {
    context,
    oscillatorStarts,
    oscillatorStops,
    gainSetValues,
    gainRampValues,
    resolveResume: () => resolveResume?.(),
  };
}

describe("RecordingSoundPlayer", () => {
  it("warms and keeps one inaudible non-zero output graph alive", async () => {
    const fake = makeAudioContext();
    const player = new RecordingSoundPlayer(() => fake.context);

    await Promise.all([player.warm(), player.warm()]);

    expect(fake.context.resume).toHaveBeenCalledTimes(1);
    expect(fake.context.createOscillator).toHaveBeenCalledTimes(1);
    expect(fake.oscillatorStarts).toEqual([10]);
    expect(fake.gainSetValues).toContain(0.00001);
  });

  it("queues the start cue before a suspended context finishes resuming", async () => {
    const fake = makeAudioContext("suspended", true);
    const player = new RecordingSoundPlayer(() => fake.context);

    const playing = player.playStart();

    expect(fake.context.resume).toHaveBeenCalledTimes(1);
    expect(fake.oscillatorStarts).toEqual([10]);
    fake.resolveResume();
    await playing;
  });

  it("uses a louder peak gain for the stop cue", async () => {
    const fake = makeAudioContext("running");
    const player = new RecordingSoundPlayer(() => fake.context);

    await player.playStart();
    await player.playStop();

    expect(fake.gainRampValues.filter((value) => value > 0)).toEqual([0.18, 0.26]);
  });

  it("disposes the keep-alive source and audio context", async () => {
    const fake = makeAudioContext("running");
    const player = new RecordingSoundPlayer(() => fake.context);
    await player.warm();

    await player.dispose();

    expect(fake.oscillatorStops).toEqual([10]);
    expect(fake.context.close).toHaveBeenCalledTimes(1);
  });
});
