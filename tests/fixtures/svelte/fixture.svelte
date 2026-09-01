<script lang="ts">
function trivial(x: number): number { return x; }

function atLimit(x: number): number {
  if (x === 0) x++;
  if (x === 1) x++;
  if (x === 2) x++;
  if (x === 3) x++;
  if (x === 4) x++;
  if (x === 5) x++;
  if (x === 6) x++;
  if (x === 7) x++;
  if (x === 8) x++;
  if (x === 9) x++;
  if (x === 10) x++;
  if (x === 11) x++;
  if (x === 12) x++;
  if (x === 13) x++;
  return x;
}

function overComplexity(x: number): number {
  if (x === 0) x++;
  if (x === 1) x++;
  if (x === 2) x++;
  if (x === 3) x++;
  if (x === 4) x++;
  if (x === 5) x++;
  if (x === 6) x++;
  if (x === 7) x++;
  if (x === 8) x++;
  if (x === 9) x++;
  if (x === 10) x++;
  if (x === 11) x++;
  if (x === 12) x++;
  if (x === 13) x++;
  if (x === 14) x++;
  return x;
}

function overDepth(x: number): number {
  if (x) {
    while (x) {
      for (;;) {
        try {
          if (x) return x;
        } catch (error) { return 0; }
      }
    }
  }
  return x;
}

function overLines(x: number): number {
  x += 0;
  x += 1;
  x += 2;
  x += 3;
  x += 4;
  x += 5;
  x += 6;
  x += 7;
  x += 8;
  x += 9;
  x += 10;
  x += 11;
  x += 12;
  x += 13;
  x += 14;
  x += 15;
  x += 16;
  x += 17;
  x += 18;
  x += 19;
  x += 20;
  x += 21;
  x += 22;
  x += 23;
  x += 24;
  x += 25;
  x += 26;
  x += 27;
  x += 28;
  x += 29;
  x += 30;
  x += 31;
  x += 32;
  x += 33;
  x += 34;
  x += 35;
  x += 36;
  x += 37;
  x += 38;
  x += 39;
  x += 40;
  x += 41;
  x += 42;
  x += 43;
  x += 44;
  x += 45;
  x += 46;
  x += 47;
  x += 48;
  x += 49;
  x += 50;
  x += 51;
  x += 52;
  x += 53;
  x += 54;
  x += 55;
  x += 56;
  x += 57;
  x += 58;
  x += 59;
  x += 60;
  x += 61;
  x += 62;
  x += 63;
  x += 64;
  x += 65;
  x += 66;
  x += 67;
  x += 68;
  x += 69;
  x += 70;
  x += 71;
  x += 72;
  x += 73;
  x += 74;
  x += 75;
  x += 76;
  x += 77;
  x += 78;
  x += 79;
  x += 80;
  x += 81;
  x += 82;
  x += 83;
  x += 84;
  x += 85;
  x += 86;
  x += 87;
  x += 88;
  x += 89;
  x += 90;
  x += 91;
  x += 92;
  x += 93;
  x += 94;
  x += 95;
  x += 96;
  x += 97;
  x += 98;
  return x;
}

function overParams(p0: number, p1: number, p2: number, p3: number, p4: number, p5: number, p6: number): number { return p0; }

function nested(x: number): number {
  const inner = (x: number): number => x ? 1 : 0;
  return inner(x);
}

function decisions(x: number): number {
  if (x) x++; else if (x < 0) x--;
  for (let i = 0; i < x; i++) x++;
  for (const key in { x }) x += key.length;
  while (x > 10) x--;
  do { x++; } while (x < 0);
  switch (x) { case 1: x++; break; case 2: x--; break; default: break; }
  try { x++; } catch (error) { x--; }
  x = x ? x : 0;
  x = (x && 1) || (x ?? 0);
  x &&= 1; x ||= 1; x ??= 1;
  return x;
}

class Counter {
  increment(x: number): number { if (x) return x + 1; return 0; }
}

function reviewCases(values: number[], x: number): number {
  const marker = "&& || ??"; // operators inside strings add 0
  if (x === 0) x++; // +1 if
  else if (x === 1) x++; // +1 else-if
  else if (x === 2) x++; // +1 else-if
  else x--;
  try { x++; } catch (error) { x--; } finally { x += 0; } // +1 catch
  values.map(value => value + marker.length); // anonymous callback: base 1
  return x;
}

function commentLines(x: number): number {
  /* comment-only */
  /* inline */ return x;
}

function boolChain(a: boolean, b: boolean, c: boolean, d: boolean): boolean {
  return a && (b || c) && d;
}

function ternaryBreak(a: boolean, b: boolean, c: boolean, d: boolean, e: boolean): boolean {
  return (a && b) ? (c || d) : e;
}
</script>

<!-- +1 template if -->
{#if value && ready} <!-- +1 && -->
  <!-- +1 nested each -->
  {#each items as item}
    <!-- +1 nested await -->
    {#await load(item)}
      <p>{item ? item : value}</p> <!-- +1 ternary -->
    {:catch error} <!-- +1 catch -->
      <p>{error}</p>
    {/await}
  {/each}
{:else if fallback || ready} <!-- +1 else-if, +1 || -->
  <p>{fallback ?? value}</p> <!-- +1 ?? -->
{/if}

<p>{`s: ${ready && value}`}</p>
<p>{a && b /* || */}</p>

<style>.x { color: red; }</style>
