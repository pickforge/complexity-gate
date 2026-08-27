fn trivial(x: i32) -> i32 { x }

fn at_limit(mut x: i32) -> i32 {
    if x == 0 { x += 1; }
    if x == 1 { x += 1; }
    if x == 2 { x += 1; }
    if x == 3 { x += 1; }
    if x == 4 { x += 1; }
    if x == 5 { x += 1; }
    if x == 6 { x += 1; }
    if x == 7 { x += 1; }
    if x == 8 { x += 1; }
    if x == 9 { x += 1; }
    if x == 10 { x += 1; }
    if x == 11 { x += 1; }
    if x == 12 { x += 1; }
    if x == 13 { x += 1; }
    x
}

fn over_complexity(mut x: i32) -> i32 {
    if x == 0 { x += 1; }
    if x == 1 { x += 1; }
    if x == 2 { x += 1; }
    if x == 3 { x += 1; }
    if x == 4 { x += 1; }
    if x == 5 { x += 1; }
    if x == 6 { x += 1; }
    if x == 7 { x += 1; }
    if x == 8 { x += 1; }
    if x == 9 { x += 1; }
    if x == 10 { x += 1; }
    if x == 11 { x += 1; }
    if x == 12 { x += 1; }
    if x == 13 { x += 1; }
    if x == 14 { x += 1; }
    x
}

fn over_depth(x: i32) -> i32 {
    if x > 0 {
        while true {
            for i in 0..x {
                loop {
                    if i > 0 { return i; }
                    break;
                }
            }
            break;
        }
    }
    x
}

fn over_lines(mut x: i32) -> i32 {
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
    x
}

fn over_params(a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32)->i32 { a }

fn nested(x: i32) -> i32 {
    let inner = |y: i32| y + 1;
    inner(x)
}

fn decisions(mut x: i32, value: Option<i32>) -> i32 {
    if x > 0 { x += 1; }
    if let Some(v) = value { x += v; }
    for i in 0..x { x += i; }
    while x > 10 { x -= 1; }
    loop { break; }
    match x { 1 => x += 1, 2 => x -= 1, _ => {} }
    if x > 0 && x < 10 || x == 20 { x += 1; }
    let Some(v) = value else { return x; };
    x + v
}

fn review_cases(values: &[i32], mut x: i32, result: Result<(), ()>) -> i32 {
    let marker = "&& ||";
    if x == 0 { x += 1; }
    else if x == 1 { x += 1; }
    else if x == 2 { x += 1; }
    else { x -= 1; }
    if result.is_err() { x -= 1; } // Result error branch is the catch equivalent.
    values.iter().map(|value| *value + marker.len() as i32).sum::<i32>();
    x
}
