// ─────────────────────────────────────────────────────────────────────────────
// PasswordGenerator.tsx — interactive password / passphrase generator.
//
// Two modes:
//   • Random     — charset-based, length 8–48, toggleable character classes.
//   • Passphrase — word-based (curated EFF-subset wordlist), word count 3–8.
//
// Entropy source: crypto.getRandomValues() — never Math.random().
// The generated password is pure client-side. It only leaves the device when
// the user explicitly pastes it into a form and submits.
//
// Props:
//   onUse(pw)   — called when the "Use this" button is clicked.
//   compact     — hides the settings panel (for tight spaces).
// ─────────────────────────────────────────────────────────────────────────────

import { useCallback, useMemo, useState } from 'react';
import '../styles/v2.css';

// ── EFF-subset wordlist (250 common, memorable English words) ─────────────────
const WORDLIST: readonly string[] = [
  'anchor', 'angle', 'ankle', 'anvil', 'apple', 'apron', 'arrow', 'atlas',
  'attic', 'badge', 'barge', 'basin', 'batch', 'beacon', 'bench', 'birch',
  'blade', 'blast', 'blend', 'block', 'bloom', 'board', 'bolt', 'bond',
  'boost', 'brake', 'branch', 'brick', 'bridge', 'brush', 'bundle', 'cabin',
  'cable', 'canal', 'cedar', 'chain', 'chalk', 'chart', 'cider', 'clamp',
  'cleft', 'cliff', 'clock', 'cloth', 'cloud', 'coast', 'cobalt', 'comet',
  'coral', 'crane', 'crest', 'crisp', 'crown', 'crust', 'curve', 'cycle',
  'delta', 'depth', 'digit', 'ditch', 'dodge', 'draft', 'drift', 'drone',
  'dusk', 'eagle', 'earth', 'elder', 'ember', 'epoch', 'fence', 'ferry',
  'field', 'flame', 'flint', 'flood', 'floor', 'focus', 'forge', 'frost',
  'gauge', 'globe', 'grain', 'grant', 'grove', 'guard', 'gulch', 'haven',
  'hedge', 'hinge', 'holly', 'honey', 'hover', 'ivory', 'joint', 'kayak',
  'knack', 'lance', 'larch', 'laser', 'latch', 'ledge', 'level', 'liver',
  'lodge', 'loft', 'lunar', 'manor', 'maple', 'march', 'marsh', 'mast',
  'meadow', 'merit', 'mosaic', 'mount', 'mulch', 'nerve', 'noble', 'notch',
  'orbit', 'oxide', 'panel', 'patch', 'pearl', 'pedal', 'petal', 'pilot',
  'pixel', 'plank', 'plumb', 'polar', 'porch', 'prism', 'probe', 'prune',
  'quartz', 'quest', 'quote', 'radix', 'rally', 'raven', 'realm', 'reef',
  'relay', 'ridge', 'rivet', 'roost', 'rustic', 'salvo', 'scout', 'shaft',
  'shale', 'shelf', 'shift', 'shrub', 'skiff', 'slate', 'sleet', 'slope',
  'snare', 'solar', 'spire', 'sprig', 'staff', 'stalk', 'stamp', 'stave',
  'steel', 'stern', 'stone', 'storm', 'stout', 'strap', 'straw', 'stride',
  'surge', 'swamp', 'swarm', 'swift', 'swirl', 'talon', 'tempo', 'thorn',
  'tidal', 'tiger', 'torch', 'tower', 'trace', 'track', 'trail', 'trout',
  'trunk', 'tulip', 'turret', 'twig', 'ultra', 'umber', 'valor', 'vault',
  'verge', 'vigor', 'viola', 'vista', 'vortex', 'walnut', 'wedge', 'wheat',
  'wheel', 'whirl', 'widget', 'wisp', 'world', 'wrath', 'wrench', 'yield',
  'zenith', 'abbot', 'acorn', 'agave', 'algae', 'almond', 'amber', 'amble',
  'amend', 'ample', 'annex', 'antic', 'argon', 'aroid', 'aspen', 'atlas',
  'audit', 'avian', 'axiom', 'azure', 'basil', 'bathe', 'blend', 'bloom',
  'blunt', 'blurb', 'boxer', 'braid', 'brass', 'briar', 'brine', 'brisk',
  'broth', 'brunt', 'cacao', 'cadet', 'cairn', 'camel', 'cargo',
];

// ── Charsets ───────────────────────────────────────────────────────────────
const CS_LOWER  = 'abcdefghijklmnopqrstuvwxyz';
const CS_UPPER  = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
const CS_NUMS   = '0123456789';
const CS_SYMS   = '!@#$%&*-_=+?';

