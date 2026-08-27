int trivial(int x) => x;

int atLimit(int x) {
  if (x == 0) x++; // +1 if
  if (x == 1) x++; // +1 if
  if (x == 2) x++; // +1 if
  if (x == 3) x++; // +1 if
  if (x == 4) x++; // +1 if
  if (x == 5) x++; // +1 if
  if (x == 6) x++; // +1 if
  if (x == 7) x++; // +1 if
  if (x == 8) x++; // +1 if
  if (x == 9) x++; // +1 if
  if (x == 10) x++; // +1 if
  if (x == 11) x++; // +1 if
  if (x == 12) x++; // +1 if
  if (x == 13) x++; // +1 if
  return x;
}

int overComplexity(int x) {
  if (x == 0) x++; // +1 if
  if (x == 1) x++; // +1 if
  if (x == 2) x++; // +1 if
  if (x == 3) x++; // +1 if
  if (x == 4) x++; // +1 if
  if (x == 5) x++; // +1 if
  if (x == 6) x++; // +1 if
  if (x == 7) x++; // +1 if
  if (x == 8) x++; // +1 if
  if (x == 9) x++; // +1 if
  if (x == 10) x++; // +1 if
  if (x == 11) x++; // +1 if
  if (x == 12) x++; // +1 if
  if (x == 13) x++; // +1 if
  if (x == 14) x++; // +1 if
  return x;
}

int overDepth(int x) {
  if (x > 0) { // +1 if
    while (x > 0) { // +1 while
      for (var i = 0; i < x; i++) { // +1 for
        try {
          if (i > 0) return i; // +1 if
        } catch (_) { return 0; } // +1 catch
      }
    }
  }
  return x;
}

int overLines(int x) {
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

int overParams(int a,int b,int c,int d,int e,int f,int g) => a;

int nested(int x) {
  int inner(int y) => y > 0 ? 1 : 0; // +1 ternary
  return inner(x);
}

int decisions(int x, int? value) {
  if (x > 0) x++; // +1 if
  for (var i = 0; i < x; i++) x += i; // +1 for
  for (final item in [x]) x += item; // +1 for-in
  while (x > 10) x--; // +1 while
  do { x++; } while (x < 0); // +1 do-while
  switch (x) { case 1: x++; case 2: x--; default: break; } // +2 cases
  x = switch (x) { 1 => 1, 2 => 2, _ => 0 }; // +2 cases
  try { x++; } catch (_) { x--; } // +1 catch
  x = x > 0 ? x : 0; // +1 ternary
  x = (x > 0 && x < 10) || value == null ? 1 : 0; // +3 && || ternary
  value ??= 0; // +1 ??=
  x = value ?? x; // +1 ??
  return x;
}

class Counter {
  int increment(int x) => x > 0 ? x + 1 : 0; // +1 ternary
}

int reviewCases(List<int> values, int x) {
  const marker = "&& || ??"; // operators inside strings add 0
  if (x == 0) x++; // +1 if
  else if (x == 1) x++; // +1 else-if
  else if (x == 2) x++; // +1 else-if
  else x--;
  try { x++; } catch (_) { x--; } finally { x += 0; } // +1 catch
  values.map((value) => value + marker.length); // anonymous callback: base 1
  return x;
}
