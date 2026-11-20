// Fabrique les petits sons intégrés (WAV mono 16 bits). Usage : node scripts/make-sounds.mjs
import { writeFileSync, mkdirSync } from "node:fs";
const SR = 44100;
const out = new URL("../src-tauri/sounds/", import.meta.url);
mkdirSync(out, { recursive: true });

function render(seconds, fn) {
  const n = Math.floor(seconds * SR);
  const a = new Float64Array(n);
  for (let i = 0; i < n; i++) a[i] = fn(i / SR);
  let peak = 0;
  for (const v of a) peak = Math.max(peak, Math.abs(v));
  const fade = Math.floor(0.012 * SR), tail = Math.floor(0.02 * SR);
  for (let i = 0; i < n; i++) {
    let g = 0.5 / (peak || 1);
    if (i < fade) g *= i / fade;
    if (i > n - tail) g *= (n - i) / tail;
    a[i] *= g;
  }
  return a;
}
function wav(name, a) {
  const b = Buffer.alloc(44 + a.length * 2);
  b.write("RIFF", 0); b.writeUInt32LE(36 + a.length * 2, 4); b.write("WAVEfmt ", 8);
  b.writeUInt32LE(16, 16); b.writeUInt16LE(1, 20); b.writeUInt16LE(1, 22);
  b.writeUInt32LE(SR, 24); b.writeUInt32LE(SR * 2, 28); b.writeUInt16LE(2, 32); b.writeUInt16LE(16, 34);
  b.write("data", 36); b.writeUInt32LE(a.length * 2, 40);
  a.forEach((v, i) => b.writeInt16LE(Math.round(Math.max(-1, Math.min(1, v)) * 32767), 44 + i * 2));
  writeFileSync(new URL(name + ".wav", out), b);
}
const TAU = Math.PI * 2;
const note = (t, t0, f, decay) => t < t0 ? 0 :
  Math.exp(-(t - t0) / decay) * (1 - Math.exp(-(t - t0) / 0.008)) * (Math.sin(TAU * f * (t - t0)) + 0.12 * Math.sin(TAU * 2 * f * (t - t0)));

// Carillon : deux notes montantes
wav("carillon", render(0.9, t => note(t, 0, 523.25, 0.2) + note(t, 0.16, 659.25, 0.26)));

// Goutte : deux petites gouttes descendantes
const drop = (t, t0, f1, f2, d) => {
  if (t < t0) return 0; const x = t - t0;
  const f = f2 + (f1 - f2) * Math.exp(-x / 0.03);
  return Math.exp(-x / d) * Math.sin(TAU * (f2 * x + (f1 - f2) * 0.03 * (1 - Math.exp(-x / 0.03))));
};
wav("goutte", render(0.5, t => drop(t, 0, 900, 480, 0.08)));

// Toc toc : deux coups sur du bois
let seed = 12345; const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32) * 2 - 1;
let lp = 0;
const knock = (t, t0) => {
  if (t < t0) return 0; const x = t - t0;
  lp += 0.25 * (rnd() - lp);
  return Math.exp(-x / 0.035) * Math.sin(TAU * (210 - 300 * x) * x) + 0.5 * Math.exp(-x / 0.012) * lp
    + 0.35 * Math.exp(-x / 0.02) * Math.sin(TAU * 640 * x);
};
wav("toctoc", render(0.5, t => knock(t, 0) + 0.9 * knock(t, 0.17)));

// Cloche : partiels de cloche
const bell = (t) => [[1, 1, 0.45], [2.76, 0.28, 0.22], [5.4, 0.08, 0.12]]
  .reduce((s, [m, a, d]) => s + a * Math.exp(-t / d) * Math.sin(TAU * 440 * m * t), 0);
wav("cloche", render(1.6, bell));

// Bulle : petit « pop » montant
wav("bulle", render(0.3, t => {
  const f1 = 300, f2 = 760, T = 0.12;
  const ph = t < T ? f1 * t + (f2 - f1) * t * t / (2 * T) : f1 * T + (f2 - f1) * T / 2 + f2 * (t - T);
  return Math.exp(-t / 0.07) * Math.sin(TAU * ph);
}));
console.log("Sons écrits dans src-tauri/sounds/");
