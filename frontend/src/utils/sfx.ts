// sfx.ts — Synthesized sound kit (§6.13). No audio assets — pure WebAudio.
//
// Off by default. A masthead toggle enables it. Each sound is a tiny synthesis
// routine. The module exports a singleton `sfx` used at key interaction points.
//
// Mobile: navigator.vibrate is also triggered for stamp and file on supported devices.

class SfxKit {
  private ctx: AudioContext | null = null;
  private _enabled = false;

  private getCtx(): AudioContext {
    if (!this.ctx) this.ctx = new AudioContext();
    if (this.ctx.state === 'suspended') void this.ctx.resume();
    return this.ctx;
  }

  enable()     { this._enabled = true; }
  disable()    { this._enabled = false; }
  isEnabled()  { return this._enabled; }
  toggle()     { this._enabled = !this._enabled; return this._enabled; }

  // Soft tick on row hover
  tick() {
    if (!this._enabled) return;
    const ctx = this.getCtx();
    const osc  = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.frequency.value = 880;
    const t = ctx.currentTime;
    gain.gain.setValueAtTime(0.025, t);
    gain.gain.exponentialRampToValueAtTime(0.001, t + 0.035);
    osc.start(t);
    osc.stop(t + 0.035);
  }

  // Brief click on navigation
  click() {
    if (!this._enabled) return;
    const ctx = this.getCtx();
    const osc  = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.frequency.value = 660;
    const t = ctx.currentTime;
    gain.gain.setValueAtTime(0.04, t);
    gain.gain.exponentialRampToValueAtTime(0.001, t + 0.05);
    osc.start(t);
    osc.stop(t + 0.05);
  }

  // Rising chime on acknowledge
  ack() {
    if (!this._enabled) return;
    const ctx = this.getCtx();
    [440, 550, 660].forEach((freq, i) => {
      const osc  = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = freq;
      const t = ctx.currentTime + i * 0.065;
      gain.gain.setValueAtTime(0.07, t);
      gain.gain.exponentialRampToValueAtTime(0.001, t + 0.18);
      osc.start(t);
      osc.stop(t + 0.2);
    });
    navigator.vibrate?.([15]);
  }

  // Noise burst + low thunk on stamp press
  stamp() {
    if (!this._enabled) return;
    const ctx = this.getCtx();
    // Noise burst — paper impact
    const buf  = ctx.createBuffer(1, Math.floor(ctx.sampleRate * 0.1), ctx.sampleRate);
    const data = buf.getChannelData(0);
    for (let i = 0; i < data.length; i++) data[i] = (Math.random() * 2 - 1);
    const src    = ctx.createBufferSource();
    const filt   = ctx.createBiquadFilter();
    const gainN  = ctx.createGain();
    src.buffer   = buf;
    filt.type    = 'lowpass';
    filt.frequency.value = 400;
    src.connect(filt);
    filt.connect(gainN);
    gainN.connect(ctx.destination);
    const t = ctx.currentTime;
    gainN.gain.setValueAtTime(0.18, t);
    gainN.gain.exponentialRampToValueAtTime(0.001, t + 0.11);
    src.start(t);
    // Low thunk — rubber stamp weight
    const osc  = ctx.createOscillator();
    const gainT = ctx.createGain();
    osc.connect(gainT);
    gainT.connect(ctx.destination);
    osc.frequency.setValueAtTime(130, t);
    osc.frequency.exponentialRampToValueAtTime(45, t + 0.18);
    gainT.gain.setValueAtTime(0.28, t);
    gainT.gain.exponentialRampToValueAtTime(0.001, t + 0.22);
    osc.start(t);
    osc.stop(t + 0.25);
    navigator.vibrate?.([25]);
  }

  // Sawtooth whoosh on file fly-up
  file() {
    if (!this._enabled) return;
    const ctx  = this.getCtx();
    const osc  = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.type = 'sawtooth';
    const t = ctx.currentTime;
    osc.frequency.setValueAtTime(220, t);
    osc.frequency.exponentialRampToValueAtTime(55, t + 0.38);
    gain.gain.setValueAtTime(0.08, t);
    gain.gain.exponentialRampToValueAtTime(0.001, t + 0.42);
    osc.start(t);
    osc.stop(t + 0.45);
    navigator.vibrate?.([10, 40, 10]);
  }

  // Open a new page/route
  open() {
    if (!this._enabled) return;
    const ctx  = this.getCtx();
    const osc  = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.frequency.value = 330;
    const t = ctx.currentTime;
    gain.gain.setValueAtTime(0.04, t);
    gain.gain.exponentialRampToValueAtTime(0.001, t + 0.06);
    osc.start(t);
    osc.stop(t + 0.06);
  }
}

export const sfx = new SfxKit();