// ── Entropy / strength ─────────────────────────────────────────────────────
function entropyBits(charsetSize: number, length: number): number {
  if (charsetSize <= 1) return 0;
  return Math.floor(length * Math.log2(charsetSize));
}
function passphraseEntropy(wordlistSize: number, wordCount: number, appendNum: boolean): number {
  const base = Math.floor(wordCount * Math.log2(wordlistSize));
  return appendNum ? base + Math.floor(Math.log2(100)) : base;
}

type StrengthKey = 'weak' | 'moderate' | 'strong';
function strengthOf(bits: number): { k: StrengthKey; pct: number; label: string } {
  if (bits < 45) return { k: 'weak',     pct: Math.round((bits / 45) * 33),         label: `Weak · ${bits} bits` };
  if (bits < 75) return { k: 'moderate', pct: Math.round(33 + ((bits - 45) / 30) * 34), label: `Moderate · ${bits} bits` };
  return            { k: 'strong',   pct: Math.min(100, Math.round(67 + ((bits - 75) / 53) * 33)), label: `Strong · ${bits} bits` };
}

// ── Generators ──────────────────────────────────────────────────────────────
function generateRandom(
  length: number,
  upper: boolean, lower: boolean, nums: boolean, syms: boolean,
): string {
  let charset = '';
  if (lower) charset += CS_LOWER;
  if (upper) charset += CS_UPPER;
  if (nums)  charset += CS_NUMS;
  if (syms)  charset += CS_SYMS;
  if (!charset) charset = CS_LOWER + CS_NUMS;

  // Rejection sampling for uniform distribution.
  const max256 = Math.floor(256 / charset.length) * charset.length;
  const buf = new Uint8Array(length * 8);
  crypto.getRandomValues(buf);

  const out: string[] = [];
  let i = 0;
  while (out.length < length) {
    if (i >= buf.length) {
      const extra = new Uint8Array(length * 4);
      crypto.getRandomValues(extra);
      buf.set(extra.slice(0, buf.length));
      i = 0;
    }
    const b = buf[i++];
    if (b < max256) out.push(charset[b % charset.length]);
  }
  return out.join('');
}

function generatePassphrase(
  wordCount: number,
  sep: string,
  capitalize: boolean,
  appendNum: boolean,
): string {
  const arr = new Uint32Array(wordCount + 1);
  crypto.getRandomValues(arr);
  const words = Array.from({ length: wordCount }, (_, i) => {
    const w = WORDLIST[arr[i] % WORDLIST.length];
    return capitalize ? w[0].toUpperCase() + w.slice(1) : w;
  });
  const num = appendNum ? sep + String(arr[wordCount] % 100).padStart(2, '0') : '';
  return words.join(sep) + num;
}

// ── Sub-components ──────────────────────────────────────────────────────────
function PwSlider({
  label, value, min, max,
  onChange,
}: {
  label: string; value: number; min: number; max: number;
  onChange: (v: number) => void;
}) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className="lg-pw__slider">
      <div className="top">
        <span className="k">{label}</span>
        <span className="v">{value}</span>
      </div>
      <div className="lg-pw__track">
        <i style={{ width: `${pct}%` }} />
        <span className="h" style={{ left: `calc(${pct}% - 3.5px)` }} />
        <input
          type="range"
          min={min}
          max={max}
          value={value}
          onChange={(e) => onChange(Number(e.target.value))}
          aria-label={label}
        />
      </div>
    </div>
  );
}

function PwOption({
  label, on, example, onToggle,
}: {
  label: string; on: boolean; example: string; onToggle: () => void;
}) {
  return (
    <button type="button" className={`lg-pw__opt${on ? ' on' : ''}`} onClick={onToggle}>
      <span className="b">{on ? '✓' : ''}</span>
      <span className="l">{label}</span>
      <span className="ex">{example}</span>
    </button>
  );
}

// ── Main component ──────────────────────────────────────────────────────────
interface Props {
  /** Called when the user clicks "Use this". Provides the generated password. */
  onUse?: (pw: string) => void;
  /** Hide the settings section (sliders/options) — for inline compact use. */
  compact?: boolean;
  /** Which mode to start in. Defaults to 'random'. */
  mode?: 'random' | 'passphrase';
}

