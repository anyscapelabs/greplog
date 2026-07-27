const ENCODING = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
const ENCODING_LEN = ENCODING.length;

let lastTimestamp = 0;
let lastRandom = 0;

function randomChar(): string {
  return ENCODING[Math.floor(Math.random() * ENCODING_LEN)];
}

export function generateULID(): string {
  const now = Date.now();
  if (now !== lastTimestamp) {
    lastTimestamp = now;
    lastRandom = 0;
  }
  lastRandom++;

  const timestampStr = timestampToULID(now);
  const randomStr = randomToULID(lastRandom);

  return timestampStr + randomStr;
}

function timestampToULID(ts: number): string {
  let str = '';
  let val = ts;
  for (let i = 0; i < 10; i++) {
    str = ENCODING[val % ENCODING_LEN] + str;
    val = Math.floor(val / ENCODING_LEN);
  }
  return str;
}

function randomToULID(seq: number): string {
  let str = '';
  for (let i = 0; i < 16; i++) {
    const idx = i < 4
      ? Math.floor(Math.random() * ENCODING_LEN)
      : ((seq >> ((i - 4) * 5)) + Math.floor(Math.random() * 8)) % ENCODING_LEN;
    str += ENCODING[idx];
  }
  return str;
}
