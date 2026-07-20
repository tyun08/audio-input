export type AudioContextLike = Pick<
  AudioContext,
  | "state"
  | "currentTime"
  | "destination"
  | "resume"
  | "close"
  | "createGain"
  | "createConstantSource"
  | "createOscillator"
>;

type AudioContextFactory = () => AudioContextLike;

/**
 * Keeps the output side of Web Audio ready without touching microphone input.
 * A connected zero-gain source prevents a hidden WKWebView from cold-starting
 * the audio graph only after the recording state event arrives.
 */
export class RecordingSoundPlayer {
  private context: AudioContextLike | null = null;
  private keepAlive: ConstantSourceNode | null = null;
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
    return this.playTone(880, 0.06, 0.08, 0.09);
  }

  playStop(): Promise<void> {
    return this.playTone(440, 0.09, 0.11, 0.12);
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
      await this.ensureRunning(context);
      if (!this.keepAlive && context.state === "running") {
        const source = context.createConstantSource();
        const gain = context.createGain();
        gain.gain.setValueAtTime(0, context.currentTime);
        source.connect(gain);
        gain.connect(context.destination);
        source.start(context.currentTime);
        this.keepAlive = source;
      }
    } catch {
      // Recording cues are non-critical; recording must still work if Web
      // Audio is unavailable or the platform refuses background playback.
    }
  }

  private async playTone(
    frequency: number,
    sustainUntil: number,
    fadeUntil: number,
    stopAt: number
  ): Promise<void> {
    try {
      const context = this.getContext();
      await this.ensureRunning(context);
      const now = context.currentTime;
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.connect(gain);
      gain.connect(context.destination);
      oscillator.type = "sine";
      oscillator.frequency.setValueAtTime(frequency, now);
      gain.gain.setValueAtTime(0, now);
      gain.gain.linearRampToValueAtTime(0.18, now + 0.005);
      gain.gain.setValueAtTime(0.18, now + sustainUntil);
      gain.gain.linearRampToValueAtTime(0, now + fadeUntil);
      oscillator.start(now);
      oscillator.stop(now + stopAt);
    } catch {
      // Non-critical — swallow Web Audio errors without affecting recording.
    }
  }
}