export function PasswordGenerator({ onUse, compact = false, mode: initialMode = 'random' }: Props) {
  const [mode, setMode]             = useState<'random' | 'passphrase'>(initialMode);
  const [length, setLength]         = useState(20);
  const [inclUpper, setInclUpper]   = useState(true);
  const [inclLower, setInclLower]   = useState(true);
  const [inclNums, setInclNums]     = useState(true);
  const [inclSyms, setInclSyms]     = useState(true);
  const [wordCount, setWordCount]   = useState(4);
  const [sep, setSep]               = useState('-');
  const [capitalize, setCapitalize] = useState(true);
  const [appendNum, setAppendNum]   = useState(true);
  const [seed, setSeed]             = useState(0);     // bump to regenerate
  const [copied, setCopied]         = useState(false);

  const password = useMemo(() => {
    void seed; // tracked as dependency
    return mode === 'random'
      ? generateRandom(length, inclUpper, inclLower, inclNums, inclSyms)
      : generatePassphrase(wordCount, sep, capitalize, appendNum);
  }, [mode, length, inclUpper, inclLower, inclNums, inclSyms,
      wordCount, sep, capitalize, appendNum, seed]);

  const bits = mode === 'random'
    ? (() => {
        let cs = 0;
        if (inclLower) cs += CS_LOWER.length;
        if (inclUpper) cs += CS_UPPER.length;
        if (inclNums)  cs += CS_NUMS.length;
        if (inclSyms)  cs += CS_SYMS.length;
        return entropyBits(cs || 36, length);
      })()
    : passphraseEntropy(WORDLIST.length, wordCount, appendNum);

  const strength = strengthOf(bits);

  const regenerate = useCallback(() => setSeed((s) => s + 1), []);

  const copy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(password);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch { /* clipboard unavailable */ }
  }, [password]);

  return (
    <div className="lg-pw">
      {/* ── Header ──────────────────────────────────────────────────── */}
      <div className="lg-pw__head">
        <div className="lg-pw__title"><b>Password</b> · generator · client-side only</div>
        <div className="lg-pw__tabs">
          <button
            type="button"
            className={`lg-pw__tab${mode === 'random' ? ' on' : ''}`}
            onClick={() => setMode('random')}
          >
            Random
          </button>
          <button
            type="button"
            className={`lg-pw__tab${mode === 'passphrase' ? ' on' : ''}`}
            onClick={() => setMode('passphrase')}
          >
            Passphrase
          </button>
        </div>
      </div>

      {/* ── Output ──────────────────────────────────────────────────── */}
      <div className="lg-pw__out">
        <div className="lg-pw__pw" aria-label="Generated password">{password}</div>
        <div className="lg-pw__copy">
          <button
            type="button"
            className={`solid${copied ? ' copied' : ''}`}
            onClick={() => { void copy(); }}
          >
            {copied ? 'Copied ✓' : 'Copy ↗'}
          </button>
          <button type="button" onClick={regenerate}>Regenerate ⟳</button>
        </div>
      </div>

      {/* ── Strength ────────────────────────────────────────────────── */}
      <div className="lg-pw__strength">
        <div className="row">
          <span>Strength</span>
          <b className={strength.k}>{strength.label}</b>
        </div>
        <div className="lg-pw__bar">
          <i className={strength.k} style={{ width: `${strength.pct}%` }} />
        </div>
      </div>

      {/* ── Settings ────────────────────────────────────────────────── */}
      {!compact && (
        <div className="lg-pw__settings">
          {mode === 'random' ? (
            <>
              <PwSlider label="Length" value={length} min={8} max={48} onChange={setLength} />
              <div className="lg-pw__opts">
                <PwOption label="A — Z" on={inclUpper} example="UPPER" onToggle={() => setInclUpper((v) => !v)} />
                <PwOption label="a — z" on={inclLower} example="lower" onToggle={() => setInclLower((v) => !v)} />
                <PwOption label="0 — 9" on={inclNums}  example="0193"  onToggle={() => setInclNums((v) => !v)} />
                <PwOption label="!@#$%" on={inclSyms}  example="!@$&*" onToggle={() => setInclSyms((v) => !v)} />
              </div>
            </>
          ) : (
            <>
              <PwSlider label="Words" value={wordCount} min={3} max={8} onChange={setWordCount} />
              <div className="lg-pw__opts">
                {([
                  { label: 'Separator', on: true,       ex: sep,         action: () => setSep(sep === '-' ? ' ' : sep === ' ' ? '.' : '-') },
                  { label: 'Capitalize', on: capitalize, ex: 'Mosaic',   action: () => setCapitalize((v) => !v) },
                  { label: 'Append #',  on: appendNum,  ex: 'Mosaic 42', action: () => setAppendNum((v) => !v) },
                  { label: 'Wordlist',  on: true,        ex: '250 words', action: () => {} },
                ] as const).map(({ label, on, ex, action }) => (
                  <PwOption key={label} label={label} on={on} example={ex} onToggle={action} />
                ))}
              </div>
            </>
          )}
        </div>
      )}

      {/* ── Use this ────────────────────────────────────────────────── */}
      {onUse && (
        <div className="lg-pw__use">
          <span className="ln">Generated locally · <b>never sent</b> until form submit</span>
          <button
            type="button"
            className="lg-bt lg-bt--solid"
            onClick={() => onUse(password)}
          >
            Use this <span className="arr">↗</span>
          </button>
        </div>
      )}
    </div>
  );
}
