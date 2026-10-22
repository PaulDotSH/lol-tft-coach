// Formatting shared by the draft / team panels.
/** Signed points: "+1.9", "−0.5", "±0.0". */
export const pts = (n: number) => (n > 0 ? `+${n.toFixed(1)}` : n < 0 ? `−${(-n).toFixed(1)}` : '±0.0');
export const tone = (n: number, eps = 0.25) => (n > eps ? 'good' : n < -eps ? 'bad' : 'muted');
