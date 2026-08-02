export type AudioContextLike = Pick<
  AudioContext,
  "state" | "currentTime" | "destination" | "resume" | "close" | "createGain" | "createOscillator"
>;

type AudioContextFactory = () => AudioContextLike;

const OUTPUT_KEEP_ALIVE_GAIN = 0.00001;
const OUTPUT_KEEP_ALIVE_FREQUENCY_HZ = 20;
const START_CUE_GAIN = 0.18;
const STOP_CUE_GAIN = 0.26;

/**
 * Keeps the output side of Web Audio ready without touching microphone input.
 * A connected, inaudible non-zero oscillator prevents WebKit/CoreAudio from
 * optimizing the graph to silence and putting the output device back to sleep.
 */
export class RecordingSoundPlayer {
  private context: AudioContextLike | null = null;
  private keepAlive: OscillatorNode | null = null;
  private warming: Promise<void> | null = null;

  constructor(
    private readonly createContext: AudioContextFactory = () =>
      new AudioContext({ latencyHint: "interactive" })
  ) {}

  warm(): Promise<void> {
    if (!this.warming) {
      this.warming = this.doWarm().finally(() => {
        this.warming = null;
      });
    }
    return this.warming;
  }

  playStart(): Promise<void> {
    return this.playTone(880, START_CUE_GAIN, 0.06, 0.08, 0.09);
  }

  playStop(): Promise<void> {
    return this.playTone(440, STOP_CUE_GAIN, 0.09, 0.11, 0.12);
  }

  async dispose(): Promise<void> {
    const context = this.context;
    if (!context) return;

    try {
      this.keepAlive?.stop(context.currentTime);
    } catch {}
    this.keepAlive = null;
    this.context = null;

    try {
      await context.close();
    } catch {}
  }

  private getContext(): AudioContextLike {
    if (!this.context || this.context.state === "closed") {
      this.context = this.createContext();
      this.keepAlive = null;
    }
    return this.context;
  }

  private async ensureRunning(context: AudioContextLike): Promise<void> {
    if (context.state !== "running") {
      await context.resume();
    }
  }

  private async doWarm(): Promise<void> {
    try {
      const context = this.getContext();
      if (!this.keepAlive) {
        const source = context.createOscillator();
        const gain = context.createGain();
        source.type = "sine";
        source.frequency.setValueAtTime(OUTPUT_KEEP_ALIVE_FREQUENCY_HZ, context.currentTime);
        gain.gain.setValueAtTime(OUTPUT_KEEP_ALIVE_GAIN, context.currentTime);
        source.connect(gain);
        gain.connect(context.destination);
        source.start(context.currentTime);
        this.keepAlive = source;
      }
      await this.ensureRunning(context);
    } catch {
      // Recording cues are non-critical; recording must still work if Web
      // Audio is unavailable or the platform refuses background playback.
    }
  }

  private async playTone(
    frequency: number,
    peakGain: number,
    sustainUntil: number,
    fadeUntil: number,
    stopAt: number
  ): Promise<void> {
    try {
      const context = this.getContext();
      const now = context.currentTime;
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.connect(gain);
      gain.connect(context.destination);
      oscillator.type = "sine";
      oscillator.frequency.setValueAtTime(frequency, now);
      gain.gain.setValueAtTime(0, now);
      gain.gain.linearRampToValueAtTime(peakGain, now + 0.005);
      gain.gain.setValueAtTime(peakGain, now + sustainUntil);
      gain.gain.linearRampToValueAtTime(0, now + fadeUntil);
      oscillator.start(now);
      oscillator.stop(now + stopAt);

      // Queue the cue before resuming. In a background WKWebView, the
      // continuation after await context.resume() can be throttled for hundreds
      // of milliseconds even though the audio graph is ready sooner. A cue
      // scheduled at the suspended context's currentTime starts immediately
      // when WebAudio resumes without waiting for that JavaScript continuation.
      await this.ensureRunning(context);
    } catch {
      // Non-critical — swallow Web Audio errors without affecting recording.
    }
  }
}
